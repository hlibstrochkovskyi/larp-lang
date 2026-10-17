//! Non-moving traced storage with checked generations and iterative marking.

use crate::value::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Handle {
    index: usize,
    generation: u64,
}

#[derive(Debug)]
pub(crate) struct Closure {
    pub function: usize,
    pub captures: Vec<Value>,
}

#[derive(Debug)]
pub(crate) struct Array {
    pub values: Vec<Value>,
    pub iterations: usize,
}

#[derive(Debug)]
pub(crate) struct Instance {
    pub class: usize,
    pub fields: Vec<Value>,
}

#[derive(Debug)]
pub(crate) enum Object {
    Closure(Closure),
    Array(Array),
    Instance(Instance),
}

impl From<Closure> for Object {
    fn from(closure: Closure) -> Self {
        Self::Closure(closure)
    }
}

impl From<Array> for Object {
    fn from(array: Array) -> Self {
        Self::Array(array)
    }
}

impl From<Instance> for Object {
    fn from(instance: Instance) -> Self {
        Self::Instance(instance)
    }
}

struct Slot {
    generation: u64,
    object: Option<Object>,
}

pub(crate) struct Heap {
    slots: Vec<Slot>,
    free: Vec<usize>,
    next_collection: usize,
    pub allocated: u64,
    pub reclaimed: u64,
    pub collections: u64,
    pub live: usize,
    pub peak_live: usize,
}

impl Default for Heap {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            next_collection: 64,
            allocated: 0,
            reclaimed: 0,
            collections: 0,
            live: 0,
            peak_live: 0,
        }
    }
}

impl Heap {
    pub fn should_collect(&self, stress: bool) -> bool {
        stress || self.live >= self.next_collection
    }

    pub fn allocate(&mut self, object: impl Into<Object>) -> Result<Handle, &'static str> {
        let object = object.into();
        let index = if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index];
            slot.generation = slot
                .generation
                .checked_add(1)
                .ok_or("Heap handle generation exhausted")?;
            slot.object = Some(object);
            index
        } else {
            let index = self.slots.len();
            self.slots.push(Slot {
                generation: 0,
                object: Some(object),
            });
            index
        };
        self.allocated += 1;
        self.live += 1;
        self.peak_live = self.peak_live.max(self.live);
        Ok(Handle {
            index,
            generation: self.slots[index].generation,
        })
    }

    fn object(&self, handle: Handle) -> Option<&Object> {
        self.slots
            .get(handle.index)
            .filter(|slot| slot.generation == handle.generation)?
            .object
            .as_ref()
    }

    fn object_mut(&mut self, handle: Handle) -> Option<&mut Object> {
        self.slots
            .get_mut(handle.index)
            .filter(|slot| slot.generation == handle.generation)?
            .object
            .as_mut()
    }

    pub fn get(&self, handle: Handle) -> Option<&Closure> {
        match self.object(handle)? {
            Object::Closure(closure) => Some(closure),
            _ => None,
        }
    }

    pub fn array(&self, handle: Handle) -> Option<&Array> {
        match self.object(handle)? {
            Object::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn array_mut(&mut self, handle: Handle) -> Option<&mut Array> {
        match self.object_mut(handle)? {
            Object::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn instance(&self, handle: Handle) -> Option<&Instance> {
        match self.object(handle)? {
            Object::Instance(instance) => Some(instance),
            _ => None,
        }
    }

    pub fn instance_mut(&mut self, handle: Handle) -> Option<&mut Instance> {
        match self.object_mut(handle)? {
            Object::Instance(instance) => Some(instance),
            _ => None,
        }
    }

    pub fn collect<'a>(&mut self, roots: impl Iterator<Item = &'a Value>) {
        let mut pending = Vec::new();
        for root in roots {
            root.handles(&mut pending);
        }
        let mut marked = vec![false; self.slots.len()];
        while let Some(handle) = pending.pop() {
            if marked.get(handle.index) != Some(&false) {
                continue;
            }
            let Some(object) = self.object(handle) else {
                continue;
            };
            marked[handle.index] = true;
            let values = match object {
                Object::Closure(closure) => &closure.captures,
                Object::Array(array) => &array.values,
                Object::Instance(instance) => &instance.fields,
            };
            for value in values {
                value.handles(&mut pending);
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !marked[index] && slot.object.take().is_some() {
                self.free.push(index);
                self.reclaimed += 1;
                self.live -= 1;
            }
        }
        self.collections += 1;
        self.next_collection = self.live.saturating_mul(2).max(64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generations_reject_stale_handles_and_live_captures_are_transitively_rooted() {
        let mut heap = Heap::default();
        let child = heap
            .allocate(Closure {
                function: 0,
                captures: vec![],
            })
            .unwrap();
        let parent = heap
            .allocate(Closure {
                function: 1,
                captures: vec![Value::Option(Some(Box::new(Value::Closure(child))))],
            })
            .unwrap();
        let root = Value::Closure(parent);
        heap.collect(std::iter::once(&root));
        assert!(heap.get(parent).is_some());
        assert!(heap.get(child).is_some());
        heap.collect(std::iter::empty());
        assert!(heap.get(parent).is_none());
        assert!(heap.get(child).is_none());
        let reused = heap
            .allocate(Closure {
                function: 2,
                captures: vec![],
            })
            .unwrap();
        assert!(heap.get(reused).is_some());
        assert!(heap.get(parent).is_none());
        assert_eq!(heap.reclaimed, 2);
    }

    #[test]
    fn unreachable_cycles_and_deep_capture_graphs_are_collected_iteratively() {
        let mut heap = Heap::default();
        let first = heap
            .allocate(Closure {
                function: 0,
                captures: vec![],
            })
            .unwrap();
        let second = heap
            .allocate(Closure {
                function: 0,
                captures: vec![Value::Closure(first)],
            })
            .unwrap();
        let Some(Object::Closure(closure)) = heap.slots[first.index].object.as_mut() else {
            unreachable!()
        };
        closure.captures.push(Value::Closure(second));
        let mut current = second;
        for _ in 0..20_000 {
            current = heap
                .allocate(Closure {
                    function: 0,
                    captures: vec![Value::Closure(current)],
                })
                .unwrap();
        }
        heap.collect(std::iter::once(&Value::Closure(current)));
        assert_eq!(heap.live, 20_002);
        heap.collect(std::iter::empty());
        assert_eq!(heap.live, 0);
        assert_eq!(heap.reclaimed, 20_002);
    }
    #[test]
    fn array_closure_cycles_and_fixed_array_payloads_are_traced_and_reclaimed() {
        let mut heap = Heap::default();
        let array = heap
            .allocate(Array {
                values: vec![],
                iterations: 0,
            })
            .unwrap();
        let closure = heap
            .allocate(Closure {
                function: 0,
                captures: vec![Value::Result(Ok(Box::new(Value::FixedArray(vec![
                    Value::Array(array),
                ]))))],
            })
            .unwrap();
        heap.array_mut(array)
            .unwrap()
            .values
            .push(Value::Closure(closure));
        let root = Value::Option(Some(Box::new(Value::FixedArray(vec![Value::Array(array)]))));
        heap.collect(std::iter::once(&root));
        assert!(heap.array(array).is_some());
        assert!(heap.get(closure).is_some());
        assert_eq!(heap.live, 2);
        heap.collect(std::iter::empty());
        assert_eq!(heap.live, 0);
        assert!(heap.array(array).is_none());
        assert!(heap.get(closure).is_none());
        let new_array = heap
            .allocate(Array {
                values: vec![],
                iterations: 0,
            })
            .unwrap();
        assert!(heap.array(new_array).is_some());
        assert!(heap.get(closure).is_none());
        assert!(heap.array(closure).is_none());
    }

    #[test]
    fn deep_array_edges_do_not_recurse_during_marking_or_sweeping() {
        let mut heap = Heap::default();
        let mut handle = heap
            .allocate(Array {
                values: vec![],
                iterations: 0,
            })
            .unwrap();
        for _ in 0..20_000 {
            handle = heap
                .allocate(Array {
                    values: vec![Value::Array(handle)],
                    iterations: 0,
                })
                .unwrap();
        }
        heap.collect(std::iter::once(&Value::Array(handle)));
        assert_eq!(heap.live, 20_001);
        heap.collect(std::iter::empty());
        assert_eq!(heap.live, 0);
    }
    #[test]
    fn objects_arrays_and_closures_form_traced_cycles_with_stable_identities() {
        let mut heap = Heap::default();
        let instance = heap
            .allocate(Instance {
                class: 0,
                fields: vec![],
            })
            .unwrap();
        let array = heap
            .allocate(Array {
                values: vec![Value::Object(instance)],
                iterations: 0,
            })
            .unwrap();
        let closure = heap
            .allocate(Closure {
                function: 0,
                captures: vec![Value::Array(array)],
            })
            .unwrap();
        heap.instance_mut(instance)
            .unwrap()
            .fields
            .push(Value::Option(Some(Box::new(Value::Closure(closure)))));
        let root = Value::FixedArray(vec![Value::Result(Ok(Box::new(Value::Object(instance))))]);
        heap.collect(std::iter::once(&root));
        assert_eq!(heap.live, 3);
        assert_eq!(heap.instance(instance).unwrap().class, 0);
        assert!(heap.array(array).is_some());
        assert!(heap.get(closure).is_some());
        assert!(Value::Object(instance).equals(&Value::Object(instance)));
        heap.collect(std::iter::empty());
        assert_eq!(heap.live, 0);
        assert_eq!(heap.reclaimed, 3);
        let replacement = heap
            .allocate(Instance {
                class: 0,
                fields: vec![],
            })
            .unwrap();
        assert!(!Value::Object(replacement).equals(&Value::Object(instance)));
        assert!(heap.instance(instance).is_none());
    }

    #[test]
    fn deep_object_chains_are_collected_without_recursive_graph_destruction() {
        let mut heap = Heap::default();
        let mut handle = heap
            .allocate(Instance {
                class: 0,
                fields: vec![],
            })
            .unwrap();
        for _ in 0..20_000 {
            handle = heap
                .allocate(Instance {
                    class: 0,
                    fields: vec![Value::Object(handle)],
                })
                .unwrap();
        }
        heap.collect(std::iter::once(&Value::Object(handle)));
        assert_eq!(heap.live, 20_001);
        heap.collect(std::iter::empty());
        assert_eq!(heap.live, 0);
    }
}

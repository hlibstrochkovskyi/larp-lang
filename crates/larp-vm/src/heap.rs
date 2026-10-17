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

struct Slot {
    generation: u64,
    closure: Option<Closure>,
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

    pub fn allocate(&mut self, closure: Closure) -> Result<Handle, &'static str> {
        let index = if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index];
            slot.generation = slot
                .generation
                .checked_add(1)
                .ok_or("Heap handle generation exhausted")?;
            slot.closure = Some(closure);
            index
        } else {
            let index = self.slots.len();
            self.slots.push(Slot {
                generation: 0,
                closure: Some(closure),
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

    pub fn get(&self, handle: Handle) -> Option<&Closure> {
        self.slots
            .get(handle.index)
            .filter(|slot| slot.generation == handle.generation)?
            .closure
            .as_ref()
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
            let Some(closure) = self.get(handle) else {
                continue;
            };
            marked[handle.index] = true;
            for capture in &closure.captures {
                capture.handles(&mut pending);
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !marked[index] && slot.closure.take().is_some() {
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
        heap.slots[first.index]
            .closure
            .as_mut()
            .unwrap()
            .captures
            .push(Value::Closure(second));
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
}

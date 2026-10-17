//! Executable experiment for baseline/performed/selection/claim transitions.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Mf(i32),
    Unc(i64),
    Verdict(bool),
    Text(String),
}

impl Value {
    fn kind(&self) -> &'static str {
        match self {
            Self::Mf(_) => "mf",
            Self::Unc(_) => "unc",
            Self::Verdict(_) => "verdict",
            Self::Text(_) => "text",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    DuplicateField(String),
    UnknownField(String),
    WrongType {
        field: String,
        expected: &'static str,
        found: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    fields: BTreeMap<String, Value>,
}

impl State {
    pub fn new(fields: impl IntoIterator<Item = (String, Value)>) -> Result<Self, ModelError> {
        let mut result = BTreeMap::new();
        for (field, value) in fields {
            if result.contains_key(&field) {
                return Err(ModelError::DuplicateField(field));
            }
            result.insert(field, value);
        }
        Ok(Self { fields: result })
    }

    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }
}

pub struct Scenario<'a> {
    root: &'a mut State,
    baseline: State,
    performed: State,
    selected: BTreeSet<String>,
    claims: BTreeMap<String, Value>,
}

pub struct Row<'a> {
    pub field: &'a str,
    pub baseline: &'a Value,
    pub performed: &'a Value,
    pub claim: Option<&'a Value>,
    pub selected: bool,
}

impl<'a> Scenario<'a> {
    pub fn enter(root: &'a mut State) -> Self {
        Self {
            baseline: root.clone(),
            performed: root.clone(),
            root,
            selected: BTreeSet::new(),
            claims: BTreeMap::new(),
        }
    }

    pub fn get(&self, field: &str) -> Option<&Value> {
        self.performed.get(field)
    }

    pub fn set(&mut self, field: &str, value: Value) -> Result<(), ModelError> {
        let previous = self
            .performed
            .fields
            .get_mut(field)
            .ok_or_else(|| ModelError::UnknownField(field.into()))?;
        if previous.kind() != value.kind() {
            return Err(ModelError::WrongType {
                field: field.into(),
                expected: previous.kind(),
                found: value.kind(),
            });
        }
        *previous = value;
        Ok(())
    }

    pub fn flex(&mut self, field: &str, value: Value) -> Result<(), ModelError> {
        self.set(field, value.clone())?;
        self.claims.insert(field.into(), value);
        Ok(())
    }

    pub fn canon(&mut self, field: &str) -> Result<(), ModelError> {
        if !self.performed.fields.contains_key(field) {
            return Err(ModelError::UnknownField(field.into()));
        }
        self.selected.insert(field.into());
        Ok(())
    }

    pub fn inspect(&self) -> Vec<Row<'_>> {
        self.baseline
            .fields
            .iter()
            .map(|(field, baseline)| Row {
                field,
                baseline,
                performed: &self.performed.fields[field],
                claim: self.claims.get(field),
                selected: self.selected.contains(field),
            })
            .collect()
    }

    /// Consuming explicit completion corresponds to normal block fallthrough.
    /// Every allocation happens before the real root is replaced.
    pub fn commit(self) {
        let mut next = self.baseline;
        for field in self.selected {
            next.fields
                .insert(field.clone(), self.performed.fields[&field].clone());
        }
        *self.root = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn founder() -> State {
        State::new([
            ("valuation".into(), Value::Unc(20)),
            ("aura".into(), Value::Mf(0)),
            ("name".into(), Value::Text("bro".into())),
        ])
        .unwrap()
    }

    #[test]
    fn selection_commits_final_value_and_discards_unselected_claim() {
        let mut real = founder();
        let mut scenario = Scenario::enter(&mut real);
        scenario
            .flex("valuation", Value::Unc(1_000_000_000))
            .unwrap();
        scenario.set("aura", Value::Mf(2)).unwrap();
        scenario.canon("aura").unwrap();
        scenario.canon("aura").unwrap();
        scenario.set("aura", Value::Mf(3)).unwrap();
        let report = scenario.inspect();
        let valuation = report.iter().find(|row| row.field == "valuation").unwrap();
        assert_eq!(valuation.baseline, &Value::Unc(20));
        assert_eq!(valuation.claim, Some(&Value::Unc(1_000_000_000)));
        assert!(!valuation.selected);
        scenario.commit();
        assert_eq!(real.get("valuation"), Some(&Value::Unc(20)));
        assert_eq!(real.get("aura"), Some(&Value::Mf(3)));
    }

    #[test]
    fn ordinary_write_preserves_last_claim_and_selection_may_precede_write() {
        let mut real = founder();
        let mut scenario = Scenario::enter(&mut real);
        scenario.canon("aura").unwrap();
        scenario.flex("aura", Value::Mf(500)).unwrap();
        scenario.flex("aura", Value::Mf(600)).unwrap();
        scenario.set("aura", Value::Mf(2)).unwrap();
        let report = scenario.inspect();
        let aura = report.iter().find(|row| row.field == "aura").unwrap();
        assert_eq!(aura.claim, Some(&Value::Mf(600)));
        assert_eq!(aura.performed, &Value::Mf(2));
        assert!(aura.selected);
        scenario.commit();
        assert_eq!(real.get("aura"), Some(&Value::Mf(2)));
    }

    #[test]
    fn discard_and_host_unwind_leave_baseline_intact() {
        let mut real = founder();
        let baseline = real.clone();
        {
            let mut scenario = Scenario::enter(&mut real);
            scenario.set("aura", Value::Mf(500)).unwrap();
            scenario.canon("aura").unwrap();
        }
        assert_eq!(real, baseline);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut scenario = Scenario::enter(&mut real);
            scenario.set("aura", Value::Mf(500)).unwrap();
            scenario.canon("aura").unwrap();
            panic!("injected host unwind; not a LARP runtime error");
        }));
        assert!(result.is_err());
        assert_eq!(real, baseline);
    }

    #[test]
    fn complete_selection_matrix_preserves_exactly_the_chosen_fields() {
        for mask in 0..8 {
            let mut real = founder();
            let baseline = real.clone();
            let fields = ["aura", "valuation", "name"];
            let values = [Value::Mf(49), Value::Unc(100), Value::Text("unc".into())];
            let mut scenario = Scenario::enter(&mut real);
            for (index, field) in fields.iter().enumerate() {
                scenario.set(field, values[index].clone()).unwrap();
                if mask & (1 << index) != 0 {
                    scenario.canon(field).unwrap();
                }
            }
            scenario.commit();
            for (index, field) in fields.iter().enumerate() {
                let expected = if mask & (1 << index) != 0 {
                    Some(&values[index])
                } else {
                    baseline.get(field)
                };
                assert_eq!(real.get(field), expected, "mask={mask}, field={field}");
            }
        }
    }

    #[test]
    fn invalid_writes_and_selections_do_not_change_state() {
        let mut real = founder();
        let baseline = real.clone();
        let mut scenario = Scenario::enter(&mut real);
        assert!(matches!(
            scenario.flex("aura", Value::Unc(9)),
            Err(ModelError::WrongType { .. })
        ));
        assert!(matches!(
            scenario.canon("missing"),
            Err(ModelError::UnknownField(_))
        ));
        assert!(
            scenario
                .inspect()
                .iter()
                .all(|row| row.claim.is_none() && !row.selected)
        );
        scenario.commit();
        assert_eq!(real, baseline);
    }

    #[test]
    fn sequential_scenarios_observe_the_previous_committed_baseline() {
        let mut real = founder();
        let mut first = Scenario::enter(&mut real);
        first.set("aura", Value::Mf(1)).unwrap();
        first.canon("aura").unwrap();
        first.commit();
        let second = Scenario::enter(&mut real);
        let report = second.inspect();
        let aura = report.iter().find(|row| row.field == "aura").unwrap();
        assert_eq!(aura.baseline, &Value::Mf(1));
        assert_eq!(aura.performed, &Value::Mf(1));
        assert!(!aura.selected);
    }

    #[test]
    fn configuration_trial_selects_workers_only_when_budget_allows() {
        for (budget, expected_workers) in [(100, 8), (90, 2)] {
            let mut real = State::new([
                ("workers".into(), Value::Mf(2)),
                ("retryDelayMs".into(), Value::Mf(1000)),
            ])
            .unwrap();
            let mut scenario = Scenario::enter(&mut real);
            scenario.flex("workers", Value::Mf(8)).unwrap();
            scenario.set("retryDelayMs", Value::Mf(250)).unwrap();
            let Some(Value::Mf(workers)) = scenario.get("workers") else {
                panic!("workers must retain its mf schema");
            };
            if workers * 12 <= budget {
                scenario.canon("workers").unwrap();
            }
            scenario.commit();
            assert_eq!(real.get("workers"), Some(&Value::Mf(expected_workers)));
            assert_eq!(real.get("retryDelayMs"), Some(&Value::Mf(1000)));
        }
    }

    #[test]
    fn selective_commit_does_not_preserve_cross_field_business_invariants() {
        let mut real = State::new([
            ("balance".into(), Value::Mf(100)),
            ("reserved".into(), Value::Mf(20)),
        ])
        .unwrap();
        let mut scenario = Scenario::enter(&mut real);
        scenario.set("balance", Value::Mf(10)).unwrap();
        scenario.set("reserved", Value::Mf(0)).unwrap();
        scenario.canon("balance").unwrap();
        scenario.commit();
        let Some(Value::Mf(balance)) = real.get("balance") else {
            panic!("balance must retain its mf schema");
        };
        let Some(Value::Mf(reserved)) = real.get("reserved") else {
            panic!("reserved must retain its mf schema");
        };
        assert!(reserved > balance);
    }
}

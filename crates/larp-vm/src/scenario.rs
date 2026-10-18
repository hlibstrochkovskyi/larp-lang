//! Isolated value snapshots and single-publication selected-field commits.

use crate::heap::{Handle, Heap};
use crate::value::Value;
use larp_bytecode::Class;
use larp_types::Scalar;
use std::fmt;
use std::io::Write;

pub(crate) struct Scenario {
    pub entry: usize,
    pub class: usize,
    pub label: String,
    pub root: Handle,
    pub baseline: Vec<Value>,
    pub performed: Vec<Value>,
    pub claims: Vec<Option<Value>>,
    pub selected: Vec<bool>,
}

impl Scenario {
    pub fn new(
        entry: usize,
        class: usize,
        label: String,
        root: Handle,
        baseline: Vec<Value>,
    ) -> Self {
        Self {
            entry,
            class,
            label,
            root,
            performed: baseline.clone(),
            claims: vec![None; baseline.len()],
            selected: vec![false; baseline.len()],
            baseline,
        }
    }

    pub fn check_receiver(&self, receiver: Value, class: usize) {
        assert!(
            matches!(receiver, Value::View { entry } if entry == self.entry),
            "verified active scenario view"
        );
        assert_eq!(class, self.class, "verified scenario layout");
    }

    pub fn commit(self, heap: &mut Heap) {
        let prepared = self
            .baseline
            .into_iter()
            .zip(self.performed)
            .zip(self.selected)
            .map(|((baseline, performed), selected)| if selected { performed } else { baseline })
            .collect();
        heap.instance_mut(self.root)
            .expect("rooted live scenario root")
            .fields = prepared;
    }

    pub fn inspect(
        &self,
        class: &Class,
        fields: &[usize],
        inspection: &mut dyn Write,
    ) -> std::io::Result<()> {
        writeln!(inspection, "PALANTIR [{}]", self.label)?;
        writeln!(
            inspection,
            "PROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED"
        )?;
        for &field in fields {
            write!(
                inspection,
                "{} | {} | {} | ",
                class.fields[field].name,
                Snapshot(&self.baseline[field]),
                Snapshot(&self.performed[field])
            )?;
            if let Some(claim) = &self.claims[field] {
                write!(inspection, "{}", Snapshot(claim))?;
            } else {
                write!(inspection, "-")?;
            }
            writeln!(inspection, " | {}", self.selected[field])?;
        }
        Ok(())
    }
}

struct Snapshot<'a>(&'a Value);

impl fmt::Display for Snapshot<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Value::Scalar(Scalar::Text(text)) => write!(output, "{text:?}"),
            Value::Scalar(scalar) => scalar.fmt(output),
            Value::ConversionError(message) => {
                write!(output, "ConversionError {{ message: {message:?} }}")
            }
            Value::Option(None) => output.write_str("no_bitches"),
            Value::Option(Some(value)) => write!(output, "some({})", Snapshot(value)),
            Value::Result(Ok(value)) => write!(output, "win({})", Snapshot(value)),
            Value::Result(Err(value)) => write!(output, "fumble({})", Snapshot(value)),
            Value::FixedArray(values) => {
                output.write_str("[")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.write_str(", ")?;
                    }
                    Snapshot(value).fmt(output)?;
                }
                output.write_str("]")
            }
            _ => unreachable!("verified SnapshotValue field"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_formats_nested_snapshot_values_without_raw_text_line_breaks() {
        let nested = Value::Option(Some(Box::new(Value::FixedArray(vec![
            Value::Result(Ok(Box::new(Value::Scalar(Scalar::Text(
                "line\nnext".into(),
            ))))),
            Value::Result(Err(Box::new(Value::ConversionError(
                "invalid\tvalue".into(),
            )))),
        ]))));
        assert_eq!(
            format!("{}", Snapshot(&nested)),
            "some([win(\"line\\nnext\"), fumble(ConversionError { message: \"invalid\\tvalue\" })])"
        );
        assert_eq!(format!("{}", Snapshot(&Value::Option(None))), "no_bitches");
    }
}

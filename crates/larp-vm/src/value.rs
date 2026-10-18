use crate::heap::Handle;
use larp_types::{Intrinsic, Scalar, Type};

#[derive(Clone, Debug)]
pub(crate) enum Value {
    Scalar(Scalar),
    Option(Option<Box<Value>>),
    Result(Result<Box<Value>, Box<Value>>),
    ConversionError(String),
    Closure(Handle),
    Array(Handle),
    Object(Handle),
    View { entry: usize },
    Role { object: Handle, role: usize },
    FixedArray(Vec<Value>),
}

impl Value {
    pub fn equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Scalar(a), Self::Scalar(b)) => a == b,
            (Self::Option(None), Self::Option(None)) => true,
            (Self::Option(Some(a)), Self::Option(Some(b))) => a.equals(b),
            (Self::Result(Ok(a)), Self::Result(Ok(b)))
            | (Self::Result(Err(a)), Self::Result(Err(b))) => a.equals(b),
            (Self::ConversionError(a), Self::ConversionError(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            (Self::Role { object: a, role: x }, Self::Role { object: b, role: y }) => {
                a == b && x == y
            }
            (Self::FixedArray(a), Self::FixedArray(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.equals(b))
            }
            _ => false,
        }
    }

    pub fn handles(&self, pending: &mut Vec<Handle>) {
        let mut values = vec![self];
        while let Some(value) = values.pop() {
            match value {
                Self::Closure(handle) | Self::Array(handle) | Self::Object(handle) => {
                    pending.push(*handle)
                }
                Self::Role { object, .. } => pending.push(*object),
                Self::FixedArray(elements) => values.extend(elements),
                Self::Option(Some(value)) | Self::Result(Ok(value)) | Self::Result(Err(value)) => {
                    values.push(value)
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn intrinsic(
    operation: &Intrinsic,
    mut arguments: Vec<Value>,
) -> Result<Value, (&'static str, String)> {
    let value = match operation {
        Intrinsic::None(_) => Value::Option(None),
        Intrinsic::Some(_) => Value::Option(Some(Box::new(arguments.remove(0)))),
        Intrinsic::Ok(_) => Value::Result(Ok(Box::new(arguments.remove(0)))),
        Intrinsic::Err(_) => Value::Result(Err(Box::new(arguments.remove(0)))),
        Intrinsic::IsSome(_) => Value::Scalar(Scalar::Verdict(matches!(
            arguments[0],
            Value::Option(Some(_))
        ))),
        Intrinsic::IsOk(_) => Value::Scalar(Scalar::Verdict(matches!(
            arguments[0],
            Value::Result(Ok(_))
        ))),
        Intrinsic::Copium(_) => {
            let fallback = arguments.pop().unwrap();
            let Value::Option(option) = arguments.pop().unwrap() else {
                unreachable!("verified option")
            };
            option.map_or(fallback, |value| *value)
        }
        Intrinsic::ParseScore => {
            let Value::Scalar(Scalar::Text(text)) = arguments.remove(0) else {
                unreachable!("verified text")
            };
            let digits = text.strip_prefix('-').unwrap_or(&text);
            let parsed = if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                text.parse::<i32>().ok()
            } else {
                None
            };
            Value::Result(match parsed {
                Some(value) => Ok(Box::new(Value::Scalar(Scalar::Mf(value)))),
                None => Err(Box::new(Value::Scalar(Scalar::Text("invalid mf".into())))),
            })
        }
        Intrinsic::Convert { to, fallible, .. } => {
            let number = match arguments.remove(0) {
                Value::Scalar(Scalar::Mf(value)) => i64::from(value),
                Value::Scalar(Scalar::Unc(value)) => value,
                _ => unreachable!("verified integer"),
            };
            let converted = if *to == Type::Mf {
                i32::try_from(number).ok().map(Scalar::Mf)
            } else {
                Some(Scalar::Unc(number))
            };
            if *fallible {
                Value::Result(match converted {
                    Some(value) => Ok(Box::new(Value::Scalar(value))),
                    None => Err(Box::new(Value::ConversionError(format!(
                        "value does not fit {to}"
                    )))),
                })
            } else {
                Value::Scalar(converted.ok_or_else(|| {
                    (
                        "LARP-R008",
                        format!("Checked conversion failed: value does not fit {to}"),
                    )
                })?)
            }
        }
        Intrinsic::ArrayNew { .. }
        | Intrinsic::Size(_)
        | Intrinsic::CheckIndex(_)
        | Intrinsic::Index(_)
        | Intrinsic::SetIndex(_)
        | Intrinsic::Push(_)
        | Intrinsic::Sorted(_)
        | Intrinsic::Map { .. }
        | Intrinsic::Filter { .. }
        | Intrinsic::ForEach { .. } => unreachable!("heap intrinsic handled by VM"),
        Intrinsic::ConversionMessage => {
            let Value::ConversionError(message) = arguments.remove(0) else {
                unreachable!("verified conversion error")
            };
            Value::Scalar(Scalar::Text(message))
        }
    };
    Ok(value)
}

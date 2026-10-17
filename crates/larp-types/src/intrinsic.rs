use crate::{Effects, Signature, Type};

/// Typed built-in operations; malformed operand types have no signature.
#[derive(Clone, Debug)]
pub enum Intrinsic {
    None(Type),
    Some(Type),
    Ok(Type),
    Err(Type),
    IsSome(Type),
    IsOk(Type),
    Copium(Type),
    ParseScore,
    Convert {
        from: Type,
        to: Type,
        fallible: bool,
    },
    ConversionMessage,
}

impl Intrinsic {
    pub fn signature(&self) -> Option<Signature> {
        let (parameters, result, effects) = match self {
            Self::None(ty @ Type::Baddie(_)) => (vec![], ty.clone(), Effects::NONE),
            Self::Some(ty @ Type::Baddie(payload)) => {
                (vec![payload.as_ref().clone()], ty.clone(), Effects::NONE)
            }
            Self::Ok(ty @ Type::Result(payloads)) => {
                (vec![payloads.0.clone()], ty.clone(), Effects::NONE)
            }
            Self::Err(ty @ Type::Result(payloads)) => {
                (vec![payloads.1.clone()], ty.clone(), Effects::NONE)
            }
            Self::IsSome(ty @ Type::Baddie(_)) | Self::IsOk(ty @ Type::Result(_)) => {
                (vec![ty.clone()], Type::Verdict, Effects::NONE)
            }
            Self::Copium(ty @ Type::Baddie(payload)) => (
                vec![ty.clone(), payload.as_ref().clone()],
                payload.as_ref().clone(),
                Effects::NONE,
            ),
            Self::ParseScore => (
                vec![Type::Text],
                Type::result(Type::Mf, Type::Text),
                Effects::NONE,
            ),
            Self::Convert { from, to, fallible } if from.is_integer() && to.is_integer() => (
                vec![from.clone()],
                if *fallible {
                    Type::result(to.clone(), Type::ConversionError)
                } else {
                    to.clone()
                },
                if *fallible {
                    Effects::NONE
                } else {
                    Effects::ABORT
                },
            ),
            Self::ConversionMessage => (vec![Type::ConversionError], Type::Text, Effects::NONE),
            _ => return None,
        };
        Some(Signature {
            parameters,
            result,
            effects,
        })
    }
}

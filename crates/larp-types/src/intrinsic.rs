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
    ArrayNew {
        ty: Type,
        count: usize,
    },
    Size(Type),
    Index(Type),
    CheckIndex(Type),
    SetIndex(Type),
    Push(Type),
    Sorted(Type),
    Map {
        array: Type,
        callback: Type,
    },
    Filter {
        array: Type,
        callback: Type,
    },
    ForEach {
        array: Type,
        callback: Type,
    },
}

impl Intrinsic {
    pub fn types(&self) -> Vec<&Type> {
        match self {
            Self::None(ty)
            | Self::Some(ty)
            | Self::Ok(ty)
            | Self::Err(ty)
            | Self::IsSome(ty)
            | Self::IsOk(ty)
            | Self::Copium(ty)
            | Self::Size(ty)
            | Self::Index(ty)
            | Self::CheckIndex(ty)
            | Self::SetIndex(ty)
            | Self::Push(ty)
            | Self::Sorted(ty)
            | Self::ArrayNew { ty, .. } => vec![ty],
            Self::Map { array, callback }
            | Self::Filter { array, callback }
            | Self::ForEach { array, callback } => vec![array, callback],
            Self::Convert { from, to, .. } => vec![from, to],
            _ => vec![],
        }
    }

    pub fn signature(&self) -> Option<Signature> {
        if self.types().iter().any(|ty| !ty.is_well_formed()) {
            return None;
        }
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
            Self::ArrayNew { ty, count }
                if *count <= 65_536
                    && ty.element().is_some()
                    && !matches!(ty, Type::FixedArray(_, length) if length != count) =>
            {
                (
                    vec![ty.element()?.clone(); *count],
                    ty.clone(),
                    Effects::NONE,
                )
            }
            Self::Size(ty @ Type::Array(_)) => (vec![ty.clone()], Type::Mf, Effects::READ),
            Self::Size(ty @ Type::FixedArray(_, _)) => (vec![ty.clone()], Type::Mf, Effects::NONE),
            Self::Size(Type::Text) => (vec![Type::Text], Type::Mf, Effects::ABORT),
            Self::Index(ty) if ty.element().is_some() => (
                vec![ty.clone(), Type::Mf],
                ty.element()?.clone(),
                if matches!(ty, Type::Array(_)) {
                    Effects::READ.union(Effects::ABORT)
                } else {
                    Effects::ABORT
                },
            ),
            Self::CheckIndex(ty) if ty.element().is_some() => {
                (vec![ty.clone(), Type::Mf], Type::Unit, Effects::ABORT)
            }
            Self::SetIndex(ty) if ty.element().is_some() => (
                vec![ty.clone(), Type::Mf, ty.element()?.clone()],
                if matches!(ty, Type::Array(_)) {
                    Type::Unit
                } else {
                    ty.clone()
                },
                if matches!(ty, Type::Array(_)) {
                    Effects::WRITE.union(Effects::ABORT)
                } else {
                    Effects::ABORT
                },
            ),
            Self::Push(ty @ Type::Array(element)) => (
                vec![ty.clone(), element.as_ref().clone()],
                Type::Unit,
                Effects::WRITE.union(Effects::ABORT),
            ),
            Self::Sorted(ty @ Type::Array(element))
                if element.is_integer() || **element == Type::Text =>
            {
                (vec![ty.clone()], ty.clone(), Effects::READ)
            }
            Self::Map {
                array: array @ Type::Array(element),
                callback: callback @ Type::Function(signature),
            } if signature.parameters.as_slice() == [element.as_ref().clone()]
                && signature.result != Type::Unit =>
            {
                (
                    vec![array.clone(), callback.clone()],
                    Type::array(signature.result.clone()),
                    Effects::READ.union(signature.effects),
                )
            }
            Self::Filter {
                array: array @ Type::Array(element),
                callback: callback @ Type::Function(signature),
            } if signature.parameters.as_slice() == [element.as_ref().clone()]
                && signature.result == Type::Verdict =>
            {
                (
                    vec![array.clone(), callback.clone()],
                    array.clone(),
                    Effects::READ.union(signature.effects),
                )
            }
            Self::ForEach {
                array: array @ Type::Array(element),
                callback: callback @ Type::Function(signature),
            } if signature.parameters.as_slice() == [element.as_ref().clone()]
                && signature.result == Type::Unit =>
            {
                (
                    vec![array.clone(), callback.clone()],
                    Type::Unit,
                    Effects::READ.union(signature.effects),
                )
            }
            _ => return None,
        };
        Some(Signature {
            parameters,
            result,
            effects,
        })
    }
}

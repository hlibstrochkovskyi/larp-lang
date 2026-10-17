//! Scalar types, effects, and checked operations shared across compiler backends.

use std::fmt;
use std::sync::Arc;

mod intrinsic;
pub use intrinsic::Intrinsic;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Type {
    Mf,
    Unc,
    Geeked,
    Verdict,
    Text,
    Unit,
    Function(Arc<Signature>),
    Baddie(Arc<Type>),
    Result(Arc<(Type, Type)>),
    ConversionError,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Signature {
    pub parameters: Vec<Type>,
    pub result: Type,
    pub effects: Effects,
}

impl Type {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "mf" => Self::Mf,
            "unc" => Self::Unc,
            "geeked" => Self::Geeked,
            "verdict" => Self::Verdict,
            "text" => Self::Text,
            "unit" => Self::Unit,
            "ConversionError" => Self::ConversionError,
            _ => return None,
        })
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Mf | Self::Unc | Self::Geeked)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Mf | Self::Unc)
    }

    pub fn function(parameters: Vec<Type>, result: Type, effects: Effects) -> Self {
        Self::Function(Arc::new(Signature {
            parameters,
            result,
            effects,
        }))
    }

    pub fn option(payload: Type) -> Self {
        Self::Baddie(Arc::new(payload))
    }
    pub fn result(success: Type, error: Type) -> Self {
        Self::Result(Arc::new((success, error)))
    }

    pub fn is_printable(&self) -> bool {
        matches!(
            self,
            Self::Mf | Self::Unc | Self::Geeked | Self::Verdict | Self::Text
        )
    }

    pub fn supports_equality(&self) -> bool {
        match self {
            Self::Unit | Self::Function(_) => false,
            Self::Baddie(payload) => payload.supports_equality(),
            Self::Result(payloads) => {
                payloads.0.supports_equality() && payloads.1.supports_equality()
            }
            _ => true,
        }
    }

    /// Bound expanded size as well as depth, including shared type DAGs. This
    /// keeps later structural equality, formatting, and hashing bounded too.
    pub fn within_limits(&self) -> bool {
        let mut pending = vec![(self, 0)];
        let mut visited = 0;
        while let Some((ty, depth)) = pending.pop() {
            visited += 1;
            if depth > 128 || visited > 4096 {
                return false;
            }
            match ty {
                Self::Function(signature) => {
                    if signature.parameters.len() > 4096 {
                        return false;
                    }
                    pending.push((&signature.result, depth + 1));
                    pending.extend(
                        signature
                            .parameters
                            .iter()
                            .map(|parameter| (parameter, depth + 1)),
                    );
                }
                Self::Baddie(payload) => pending.push((payload, depth + 1)),
                Self::Result(payloads) => {
                    pending.push((&payloads.0, depth + 1));
                    pending.push((&payloads.1, depth + 1));
                }
                _ => {}
            }
        }
        true
    }

    pub fn is_well_formed(&self) -> bool {
        if !self.within_limits() {
            return false;
        }
        let mut pending = vec![self];
        while let Some(ty) = pending.pop() {
            match ty {
                Self::Function(signature) => {
                    if signature.parameters.contains(&Self::Unit) {
                        return false;
                    }
                    pending.push(&signature.result);
                    pending.extend(&signature.parameters);
                }
                Self::Baddie(payload) => pending.push(payload),
                Self::Result(payloads) => {
                    pending.push(&payloads.0);
                    pending.push(&payloads.1);
                }
                _ => {}
            }
        }
        true
    }

    /// Only a function's effect bound may widen implicitly; containers are invariant.
    pub fn accepts(&self, found: &Self) -> bool {
        self == found
            || matches!((self, found), (Self::Function(expected), Self::Function(actual))
            if expected.parameters == actual.parameters && expected.result == actual.result && expected.effects.contains(actual.effects))
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function(signature) => {
                f.write_str("fn(")?;
                for (index, parameter) in signature.parameters.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    parameter.fmt(f)?;
                }
                write!(f, ") -> {} {}", signature.result, signature.effects)
            }
            Self::Baddie(payload) => write!(f, "baddie<{payload}>"),
            Self::Result(payloads) => write!(f, "result<{}, {}>", payloads.0, payloads.1),
            Self::ConversionError => f.write_str("ConversionError"),
            _ => f.write_str(match self {
                Self::Mf => "mf",
                Self::Unc => "unc",
                Self::Geeked => "geeked",
                Self::Verdict => "verdict",
                Self::Text => "text",
                Self::Unit => "unit",
                _ => unreachable!(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Mf(i32),
    Unc(i64),
    Geeked(f64),
    Verdict(bool),
    Text(String),
    Unit,
}

impl Scalar {
    pub fn ty(&self) -> Type {
        match self {
            Self::Mf(_) => Type::Mf,
            Self::Unc(_) => Type::Unc,
            Self::Geeked(_) => Type::Geeked,
            Self::Verdict(_) => Type::Verdict,
            Self::Text(_) => Type::Text,
            Self::Unit => Type::Unit,
        }
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mf(value) => value.fmt(f),
            Self::Unc(value) => value.fmt(f),
            Self::Geeked(value) if value.is_nan() => f.write_str("nan"),
            Self::Geeked(value) if *value == f64::INFINITY => f.write_str("inf"),
            Self::Geeked(value) if *value == f64::NEG_INFINITY => f.write_str("-inf"),
            Self::Geeked(value) => value.fmt(f),
            Self::Verdict(value) => f.write_str(if *value { "fr" } else { "cap" }),
            Self::Text(value) => f.write_str(value),
            Self::Unit => f.write_str("unit"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unary {
    Negate,
    Positive,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

pub fn unary_type(op: Unary, ty: &Type) -> Option<Type> {
    match op {
        Unary::Not if *ty == Type::Verdict => Some(ty.clone()),
        Unary::Negate | Unary::Positive if ty.is_numeric() => Some(ty.clone()),
        _ => None,
    }
}

pub fn binary_type(op: Binary, left: &Type, right: &Type) -> Option<Type> {
    use Binary::*;
    if left != right {
        return None;
    }
    match op {
        Add if *left == Type::Text => Some(Type::Text),
        Add | Subtract | Multiply | Divide if left.is_numeric() => Some(left.clone()),
        Remainder if left.is_integer() => Some(left.clone()),
        Equal | NotEqual if left.supports_equality() => Some(Type::Verdict),
        Less | LessEqual | Greater | GreaterEqual if left.is_numeric() => Some(Type::Verdict),
        And | Or if *left == Type::Verdict => Some(Type::Verdict),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticError {
    Overflow,
    DivisionByZero,
    InvalidTypes,
}

pub fn unary(op: Unary, value: &Scalar) -> Result<Scalar, ArithmeticError> {
    use ArithmeticError::*;
    match (op, value) {
        (Unary::Positive, value) if value.ty().is_numeric() => Ok(value.clone()),
        (Unary::Negate, Scalar::Mf(value)) => value.checked_neg().map(Scalar::Mf).ok_or(Overflow),
        (Unary::Negate, Scalar::Unc(value)) => value.checked_neg().map(Scalar::Unc).ok_or(Overflow),
        (Unary::Negate, Scalar::Geeked(value)) => Ok(Scalar::Geeked(-value)),
        (Unary::Not, Scalar::Verdict(value)) => Ok(Scalar::Verdict(!value)),
        _ => Err(InvalidTypes),
    }
}

pub fn binary(op: Binary, left: &Scalar, right: &Scalar) -> Result<Scalar, ArithmeticError> {
    use ArithmeticError::*;
    use Binary::*;
    if binary_type(op, &left.ty(), &right.ty()).is_none() {
        return Err(InvalidTypes);
    }
    macro_rules! numeric {
        ($a:expr, $b:expr, $variant:ident) => {{
            let (a, b) = (*$a, *$b);
            let value = match op {
                Add => a.checked_add(b),
                Subtract => a.checked_sub(b),
                Multiply => a.checked_mul(b),
                Divide | Remainder if b == 0 => return Err(DivisionByZero),
                Divide => a.checked_div(b),
                Remainder if b == -1 => Some(0),
                Remainder => a.checked_rem(b),
                Equal => return Ok(Scalar::Verdict(a == b)),
                NotEqual => return Ok(Scalar::Verdict(a != b)),
                Less => return Ok(Scalar::Verdict(a < b)),
                LessEqual => return Ok(Scalar::Verdict(a <= b)),
                Greater => return Ok(Scalar::Verdict(a > b)),
                GreaterEqual => return Ok(Scalar::Verdict(a >= b)),
                _ => return Err(InvalidTypes),
            };
            value.map(Scalar::$variant).ok_or(Overflow)
        }};
    }
    match (left, right) {
        (Scalar::Mf(a), Scalar::Mf(b)) => numeric!(a, b, Mf),
        (Scalar::Unc(a), Scalar::Unc(b)) => numeric!(a, b, Unc),
        (Scalar::Geeked(a), Scalar::Geeked(b)) => Ok(match op {
            Add => Scalar::Geeked(a + b),
            Subtract => Scalar::Geeked(a - b),
            Multiply => Scalar::Geeked(a * b),
            Divide => Scalar::Geeked(a / b),
            Equal => Scalar::Verdict(a == b),
            NotEqual => Scalar::Verdict(a != b),
            Less => Scalar::Verdict(a < b),
            LessEqual => Scalar::Verdict(a <= b),
            Greater => Scalar::Verdict(a > b),
            GreaterEqual => Scalar::Verdict(a >= b),
            _ => return Err(InvalidTypes),
        }),
        (Scalar::Verdict(a), Scalar::Verdict(b)) => Ok(Scalar::Verdict(match op {
            Equal => a == b,
            NotEqual => a != b,
            And => *a && *b,
            Or => *a || *b,
            _ => return Err(InvalidTypes),
        })),
        (Scalar::Text(a), Scalar::Text(b)) => Ok(match op {
            Add => Scalar::Text(format!("{a}{b}")),
            Equal => Scalar::Verdict(a == b),
            NotEqual => Scalar::Verdict(a != b),
            _ => return Err(InvalidTypes),
        }),
        _ => Err(InvalidTypes),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Effects(u8);

impl Effects {
    pub const NONE: Self = Self(0);
    pub const READ: Self = Self(1);
    pub const WRITE: Self = Self(2);
    pub const OBSERVE: Self = Self(4);
    pub const IO: Self = Self(8);
    pub const ABORT: Self = Self(16);
    pub const ALL: Self = Self(31);

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "read" => Self::READ,
            "write" => Self::WRITE,
            "observe" => Self::OBSERVE,
            "io" => Self::IO,
            "abort" => Self::ABORT,
            _ => return None,
        })
    }

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl fmt::Display for Effects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = ["read", "write", "observe", "io", "abort"]
            .into_iter()
            .filter(|name| self.contains(Self::from_name(name).unwrap()))
            .collect::<Vec<_>>();
        write!(f, "uses({})", names.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_edges_are_checked_without_host_panics() {
        for (a, b) in [
            (Scalar::Mf(i32::MIN), Scalar::Mf(-1)),
            (Scalar::Unc(i64::MIN), Scalar::Unc(-1)),
        ] {
            assert_eq!(
                binary(Binary::Divide, &a, &b),
                Err(ArithmeticError::Overflow)
            );
            assert_eq!(binary(Binary::Remainder, &a, &b).unwrap().to_string(), "0");
        }
        assert_eq!(
            binary(Binary::Divide, &Scalar::Mf(1), &Scalar::Mf(0)),
            Err(ArithmeticError::DivisionByZero)
        );
        assert_eq!(
            unary(Unary::Negate, &Scalar::Unc(i64::MIN)),
            Err(ArithmeticError::Overflow)
        );
        assert_eq!(
            binary(Binary::Add, &Scalar::Mf(1), &Scalar::Unc(1)),
            Err(ArithmeticError::InvalidTypes)
        );
    }

    #[test]
    fn floating_point_and_text_preserve_language_semantics() {
        let nan = Scalar::Geeked(f64::NAN);
        assert_eq!(
            binary(Binary::Equal, &nan, &nan),
            Ok(Scalar::Verdict(false))
        );
        assert_eq!(nan.to_string(), "nan");
        assert_eq!(Scalar::Geeked(f64::NEG_INFINITY).to_string(), "-inf");
        assert_eq!(
            binary(
                Binary::Add,
                &Scalar::Text("a".into()),
                &Scalar::Text("🦀".into())
            ),
            Ok(Scalar::Text("a🦀".into()))
        );
    }
}

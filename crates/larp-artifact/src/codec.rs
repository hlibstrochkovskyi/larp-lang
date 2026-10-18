//! Version 1 explicit wire tags for instructions and their operands.
//!
//! Every tag below is an unsigned byte. Indices and vector/string lengths use
//! checked u32 values through Reader/Writer; integer payloads use their stated
//! widths. Floating-point payloads retain all 64 IEEE bits, including NaN
//! payloads and negative zero. Types are indices into the enclosing type table.
//!
//! Scalar: 0 Mf(i32), 1 Unc(i64), 2 Geeked(u64 bits), 3 Verdict(bool),
//! 4 Text(string), 5 Unit.
//! Unary: 0 Negate, 1 Positive, 2 Not.
//! Binary: 0 Add, 1 Subtract, 2 Multiply, 3 Divide, 4 Remainder, 5 Equal,
//! 6 NotEqual, 7 Less, 8 LessEqual, 9 Greater, 10 GreaterEqual, 11 And, 12 Or.
//! Intrinsic: 0 None, 1 Some, 2 Ok, 3 Err, 4 IsSome, 5 IsOk, 6 Copium,
//! 7 ParseScore, 8 Convert, 9 ConversionMessage, 10 ArrayNew, 11 Size,
//! 12 Index, 13 CheckIndex, 14 SetIndex, 15 Push, 16 Sorted, 17 Map,
//! 18 Filter, 19 ForEach. Operand order follows the explicit matches below.
//! Op: 0 Constant, 1 Load, 2 Store, 3 Unary, 4 Binary, 5 Jump,
//! 6 JumpIfFalse, 7 Call, 8 MakeClosure, 9 Construct, 10 NewObject,
//! 11 GetField, 12 SetField, 13 CallIndirect, 14 Intrinsic, 15 MatchResult,
//! 16 Return, 17 Print, 18 Assert, 19 Crash, 20 IntoRole, 21 CallRole,
//! 22 Scenario, 23 Canon, 24 Flex, 25 Palantir, 26 MakeLazy, 27 Force,
//! 28 SilenceEnter, 29 SilenceExit.
//!
//! These tables are wire contracts, independent of Rust enum discriminants.

use super::{Reader, Result, TypeTable, Writer};
use larp_bytecode::Op;
use larp_types::{Binary, Intrinsic, Scalar, Type, Unary};

pub(super) fn collect_op_types(table: &mut TypeTable, op: &Op) -> Result<()> {
    match op {
        Op::Unary(_, ty)
        | Op::Binary(_, ty)
        | Op::CallIndirect(ty)
        | Op::Force(ty)
        | Op::MatchResult { ty, .. } => {
            table.intern(ty)?;
        }
        Op::Intrinsic(operation) => {
            for ty in operation.types() {
                table.intern(ty)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn write_op(w: &mut Writer, types: &TypeTable, op: &Op) -> Result<()> {
    let tag = match op {
        Op::Constant(_) => 0,
        Op::Load(_) => 1,
        Op::Store(_) => 2,
        Op::Unary(..) => 3,
        Op::Binary(..) => 4,
        Op::Jump(_) => 5,
        Op::JumpIfFalse(_) => 6,
        Op::Call(_) => 7,
        Op::MakeClosure(_) => 8,
        Op::Construct(_) => 9,
        Op::NewObject(_) => 10,
        Op::GetField { .. } => 11,
        Op::SetField { .. } => 12,
        Op::CallIndirect(_) => 13,
        Op::Intrinsic(_) => 14,
        Op::MatchResult { .. } => 15,
        Op::Return => 16,
        Op::Print => 17,
        Op::Assert => 18,
        Op::Crash => 19,
        Op::IntoRole { .. } => 20,
        Op::CallRole { .. } => 21,
        Op::Scenario { .. } => 22,
        Op::Canon { .. } => 23,
        Op::Flex { .. } => 24,
        Op::Palantir { .. } => 25,
        Op::MakeLazy(_) => 26,
        Op::Force(_) => 27,
        Op::SilenceEnter => 28,
        Op::SilenceExit => 29,
    };
    w.u8(tag)?;
    match op {
        Op::Constant(value) => write_scalar(w, value),
        Op::Load(index)
        | Op::Store(index)
        | Op::Jump(index)
        | Op::JumpIfFalse(index)
        | Op::Call(index)
        | Op::MakeClosure(index)
        | Op::Construct(index)
        | Op::NewObject(index)
        | Op::MakeLazy(index) => w.index(*index),
        Op::Unary(operation, ty) => {
            write_unary(w, *operation)?;
            w.ty(types, ty)
        }
        Op::Binary(operation, ty) => {
            write_binary(w, *operation)?;
            w.ty(types, ty)
        }
        Op::GetField { class, field }
        | Op::SetField { class, field }
        | Op::Canon { class, field }
        | Op::Flex { class, field } => {
            w.index(*class)?;
            w.index(*field)
        }
        Op::CallIndirect(ty) | Op::Force(ty) => w.ty(types, ty),
        Op::Intrinsic(operation) => write_intrinsic(w, types, operation),
        Op::MatchResult { ty, success, error } => {
            w.ty(types, ty)?;
            w.index(*success)?;
            w.index(*error)
        }
        Op::IntoRole { class, role } => {
            w.index(*class)?;
            w.index(*role)
        }
        Op::CallRole { role, method } => {
            w.index(*role)?;
            w.index(*method)
        }
        Op::Scenario { entry, label } => {
            w.index(*entry)?;
            w.string(label)
        }
        Op::Palantir { class, fields } => {
            w.index(*class)?;
            w.indices(fields)
        }
        Op::Return | Op::Print | Op::Assert | Op::Crash | Op::SilenceEnter | Op::SilenceExit => {
            Ok(())
        }
    }
}

pub(super) fn read_op(r: &mut Reader<'_>, types: &[Type]) -> Result<Op> {
    Ok(match r.u8()? {
        0 => Op::Constant(read_scalar(r)?),
        1 => Op::Load(r.index()?),
        2 => Op::Store(r.index()?),
        3 => Op::Unary(read_unary(r)?, r.ty(types)?),
        4 => Op::Binary(read_binary(r)?, r.ty(types)?),
        5 => Op::Jump(r.index()?),
        6 => Op::JumpIfFalse(r.index()?),
        7 => Op::Call(r.index()?),
        8 => Op::MakeClosure(r.index()?),
        9 => Op::Construct(r.index()?),
        10 => Op::NewObject(r.index()?),
        11 => Op::GetField {
            class: r.index()?,
            field: r.index()?,
        },
        12 => Op::SetField {
            class: r.index()?,
            field: r.index()?,
        },
        13 => Op::CallIndirect(r.ty(types)?),
        14 => Op::Intrinsic(read_intrinsic(r, types)?),
        15 => Op::MatchResult {
            ty: r.ty(types)?,
            success: r.index()?,
            error: r.index()?,
        },
        16 => Op::Return,
        17 => Op::Print,
        18 => Op::Assert,
        19 => Op::Crash,
        20 => Op::IntoRole {
            class: r.index()?,
            role: r.index()?,
        },
        21 => Op::CallRole {
            role: r.index()?,
            method: r.index()?,
        },
        22 => Op::Scenario {
            entry: r.index()?,
            label: r.string()?,
        },
        23 => Op::Canon {
            class: r.index()?,
            field: r.index()?,
        },
        24 => Op::Flex {
            class: r.index()?,
            field: r.index()?,
        },
        25 => Op::Palantir {
            class: r.index()?,
            fields: r.indices(65_536)?,
        },
        26 => Op::MakeLazy(r.index()?),
        27 => Op::Force(r.ty(types)?),
        28 => Op::SilenceEnter,
        29 => Op::SilenceExit,
        _ => return Err(r.invalid("Unknown instruction tag")),
    })
}

fn write_scalar(w: &mut Writer, value: &Scalar) -> Result<()> {
    match value {
        Scalar::Mf(value) => {
            w.u8(0)?;
            w.i32(*value)
        }
        Scalar::Unc(value) => {
            w.u8(1)?;
            w.i64(*value)
        }
        Scalar::Geeked(value) => {
            w.u8(2)?;
            w.u64(value.to_bits())
        }
        Scalar::Verdict(value) => {
            w.u8(3)?;
            w.bool(*value)
        }
        Scalar::Text(value) => {
            w.u8(4)?;
            w.string(value)
        }
        Scalar::Unit => w.u8(5),
    }
}

fn read_scalar(r: &mut Reader<'_>) -> Result<Scalar> {
    Ok(match r.u8()? {
        0 => Scalar::Mf(r.i32()?),
        1 => Scalar::Unc(r.i64()?),
        2 => Scalar::Geeked(f64::from_bits(r.u64()?)),
        3 => Scalar::Verdict(r.bool()?),
        4 => Scalar::Text(r.string()?),
        5 => Scalar::Unit,
        _ => return Err(r.invalid("Unknown scalar tag")),
    })
}

fn write_unary(w: &mut Writer, operation: Unary) -> Result<()> {
    w.u8(match operation {
        Unary::Negate => 0,
        Unary::Positive => 1,
        Unary::Not => 2,
    })
}

fn read_unary(r: &mut Reader<'_>) -> Result<Unary> {
    Ok(match r.u8()? {
        0 => Unary::Negate,
        1 => Unary::Positive,
        2 => Unary::Not,
        _ => return Err(r.invalid("Unknown unary operator tag")),
    })
}

fn write_binary(w: &mut Writer, operation: Binary) -> Result<()> {
    w.u8(match operation {
        Binary::Add => 0,
        Binary::Subtract => 1,
        Binary::Multiply => 2,
        Binary::Divide => 3,
        Binary::Remainder => 4,
        Binary::Equal => 5,
        Binary::NotEqual => 6,
        Binary::Less => 7,
        Binary::LessEqual => 8,
        Binary::Greater => 9,
        Binary::GreaterEqual => 10,
        Binary::And => 11,
        Binary::Or => 12,
    })
}

fn read_binary(r: &mut Reader<'_>) -> Result<Binary> {
    Ok(match r.u8()? {
        0 => Binary::Add,
        1 => Binary::Subtract,
        2 => Binary::Multiply,
        3 => Binary::Divide,
        4 => Binary::Remainder,
        5 => Binary::Equal,
        6 => Binary::NotEqual,
        7 => Binary::Less,
        8 => Binary::LessEqual,
        9 => Binary::Greater,
        10 => Binary::GreaterEqual,
        11 => Binary::And,
        12 => Binary::Or,
        _ => return Err(r.invalid("Unknown binary operator tag")),
    })
}

fn write_intrinsic(w: &mut Writer, types: &TypeTable, operation: &Intrinsic) -> Result<()> {
    let tag = match operation {
        Intrinsic::None(_) => 0,
        Intrinsic::Some(_) => 1,
        Intrinsic::Ok(_) => 2,
        Intrinsic::Err(_) => 3,
        Intrinsic::IsSome(_) => 4,
        Intrinsic::IsOk(_) => 5,
        Intrinsic::Copium(_) => 6,
        Intrinsic::ParseScore => 7,
        Intrinsic::Convert { .. } => 8,
        Intrinsic::ConversionMessage => 9,
        Intrinsic::ArrayNew { .. } => 10,
        Intrinsic::Size(_) => 11,
        Intrinsic::Index(_) => 12,
        Intrinsic::CheckIndex(_) => 13,
        Intrinsic::SetIndex(_) => 14,
        Intrinsic::Push(_) => 15,
        Intrinsic::Sorted(_) => 16,
        Intrinsic::Map { .. } => 17,
        Intrinsic::Filter { .. } => 18,
        Intrinsic::ForEach { .. } => 19,
    };
    w.u8(tag)?;
    match operation {
        Intrinsic::None(ty)
        | Intrinsic::Some(ty)
        | Intrinsic::Ok(ty)
        | Intrinsic::Err(ty)
        | Intrinsic::IsSome(ty)
        | Intrinsic::IsOk(ty)
        | Intrinsic::Copium(ty)
        | Intrinsic::Size(ty)
        | Intrinsic::Index(ty)
        | Intrinsic::CheckIndex(ty)
        | Intrinsic::SetIndex(ty)
        | Intrinsic::Push(ty)
        | Intrinsic::Sorted(ty) => w.ty(types, ty),
        Intrinsic::Convert { from, to, fallible } => {
            w.ty(types, from)?;
            w.ty(types, to)?;
            w.bool(*fallible)
        }
        Intrinsic::ArrayNew { ty, count } => {
            w.ty(types, ty)?;
            w.index(*count)
        }
        Intrinsic::Map { array, callback }
        | Intrinsic::Filter { array, callback }
        | Intrinsic::ForEach { array, callback } => {
            w.ty(types, array)?;
            w.ty(types, callback)
        }
        Intrinsic::ParseScore | Intrinsic::ConversionMessage => Ok(()),
    }
}

fn read_intrinsic(r: &mut Reader<'_>, types: &[Type]) -> Result<Intrinsic> {
    Ok(match r.u8()? {
        0 => Intrinsic::None(r.ty(types)?),
        1 => Intrinsic::Some(r.ty(types)?),
        2 => Intrinsic::Ok(r.ty(types)?),
        3 => Intrinsic::Err(r.ty(types)?),
        4 => Intrinsic::IsSome(r.ty(types)?),
        5 => Intrinsic::IsOk(r.ty(types)?),
        6 => Intrinsic::Copium(r.ty(types)?),
        7 => Intrinsic::ParseScore,
        8 => Intrinsic::Convert {
            from: r.ty(types)?,
            to: r.ty(types)?,
            fallible: r.bool()?,
        },
        9 => Intrinsic::ConversionMessage,
        10 => Intrinsic::ArrayNew {
            ty: r.ty(types)?,
            count: r.index()?,
        },
        11 => Intrinsic::Size(r.ty(types)?),
        12 => Intrinsic::Index(r.ty(types)?),
        13 => Intrinsic::CheckIndex(r.ty(types)?),
        14 => Intrinsic::SetIndex(r.ty(types)?),
        15 => Intrinsic::Push(r.ty(types)?),
        16 => Intrinsic::Sorted(r.ty(types)?),
        17 => Intrinsic::Map {
            array: r.ty(types)?,
            callback: r.ty(types)?,
        },
        18 => Intrinsic::Filter {
            array: r.ty(types)?,
            callback: r.ty(types)?,
        },
        19 => Intrinsic::ForEach {
            array: r.ty(types)?,
            callback: r.ty(types)?,
        },
        _ => return Err(r.invalid("Unknown intrinsic tag")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use larp_types::Effects;

    fn type_table(op: &Op) -> TypeTable {
        let mut table = TypeTable::default();
        collect_op_types(&mut table, op).unwrap();
        table
    }

    fn encode(op: &Op, types: &TypeTable) -> Vec<u8> {
        let mut writer = Writer::new();
        write_op(&mut writer, types, op).unwrap();
        writer.bytes
    }

    fn roundtrip(op: &Op) -> (Vec<u8>, Op) {
        let types = type_table(op);
        let bytes = encode(op, &types);
        let mut reader = Reader::new(&bytes);
        let decoded = read_op(&mut reader, &types.types).unwrap();
        reader.finish().unwrap();
        assert_eq!(format!("{decoded:?}"), format!("{op:?}"));
        assert_eq!(encode(&decoded, &types), bytes);
        (bytes, decoded)
    }

    fn operations() -> Vec<Op> {
        vec![
            Op::Constant(Scalar::Mf(49)),
            Op::Load(2),
            Op::Store(3),
            Op::Unary(Unary::Negate, Type::Mf),
            Op::Binary(Binary::Add, Type::Unc),
            Op::Jump(5),
            Op::JumpIfFalse(6),
            Op::Call(7),
            Op::MakeClosure(8),
            Op::Construct(9),
            Op::NewObject(10),
            Op::GetField { class: 1, field: 2 },
            Op::SetField { class: 2, field: 3 },
            Op::CallIndirect(Type::function(vec![Type::Mf], Type::Unc, Effects::ABORT)),
            Op::Intrinsic(Intrinsic::ParseScore),
            Op::MatchResult {
                ty: Type::result(Type::Mf, Type::Text),
                success: 4,
                error: 5,
            },
            Op::Return,
            Op::Print,
            Op::Assert,
            Op::Crash,
            Op::IntoRole { class: 2, role: 3 },
            Op::CallRole { role: 4, method: 5 },
            Op::Scenario {
                entry: 6,
                label: "Trial 🦀".into(),
            },
            Op::Canon { class: 3, field: 4 },
            Op::Flex { class: 4, field: 5 },
            Op::Palantir {
                class: 5,
                fields: vec![1, 3, 7],
            },
            Op::MakeLazy(26),
            Op::Force(Type::lazy(Type::option(Type::Mf), Effects::ABORT)),
            Op::SilenceEnter,
            Op::SilenceExit,
        ]
    }

    fn intrinsics() -> Vec<Intrinsic> {
        let option = Type::option(Type::Mf);
        let result = Type::result(Type::Mf, Type::Text);
        let array = Type::array(Type::Mf);
        vec![
            Intrinsic::None(option.clone()),
            Intrinsic::Some(option.clone()),
            Intrinsic::Ok(result.clone()),
            Intrinsic::Err(result.clone()),
            Intrinsic::IsSome(option.clone()),
            Intrinsic::IsOk(result),
            Intrinsic::Copium(option),
            Intrinsic::ParseScore,
            Intrinsic::Convert {
                from: Type::Unc,
                to: Type::Mf,
                fallible: true,
            },
            Intrinsic::ConversionMessage,
            Intrinsic::ArrayNew {
                ty: Type::fixed_array(Type::Mf, 2),
                count: 2,
            },
            Intrinsic::Size(array.clone()),
            Intrinsic::Index(array.clone()),
            Intrinsic::CheckIndex(array.clone()),
            Intrinsic::SetIndex(array.clone()),
            Intrinsic::Push(array.clone()),
            Intrinsic::Sorted(array.clone()),
            Intrinsic::Map {
                array: array.clone(),
                callback: Type::function(vec![Type::Mf], Type::Unc, Effects::ABORT),
            },
            Intrinsic::Filter {
                array: array.clone(),
                callback: Type::function(vec![Type::Mf], Type::Verdict, Effects::NONE),
            },
            Intrinsic::ForEach {
                array,
                callback: Type::function(vec![Type::Mf], Type::Unit, Effects::OBSERVE),
            },
        ]
    }

    #[test]
    fn every_instruction_tag_and_operand_roundtrips_and_every_truncation_fails() {
        for (tag, op) in operations().iter().enumerate() {
            let (bytes, _) = roundtrip(op);
            assert_eq!(bytes[0], tag as u8);
            let types = type_table(op);
            for end in 0..bytes.len() {
                assert!(
                    read_op(&mut Reader::new(&bytes[..end]), &types.types).is_err(),
                    "tag {tag}, prefix {end}"
                );
            }
        }
    }

    #[test]
    fn every_intrinsic_tag_and_payload_roundtrips_and_every_truncation_fails() {
        for (tag, operation) in intrinsics().into_iter().enumerate() {
            let op = Op::Intrinsic(operation);
            let (bytes, _) = roundtrip(&op);
            assert_eq!(bytes[..2], [14, tag as u8]);
            let types = type_table(&op);
            for end in 0..bytes.len() {
                assert!(
                    read_op(&mut Reader::new(&bytes[..end]), &types.types).is_err(),
                    "intrinsic {tag}, prefix {end}"
                );
            }
        }
    }

    #[test]
    fn scalar_tags_preserve_widths_utf8_and_all_float_bits() {
        let scalars = [
            Scalar::Mf(i32::MIN),
            Scalar::Unc(i64::MAX),
            Scalar::Geeked(-0.0),
            Scalar::Verdict(true),
            Scalar::Text("🦀\0\r\n\"".into()),
            Scalar::Unit,
        ];
        for (tag, scalar) in scalars.into_iter().enumerate() {
            let (bytes, _) = roundtrip(&Op::Constant(scalar));
            assert_eq!(bytes[..2], [0, tag as u8]);
        }
        for bits in [
            0_u64,
            1,
            0x8000_0000_0000_0000,
            f64::INFINITY.to_bits(),
            f64::NEG_INFINITY.to_bits(),
            0x7ff8_0000_0000_0049,
            0x7ff0_0000_0000_0001,
            0xfff8_1234_5678_abcd,
        ] {
            let (bytes, decoded) = roundtrip(&Op::Constant(Scalar::Geeked(f64::from_bits(bits))));
            let Op::Constant(Scalar::Geeked(value)) = decoded else {
                panic!("Expected a float constant");
            };
            assert_eq!(value.to_bits(), bits);
            assert_eq!(bytes[2..], bits.to_le_bytes());
        }
        let (bytes, _) = roundtrip(&Op::Constant(Scalar::Mf(i32::MIN)));
        assert_eq!(bytes, [0, 0, 0, 0, 0, 128]);
        let (bytes, _) = roundtrip(&Op::Constant(Scalar::Unc(i64::MIN)));
        assert_eq!(bytes[2..], i64::MIN.to_le_bytes());
    }

    #[test]
    fn unary_and_binary_operator_tags_are_explicit_and_stable() {
        for (tag, operation) in [Unary::Negate, Unary::Positive, Unary::Not]
            .into_iter()
            .enumerate()
        {
            let ty = if operation == Unary::Not {
                Type::Verdict
            } else {
                Type::Mf
            };
            let (bytes, _) = roundtrip(&Op::Unary(operation, ty));
            assert_eq!(bytes[..2], [3, tag as u8]);
        }
        for (tag, operation) in [
            Binary::Add,
            Binary::Subtract,
            Binary::Multiply,
            Binary::Divide,
            Binary::Remainder,
            Binary::Equal,
            Binary::NotEqual,
            Binary::Less,
            Binary::LessEqual,
            Binary::Greater,
            Binary::GreaterEqual,
            Binary::And,
            Binary::Or,
        ]
        .into_iter()
        .enumerate()
        {
            let ty = if matches!(operation, Binary::And | Binary::Or) {
                Type::Verdict
            } else {
                Type::Mf
            };
            let (bytes, _) = roundtrip(&Op::Binary(operation, ty));
            assert_eq!(bytes[..2], [4, tag as u8]);
        }
    }

    #[test]
    fn unknown_tags_invalid_booleans_invalid_utf8_and_missing_type_ids_fail() {
        for bytes in [
            vec![255],
            vec![0, 255],
            vec![3, 255],
            vec![4, 255],
            vec![14, 255],
            vec![0, 3, 2],
            vec![0, 4, 1, 0, 0, 0, 255],
            vec![27, 255, 255, 255, 255],
        ] {
            assert!(read_op(&mut Reader::new(&bytes), &[]).is_err());
        }
        // Convert stores two type IDs followed by a strict Boolean.
        assert!(
            read_op(
                &mut Reader::new(&[14, 8, 0, 0, 0, 0, 0, 0, 0, 0, 2]),
                &[Type::Mf]
            )
            .is_err()
        );
        for (tag, count) in [(25_u8, 65_537_u32), (25, u32::MAX)] {
            let mut bytes = vec![tag];
            bytes.extend(0_u32.to_le_bytes());
            bytes.extend(count.to_le_bytes());
            assert!(read_op(&mut Reader::new(&bytes), &[]).is_err());
        }
        assert!(read_op(&mut Reader::new(&[0, 4, 255, 255, 255, 255]), &[]).is_err());
    }

    #[test]
    fn wide_indices_and_array_counts_are_decoded_without_operand_count_allocation() {
        let (bytes, _) = roundtrip(&Op::GetField {
            class: 0x0102_0304,
            field: u32::MAX as usize,
        });
        assert_eq!(bytes, [11, 4, 3, 2, 1, 255, 255, 255, 255]);
        let op = Op::Intrinsic(Intrinsic::ArrayNew {
            ty: Type::array(Type::Mf),
            count: u32::MAX as usize,
        });
        let (_, decoded) = roundtrip(&op);
        assert!(
            matches!(decoded, Op::Intrinsic(Intrinsic::ArrayNew { count, .. }) if count == u32::MAX as usize)
        );
        if usize::BITS > 32 {
            let types = TypeTable::default();
            let mut writer = Writer::new();
            assert!(write_op(&mut writer, &types, &Op::Load(usize::MAX)).is_err());
        }
    }

    #[test]
    fn every_embedded_type_is_interned_before_encoding() {
        for op in operations()
            .into_iter()
            .chain(intrinsics().into_iter().map(Op::Intrinsic))
        {
            let mut table = type_table(&op);
            let before = table.types.len();
            collect_op_types(&mut table, &op).unwrap();
            assert_eq!(table.types.len(), before);
            let bytes = encode(&op, &table);
            read_op(&mut Reader::new(&bytes), &table.types).unwrap();
        }
    }
}

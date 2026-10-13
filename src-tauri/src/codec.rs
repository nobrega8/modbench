//! Conversion between raw Modbus registers and typed values.
//!
//! Byte order follows the `.mbmap` spec (§6): `A` is the most significant
//! byte, and each order combines a word order with a byte order inside each
//! 16-bit register.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Bool,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float32,
    Int64,
    Uint64,
    Float64,
    String,
}

impl DataType {
    /// Registers taken by one value. `None` for `String` (it needs an explicit
    /// length) and `Some(0)` for `Bool` (a bit, not a whole register).
    pub fn register_count(self) -> Option<u16> {
        match self {
            DataType::Bool => Some(0),
            DataType::Int16 | DataType::Uint16 => Some(1),
            DataType::Int32 | DataType::Uint32 | DataType::Float32 => Some(2),
            DataType::Int64 | DataType::Uint64 | DataType::Float64 => Some(4),
            DataType::String => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ByteOrder {
    /// Most significant register first, high byte first (Modbus standard).
    #[default]
    ABCD,
    /// Least significant register first, high byte first ("word swap").
    CDAB,
    /// Most significant register first, low byte first ("byte swap").
    BADC,
    /// Least significant register first, low byte first (little-endian).
    DCBA,
}

impl ByteOrder {
    fn low_word_first(self) -> bool {
        matches!(self, ByteOrder::CDAB | ByteOrder::DCBA)
    }

    fn swap_bytes(self) -> bool {
        matches!(self, ByteOrder::BADC | ByteOrder::DCBA)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Text(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum CodecError {
    #[error("expected {expected} register(s), got {got}")]
    WrongLength { expected: usize, got: usize },
    #[error("bit {0} is out of range (0-15)")]
    BitOutOfRange(u8),
    #[error("bool in a register needs a bit index")]
    MissingBit,
    #[error("value {value:?} does not fit in {ty:?}")]
    OutOfRange { value: Value, ty: DataType },
    #[error("value {value:?} is not valid for {ty:?}")]
    WrongValue { value: Value, ty: DataType },
    #[error("string does not fit in {0} register(s)")]
    StringTooLong(usize),
    #[error("only ASCII characters can be written to a string register")]
    NonAscii,
}

/// Puts registers in most-significant-first order with high bytes first, so
/// they can be read as one big-endian byte sequence.
fn normalize(regs: &[u16], order: ByteOrder) -> Vec<u8> {
    let mut words: Vec<u16> = regs.to_vec();
    if order.low_word_first() {
        words.reverse();
    }
    words
        .iter()
        .flat_map(|w| {
            let w = if order.swap_bytes() {
                w.swap_bytes()
            } else {
                *w
            };
            w.to_be_bytes()
        })
        .collect()
}

/// Inverse of [`normalize`]: big-endian bytes back to registers in wire order.
fn denormalize(bytes: &[u8], order: ByteOrder) -> Vec<u16> {
    let mut words: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| {
            let w = u16::from_be_bytes(*b);
            if order.swap_bytes() {
                w.swap_bytes()
            } else {
                w
            }
        })
        .collect();
    if order.low_word_first() {
        words.reverse();
    }
    words
}

fn check_len(regs: &[u16], expected: usize) -> Result<(), CodecError> {
    if regs.len() == expected {
        Ok(())
    } else {
        Err(CodecError::WrongLength {
            expected,
            got: regs.len(),
        })
    }
}

/// Decodes registers into a value. `bit` selects one bit of the first
/// register for `Bool`. Bits read from coils/discrete inputs do not go through
/// here; they are already booleans.
pub fn decode(
    regs: &[u16],
    ty: DataType,
    order: ByteOrder,
    bit: Option<u8>,
) -> Result<Value, CodecError> {
    match ty {
        DataType::Bool => {
            let bit = bit.ok_or(CodecError::MissingBit)?;
            if bit > 15 {
                return Err(CodecError::BitOutOfRange(bit));
            }
            check_len(regs, 1)?;
            Ok(Value::Bool(regs[0] >> bit & 1 == 1))
        }
        DataType::String => {
            // Only the byte part of the order applies to strings.
            let bytes: Vec<u8> = regs
                .iter()
                .flat_map(|w| {
                    let w = if order.swap_bytes() {
                        w.swap_bytes()
                    } else {
                        *w
                    };
                    w.to_be_bytes()
                })
                .collect();
            let text: String = bytes.iter().map(|&b| b as char).collect();
            Ok(Value::Text(text.trim_end_matches(['\0', ' ']).to_string()))
        }
        _ => {
            let count = ty.register_count().unwrap_or(0) as usize;
            check_len(regs, count)?;
            let b = normalize(regs, order);
            Ok(match ty {
                DataType::Int16 => Value::Int(i16::from_be_bytes([b[0], b[1]]).into()),
                DataType::Uint16 => Value::Uint(u16::from_be_bytes([b[0], b[1]]).into()),
                DataType::Int32 => {
                    Value::Int(i32::from_be_bytes(b[..4].try_into().unwrap()).into())
                }
                DataType::Uint32 => {
                    Value::Uint(u32::from_be_bytes(b[..4].try_into().unwrap()).into())
                }
                DataType::Float32 => {
                    Value::Float(f32::from_be_bytes(b[..4].try_into().unwrap()).into())
                }
                DataType::Int64 => Value::Int(i64::from_be_bytes(b[..8].try_into().unwrap())),
                DataType::Uint64 => Value::Uint(u64::from_be_bytes(b[..8].try_into().unwrap())),
                DataType::Float64 => Value::Float(f64::from_be_bytes(b[..8].try_into().unwrap())),
                DataType::Bool | DataType::String => unreachable!(),
            })
        }
    }
}

fn as_i128(value: &Value, ty: DataType) -> Result<i128, CodecError> {
    match value {
        Value::Int(v) => Ok(*v as i128),
        Value::Uint(v) => Ok(*v as i128),
        Value::Float(f) if f.is_finite() => Ok(f.round() as i128),
        _ => Err(CodecError::WrongValue {
            value: value.clone(),
            ty,
        }),
    }
}

fn as_f64(value: &Value, ty: DataType) -> Result<f64, CodecError> {
    match value {
        Value::Int(v) => Ok(*v as f64),
        Value::Uint(v) => Ok(*v as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(CodecError::WrongValue {
            value: value.clone(),
            ty,
        }),
    }
}

fn int_bytes(value: &Value, ty: DataType, min: i128, max: i128) -> Result<i128, CodecError> {
    let v = as_i128(value, ty)?;
    if v < min || v > max {
        return Err(CodecError::OutOfRange {
            value: value.clone(),
            ty,
        });
    }
    Ok(v)
}

/// Encodes a value into registers in wire order. `length` is the register
/// count for `String`; it is ignored for other types. `Bool` in a register is
/// written through a read-modify-write by the caller, so it is not handled
/// here.
pub fn encode(
    value: &Value,
    ty: DataType,
    order: ByteOrder,
    length: Option<u16>,
) -> Result<Vec<u16>, CodecError> {
    let bytes: Vec<u8> = match ty {
        DataType::Int16 => (int_bytes(value, ty, i16::MIN.into(), i16::MAX.into())? as i16)
            .to_be_bytes()
            .to_vec(),
        DataType::Uint16 => (int_bytes(value, ty, 0, u16::MAX.into())? as u16)
            .to_be_bytes()
            .to_vec(),
        DataType::Int32 => (int_bytes(value, ty, i32::MIN.into(), i32::MAX.into())? as i32)
            .to_be_bytes()
            .to_vec(),
        DataType::Uint32 => (int_bytes(value, ty, 0, u32::MAX.into())? as u32)
            .to_be_bytes()
            .to_vec(),
        DataType::Int64 => (int_bytes(value, ty, i64::MIN.into(), i64::MAX.into())? as i64)
            .to_be_bytes()
            .to_vec(),
        DataType::Uint64 => (int_bytes(value, ty, 0, u64::MAX.into())? as u64)
            .to_be_bytes()
            .to_vec(),
        DataType::Float32 => (as_f64(value, ty)? as f32).to_be_bytes().to_vec(),
        DataType::Float64 => as_f64(value, ty)?.to_be_bytes().to_vec(),
        DataType::String => {
            let Value::Text(text) = value else {
                return Err(CodecError::WrongValue {
                    value: value.clone(),
                    ty,
                });
            };
            if !text.is_ascii() {
                return Err(CodecError::NonAscii);
            }
            let regs = length.unwrap_or(0) as usize;
            if text.len() > regs * 2 {
                return Err(CodecError::StringTooLong(regs));
            }
            let mut bytes = text.as_bytes().to_vec();
            bytes.resize(regs * 2, 0);
            // Strings keep their register order; only the byte order applies.
            return Ok(bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| {
                    let w = u16::from_be_bytes(*b);
                    if order.swap_bytes() {
                        w.swap_bytes()
                    } else {
                        w
                    }
                })
                .collect());
        }
        DataType::Bool => {
            return Err(CodecError::WrongValue {
                value: value.clone(),
                ty,
            });
        }
    };
    Ok(denormalize(&bytes, order))
}

/// Mask covering a bit field of `bits` bits starting at `bit`.
pub fn field_mask(bit: u8, bits: u8) -> u16 {
    (((1u32 << bits) - 1) << bit) as u16
}

/// Value of a bit field inside a register.
pub fn extract_field(word: u16, bit: u8, bits: u8) -> u16 {
    (word & field_mask(bit, bits)) >> bit
}

/// `word` with the bit field replaced by `value` (extra high bits dropped).
pub fn insert_field(word: u16, bit: u8, bits: u8, value: u16) -> u16 {
    let mask = field_mask(bit, bits);
    (word & !mask) | ((value << bit) & mask)
}

/// Raw value to engineering value: `raw * scale + offset`.
pub fn to_engineering(value: Value, scale: f64, offset: f64) -> Value {
    if scale == 1.0 && offset == 0.0 {
        return value;
    }
    match value {
        Value::Int(v) => Value::Float(v as f64 * scale + offset),
        Value::Uint(v) => Value::Float(v as f64 * scale + offset),
        Value::Float(v) => Value::Float(v * scale + offset),
        other => other,
    }
}

/// Engineering value back to raw: `(value - offset) / scale`. Integer types
/// are rounded later by [`encode`].
pub fn from_engineering(value: Value, scale: f64, offset: f64) -> Value {
    if scale == 1.0 && offset == 0.0 {
        return value;
    }
    match value {
        Value::Int(v) => Value::Float((v as f64 - offset) / scale),
        Value::Uint(v) => Value::Float((v as f64 - offset) / scale),
        Value::Float(v) => Value::Float((v - offset) / scale),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ByteOrder::*;

    const ORDERS: [ByteOrder; 4] = [ABCD, CDAB, BADC, DCBA];

    #[test]
    fn float32_in_all_byte_orders() {
        // 123.456f32 = 0x42F6E979 → bytes A=42 B=F6 C=E9 D=79
        let cases = [
            (ABCD, [0x42F6, 0xE979]),
            (CDAB, [0xE979, 0x42F6]),
            (BADC, [0xF642, 0x79E9]),
            (DCBA, [0x79E9, 0xF642]),
        ];
        for (order, regs) in cases {
            let Value::Float(v) = decode(&regs, DataType::Float32, order, None).unwrap() else {
                panic!("not a float");
            };
            assert!((v - 123.456).abs() < 1e-4, "{order:?}: {v}");
            assert_eq!(
                encode(&Value::Float(123.456), DataType::Float32, order, None).unwrap(),
                regs,
                "{order:?}"
            );
        }
    }

    #[test]
    fn int64_word_order() {
        // 0x0102030405060708
        let abcd = [0x0102, 0x0304, 0x0506, 0x0708];
        let cdab = [0x0708, 0x0506, 0x0304, 0x0102];
        let badc = [0x0201, 0x0403, 0x0605, 0x0807];
        let dcba = [0x0807, 0x0605, 0x0403, 0x0201];
        for (order, regs) in [(ABCD, abcd), (CDAB, cdab), (BADC, badc), (DCBA, dcba)] {
            assert_eq!(
                decode(&regs, DataType::Uint64, order, None).unwrap(),
                Value::Uint(0x0102030405060708),
                "{order:?}"
            );
        }
    }

    #[test]
    fn sixteen_bit_types() {
        assert_eq!(
            decode(&[0xFFFE], DataType::Int16, ABCD, None).unwrap(),
            Value::Int(-2)
        );
        assert_eq!(
            decode(&[0xFFFE], DataType::Uint16, ABCD, None).unwrap(),
            Value::Uint(65534)
        );
        // Byte swap applies to 16-bit types, word order does not.
        assert_eq!(
            decode(&[0x3412], DataType::Uint16, BADC, None).unwrap(),
            Value::Uint(0x1234)
        );
        assert_eq!(
            decode(&[0x1234], DataType::Uint16, CDAB, None).unwrap(),
            Value::Uint(0x1234)
        );
    }

    #[test]
    fn round_trip_every_numeric_type_and_order() {
        let samples = [
            (DataType::Int16, Value::Int(-12345)),
            (DataType::Uint16, Value::Uint(54321)),
            (DataType::Int32, Value::Int(-2_000_000_000)),
            (DataType::Uint32, Value::Uint(4_000_000_000)),
            (DataType::Int64, Value::Int(-9_000_000_000_000_000_000)),
            (DataType::Uint64, Value::Uint(18_000_000_000_000_000_000)),
            (DataType::Float32, Value::Float(-0.5)),
            (DataType::Float64, Value::Float(std::f64::consts::PI)),
        ];
        for order in ORDERS {
            for (ty, value) in &samples {
                let regs = encode(value, *ty, order, None).unwrap();
                assert_eq!(regs.len(), ty.register_count().unwrap() as usize);
                assert_eq!(
                    &decode(&regs, *ty, order, None).unwrap(),
                    value,
                    "{ty:?} {order:?}"
                );
            }
        }
    }

    #[test]
    fn bool_bits_in_register() {
        let reg = [0b0000_0000_0000_1000];
        assert_eq!(
            decode(&reg, DataType::Bool, ABCD, Some(3)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            decode(&reg, DataType::Bool, ABCD, Some(4)).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            decode(&reg, DataType::Bool, ABCD, Some(16)),
            Err(CodecError::BitOutOfRange(16))
        );
        assert_eq!(
            decode(&reg, DataType::Bool, ABCD, None),
            Err(CodecError::MissingBit)
        );
    }

    #[test]
    fn strings() {
        // "AB" "CD" "E\0" with high byte first
        let regs = [0x4142, 0x4344, 0x4500, 0x0000];
        assert_eq!(
            decode(&regs, DataType::String, ABCD, None).unwrap(),
            Value::Text("ABCDE".into())
        );
        // Byte-swapped devices put the first character in the low byte.
        let swapped = [0x4241, 0x4443, 0x0045, 0x0000];
        assert_eq!(
            decode(&swapped, DataType::String, BADC, None).unwrap(),
            Value::Text("ABCDE".into())
        );
        // Word order does not reverse strings.
        assert_eq!(
            decode(&regs, DataType::String, CDAB, None).unwrap(),
            Value::Text("ABCDE".into())
        );
        assert_eq!(
            encode(
                &Value::Text("ABCDE".into()),
                DataType::String,
                ABCD,
                Some(4)
            )
            .unwrap(),
            regs
        );
        assert_eq!(
            encode(
                &Value::Text("ABCDE".into()),
                DataType::String,
                ABCD,
                Some(2)
            ),
            Err(CodecError::StringTooLong(2))
        );
        assert_eq!(
            encode(&Value::Text("é".into()), DataType::String, ABCD, Some(2)),
            Err(CodecError::NonAscii)
        );
    }

    #[test]
    fn rejects_wrong_lengths_and_ranges() {
        assert_eq!(
            decode(&[1], DataType::Float32, ABCD, None),
            Err(CodecError::WrongLength {
                expected: 2,
                got: 1
            })
        );
        assert!(matches!(
            encode(&Value::Int(70000), DataType::Uint16, ABCD, None),
            Err(CodecError::OutOfRange { .. })
        ));
        assert!(matches!(
            encode(&Value::Int(-1), DataType::Uint32, ABCD, None),
            Err(CodecError::OutOfRange { .. })
        ));
        assert!(matches!(
            encode(&Value::Text("x".into()), DataType::Int16, ABCD, None),
            Err(CodecError::WrongValue { .. })
        ));
    }

    #[test]
    fn floats_written_to_integer_types_are_rounded() {
        assert_eq!(
            encode(&Value::Float(41.6), DataType::Uint16, ABCD, None).unwrap(),
            [42]
        );
    }

    #[test]
    fn bit_fields() {
        assert_eq!(field_mask(12, 4), 0xF000);
        assert_eq!(field_mask(0, 16), 0xFFFF);
        assert_eq!(extract_field(0x2A51, 12, 4), 0x2);
        assert_eq!(extract_field(0x2A51, 4, 4), 0x5);
        assert_eq!(insert_field(0x2A51, 8, 4, 0x7), 0x2751);
        assert_eq!(insert_field(0x0000, 0, 2, 0xF), 0x0003);
    }

    #[test]
    fn scaling() {
        assert_eq!(
            to_engineering(Value::Uint(12345), 0.01, 0.0),
            Value::Float(123.45)
        );
        assert_eq!(to_engineering(Value::Int(-40), 1.0, 0.0), Value::Int(-40));
        let raw = from_engineering(Value::Float(123.45), 0.01, 0.0);
        assert_eq!(encode(&raw, DataType::Uint16, ABCD, None).unwrap(), [12345]);
        assert_eq!(
            from_engineering(Value::Float(20.0), 0.1, -40.0),
            Value::Float(600.0)
        );
    }
}

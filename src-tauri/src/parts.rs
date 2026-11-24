//! Parts of a block register (spec section 4.10): named values inside one
//! `string` register. The device takes the block whole, so a write is built
//! from every part and sent in one request; a part left empty is sent as zeros.

use crate::codec::{self, ByteOrder, CodecError, Value};
use crate::mbmap::Register;

/// The whole block, built from one value per part (in the order of
/// `register.parts`). A part without a value is zero bytes, so an empty field
/// never keeps an old value by accident; registers no part covers are padding
/// and are zero too.
pub fn build_block(
    register: &Register,
    values: &[Option<Value>],
    default_order: ByteOrder,
) -> Result<Vec<u16>, CodecError> {
    let order = register.byte_order.unwrap_or(default_order);
    if register.parts.iter().any(|p| p.is_bit_field()) {
        return build_word(register, values, order);
    }
    let size = usize::from(register.length.unwrap_or(0));
    let mut block = vec![0u16; size];
    for (part, value) in register.parts.iter().zip(values) {
        let Some(value) = value else { continue };
        // Text is kept to the part's registers: 2 characters each, at most.
        let words = codec::encode(value, part.ty, order, Some(part.length))?;
        block[part.range()].copy_from_slice(&words);
    }
    Ok(block)
}

/// A word from its bit fields: a field without a value is 0, and a value
/// that does not fit its field is refused. The word is encoded as the register
/// is, so its byte and word order apply.
fn build_word(
    register: &Register,
    values: &[Option<Value>],
    order: ByteOrder,
) -> Result<Vec<u16>, CodecError> {
    let mut word: u64 = 0;
    for (part, value) in register.parts.iter().zip(values) {
        let Some(value) = value else { continue };
        let (bit, bits) = (
            u32::from(part.bit.unwrap_or(0)),
            u32::from(part.bits.unwrap_or(1)),
        );
        let raw = match value {
            Value::Bool(b) => u64::from(*b),
            Value::Uint(n) => *n,
            Value::Int(n) if *n >= 0 => *n as u64,
            _ => {
                return Err(CodecError::WrongValue {
                    value: value.clone(),
                    ty: register.ty,
                });
            }
        };
        if bits < 64 && raw >> bits != 0 {
            return Err(CodecError::OutOfRange {
                value: value.clone(),
                ty: register.ty,
            });
        }
        word |= raw << bit;
    }
    codec::encode(&Value::Uint(word), register.ty, order, None)
}

/// The value of every part, decoded from the block as read.
pub fn split_block(
    register: &Register,
    words: &[u16],
    default_order: ByteOrder,
) -> Vec<Result<Value, CodecError>> {
    let order = register.byte_order.unwrap_or(default_order);
    if register.parts.iter().any(|p| p.is_bit_field()) {
        let word = codec::decode(words, register.ty, order, None).and_then(|v| match v {
            Value::Uint(n) => Ok(n),
            other => Err(CodecError::WrongValue {
                value: other,
                ty: register.ty,
            }),
        });
        let word = match word {
            Ok(w) => w,
            Err(_) => {
                return register
                    .parts
                    .iter()
                    .map(|_| {
                        Err(CodecError::WrongLength {
                            expected: usize::from(register.ty.register_count().unwrap_or(1)),
                            got: words.len(),
                        })
                    })
                    .collect();
            }
        };
        return register
            .parts
            .iter()
            .map(|part| {
                let bits = u32::from(part.bits.unwrap_or(1));
                let mask = if bits >= 64 {
                    u64::MAX
                } else {
                    (1u64 << bits) - 1
                };
                Ok(Value::Uint(
                    (word >> u32::from(part.bit.unwrap_or(0))) & mask,
                ))
            })
            .collect();
    }
    register
        .parts
        .iter()
        .map(|part| {
            let slice = words.get(part.range()).ok_or(CodecError::WrongLength {
                expected: part.range().end,
                got: words.len(),
            })?;
            codec::decode(slice, part.ty, order, None)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mbmap;

    const MAP: &str = r#"{
  "mbmap": 2, "vendor": "T", "model": "T",
  "registers": [
    {"name": "Add SMS", "area": "holding", "address": 397, "type": "string", "length": 8, "access": "RW",
     "parts": [
       {"name": "Recipient", "offset": 0, "length": 3},
       {"name": "Port", "offset": 4, "type": "uint16"},
       {"name": "Address", "offset": 5, "type": "uint32"}
     ]}
  ]
}"#;

    fn register() -> Register {
        let parsed = mbmap::parse(MAP.as_bytes());
        parsed.map.expect("valid map").registers.remove(0)
    }

    #[test]
    fn the_block_is_built_whole_with_empty_parts_as_zeros() {
        let reg = register();
        assert_eq!(reg.parts.len(), 3);
        let values = [
            Some(Value::Text("0035".into())),
            None,
            Some(Value::Uint(0x0001_0002)),
        ];
        let block = build_block(&reg, &values, ByteOrder::ABCD).unwrap();
        assert_eq!(block.len(), 8);
        assert_eq!(block[0], u16::from_be_bytes(*b"00"));
        assert_eq!(block[1], u16::from_be_bytes(*b"35"));
        // Padding, the empty Port and the number as usual
        assert_eq!(&block[2..5], &[0, 0, 0]);
        assert_eq!(&block[5..7], &[1, 2]);
        assert_eq!(block[7], 0);
    }

    #[test]
    fn bit_fields_are_built_into_one_word_and_read_back() {
        let map = r#"{"mbmap": 2, "vendor": "T", "model": "T",
          "enums": {"cmd": {"0": "Stop", "1": "Play"}},
          "registers": [{"name": "Control", "area": "holding", "address": 2001,
            "type": "uint32", "access": "RW", "parts": [
              {"name": "Command", "bit": 0, "bits": 4, "enum": "cmd"},
              {"name": "Ack", "bit": 4},
              {"name": "Unit 1", "bit": 21}]}]}"#;
        let reg = mbmap::parse(map.as_bytes())
            .map
            .expect("valid map")
            .registers
            .remove(0);
        assert_eq!(reg.parts[0].enum_labels.len(), 2);
        let values = [Some(Value::Uint(1)), None, Some(Value::Bool(true))];
        let block = build_block(&reg, &values, ByteOrder::ABCD).unwrap();
        assert_eq!(block, vec![0x0020, 0x0001]);
        let back = split_block(&reg, &block, ByteOrder::ABCD);
        assert_eq!(back[0].as_ref().unwrap(), &Value::Uint(1));
        assert_eq!(back[1].as_ref().unwrap(), &Value::Uint(0));
        assert_eq!(back[2].as_ref().unwrap(), &Value::Uint(1));
        let too_big = [Some(Value::Uint(16)), None, None];
        assert!(build_block(&reg, &too_big, ByteOrder::ABCD).is_err());
    }

    #[test]
    fn text_longer_than_the_part_is_refused() {
        let reg = register();
        let values = [Some(Value::Text("1234567".into())), None, None];
        assert!(build_block(&reg, &values, ByteOrder::ABCD).is_err());
    }

    #[test]
    fn parts_are_read_back_from_the_block() {
        let reg = register();
        let values = [
            Some(Value::Text("0035".into())),
            Some(Value::Uint(8080)),
            Some(Value::Uint(7)),
        ];
        let block = build_block(&reg, &values, ByteOrder::ABCD).unwrap();
        let back = split_block(&reg, &block, ByteOrder::ABCD);
        assert_eq!(back[1].as_ref().unwrap(), &Value::Uint(8080));
        assert_eq!(back[2].as_ref().unwrap(), &Value::Uint(7));
        let Value::Text(text) = back[0].as_ref().unwrap() else {
            panic!("text expected");
        };
        assert_eq!(text.trim_end_matches('\0'), "0035");
    }
}

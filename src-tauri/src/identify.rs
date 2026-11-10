//! The identity a device reports with Report Slave ID (FC17) or Read Device
//! Identification (FC43), taken apart as a map's `identify` block says
//! (spec section 4.8).

use crate::mbmap::{DeviceMap, Identify, IdentifyField, IdentifyType, Role};
use crate::modbus::{DeviceIdentity, Session};

/// What a device answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// FC17: the data after the byte count (server id, run indicator, ...).
    ServerId(Vec<u8>),
    /// FC43: the objects, by id.
    Objects(Vec<(u8, String)>),
}

/// ASCII text without trailing NUL and space characters.
fn trimmed(bytes: &[u8]) -> String {
    let text: String = bytes
        .iter()
        .map(|&b| if b.is_ascii() { b as char } else { '?' })
        .collect();
    text.trim_end_matches(['\0', ' ']).to_string()
}

/// The access level (1 basic, 2 regular, 3 extended) that includes every
/// object the fields name.
fn level(identify: &Identify) -> u8 {
    let highest = identify
        .fields
        .iter()
        .filter_map(|f| f.object)
        .max()
        .unwrap_or(0);
    match highest {
        0..=2 => 1,
        3..=0x7F => 2,
        _ => 3,
    }
}

/// Asks the device. `None` when it does not support the function, answers
/// with an exception or does not answer: the check is then unknown.
pub async fn read(session: &mut Session, identify: &Identify) -> Option<Answer> {
    if identify.function == 17 {
        session.read_server_id().await.ok().map(Answer::ServerId)
    } else {
        let DeviceIdentity { objects, .. } = session.read_device_id(level(identify)).await.ok()?;
        Some(Answer::Objects(
            objects.into_iter().map(|o| (o.id, o.value)).collect(),
        ))
    }
}

fn field_value(field: &IdentifyField, answer: &Answer) -> Option<String> {
    match answer {
        Answer::Objects(objects) => {
            let object = field.object?;
            let text = objects.iter().find(|(id, _)| *id == object)?.1.as_bytes();
            Some(trimmed(text)).filter(|t| !t.is_empty())
        }
        Answer::ServerId(data) => {
            let at = field.offset? as usize;
            match field.ty? {
                IdentifyType::String => {
                    let end = at.checked_add(field.length? as usize)?;
                    Some(trimmed(data.get(at..end)?)).filter(|t| !t.is_empty())
                }
                ty => {
                    let number = if ty == IdentifyType::Uint8 {
                        i64::from(*data.get(at)?)
                    } else {
                        let pair = data.get(at..at + 2)?;
                        i64::from(u16::from_be_bytes([pair[0], pair[1]]))
                    };
                    Some(
                        field
                            .enum_labels
                            .iter()
                            .find(|(v, _)| *v == number)
                            .map_or_else(|| number.to_string(), |(_, label)| label.clone()),
                    )
                }
            }
        }
    }
}

/// The value of each field of the block, in order; `None` where the answer
/// has no value for it (too short, object missing).
pub fn values(identify: &Identify, answer: &Answer) -> Vec<Option<String>> {
    identify
        .fields
        .iter()
        .map(|f| field_value(f, answer))
        .collect()
}

/// The fields with a role that gave a value, as role and text.
pub fn roles(identify: &Identify, answer: &Answer) -> Vec<(Role, String)> {
    identify
        .fields
        .iter()
        .zip(values(identify, answer))
        .filter_map(|(f, v)| Some((f.role?, v?)))
        .collect()
}

/// Reads the identity of the device if the map has an `identify` block.
pub async fn read_for(session: &mut Session, map: &DeviceMap) -> Vec<(Role, String)> {
    let Some(identify) = &map.meta.identify else {
        return Vec::new();
    };
    match read(session, identify).await {
        Some(answer) => roles(identify, &answer),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identify(text: &str) -> Identify {
        let map = crate::mbmap::parse(text.as_bytes()).map.unwrap();
        map.meta.identify.unwrap()
    }

    const HEAD: &str = r#""mbmap": 2, "vendor": "T", "model": "M", "registers": []"#;

    #[test]
    fn fc17_fields_are_taken_by_offset() {
        let id = identify(&format!(
            r#"{{{HEAD}, "enums": {{"codes": {{"65": "DMG610"}}}},
              "identify": {{"function": 17, "fields": [
                {{"name": "Type", "offset": 0, "type": "uint8", "role": "model", "enum": "codes"}},
                {{"name": "Rev", "offset": 1, "type": "uint16", "role": "firmware"}},
                {{"name": "Name", "offset": 3, "type": "string", "length": 4, "role": "serial"}},
                {{"name": "Far", "offset": 20, "type": "uint8"}}
              ]}}}}"#
        ));
        let answer = Answer::ServerId(vec![0x41, 0x00, 0x07, b'A', b'B', 0, b' ']);
        assert_eq!(
            values(&id, &answer),
            [
                Some("DMG610".into()),
                Some("7".into()),
                Some("AB".into()),
                None
            ]
        );
        assert_eq!(roles(&id, &answer).len(), 3);
    }

    #[test]
    fn fc43_fields_are_taken_by_object() {
        let id = identify(&format!(
            r#"{{{HEAD}, "identify": {{"function": 43, "fields": [
                {{"name": "Product", "object": 1, "role": "model"}},
                {{"name": "Revision", "object": 2, "role": "firmware"}},
                {{"name": "Model", "object": 5}}
              ]}}}}"#
        ));
        assert_eq!(level(&id), 2);
        let answer = Answer::Objects(vec![(1, "PAC3200\0\0".into()), (2, "V2.4 ".into())]);
        assert_eq!(
            values(&id, &answer),
            [Some("PAC3200".into()), Some("V2.4".into()), None]
        );
    }
}

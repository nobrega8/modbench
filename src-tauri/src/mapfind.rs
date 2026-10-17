//! Finds the maps that fit the connected device, from the identity
//! registers they declare (`role` model and firmware, and `reference`
//! values). Each distinct identity register is read once, whatever the
//! number of maps that declare it.

use std::collections::HashMap;

use serde::Serialize;

use crate::codec::Value;
use crate::firmware;
use crate::mbmap::{DeviceMap, Register, Role};
use crate::modbus::{RawData, Session};
use crate::planner::Area;
use crate::poller::decode_point;

/// A map that fits, with what matched.
#[derive(Debug, Clone, Serialize)]
pub struct MapMatch {
    pub path: String,
    pub vendor: String,
    pub model: String,
    /// Higher is a better fit.
    pub score: u32,
    /// What matched, in words, e.g. `model EM540`, `3 of 3 references`.
    pub matched: Vec<String>,
    /// What does not fit but does not rule the map out, e.g. a firmware
    /// outside the map's range.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapSearch {
    pub matches: Vec<MapMatch>,
    /// Maps with identity registers, and the reads it took to check them.
    pub candidates: usize,
    pub reads: usize,
}

/// The registers of a map that say which device it is for.
fn identity_registers(map: &DeviceMap) -> Vec<&Register> {
    map.registers
        .iter()
        .filter(|r| {
            r.access.readable()
                && (r.reference.is_some() || matches!(r.role, Some(Role::Model | Role::Firmware)))
        })
        .collect()
}

/// A decoded value as text, with its enum label when it has one.
fn text_of(reg: &Register, v: &Value) -> String {
    let n = match v {
        Value::Int(n) => Some(i128::from(*n)),
        Value::Uint(n) => Some(i128::from(*n)),
        _ => None,
    };
    if let Some(n) = n
        && let Some((_, label)) = reg.enum_labels.iter().find(|(e, _)| i128::from(*e) == n)
    {
        return label.clone();
    }
    match v {
        Value::Text(t) => t.trim_end_matches(['\0', ' ']).trim().to_string(),
        Value::Float(f) => f.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Uint(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
    }
}

type Key = (Area, u16, u16);

/// Checks the maps against the device. `maps` are (path, map).
pub async fn find_maps(session: &mut Session, maps: &[(String, DeviceMap)]) -> MapSearch {
    let candidates: Vec<(&String, &DeviceMap, Vec<&Register>)> = maps
        .iter()
        .map(|(p, m)| (p, m, identity_registers(m)))
        .filter(|(_, _, regs)| !regs.is_empty())
        .collect();

    // Read each distinct identity register once.
    let mut cache: HashMap<Key, Option<RawData>> = HashMap::new();
    for (_, _, regs) in &candidates {
        for r in regs {
            let key = (r.area, r.address, r.count());
            if let std::collections::hash_map::Entry::Vacant(slot) = cache.entry(key) {
                slot.insert(session.read(key.0, key.1, key.2).await.ok());
            }
        }
    }

    let mut matches = Vec::new();
    for (path, map, regs) in &candidates {
        let mut score = 0;
        let mut matched = Vec::new();
        let mut warnings = Vec::new();
        let mut refs = (0, 0);
        let mut rejected = false;
        let mut firmware_parts = Vec::new();
        for r in regs {
            let Some(Some(data)) = cache.get(&(r.area, r.address, r.count())) else {
                continue;
            };
            let order = r.byte_order.or(map.meta.byte_order).unwrap_or_default();
            let point = decode_point(r, order, data.clone(), &map.meta.sentinels);
            if point.error.is_some() {
                continue;
            }
            match point.reference {
                Some("match") => refs.0 += 1,
                Some(_) => {
                    refs.1 += 1;
                    rejected = true;
                }
                None => {}
            }
            let Some(value) = &point.value else { continue };
            match r.role {
                Some(Role::Model) => {
                    let found = text_of(r, value);
                    let names: Vec<&String> = if map.meta.models.is_empty() {
                        vec![&map.meta.model]
                    } else {
                        map.meta.models.iter().collect()
                    };
                    if names
                        .iter()
                        .any(|n| n.trim().eq_ignore_ascii_case(found.trim()))
                    {
                        score += 5;
                        matched.push(format!("model {found}"));
                    } else {
                        rejected = true;
                    }
                }
                Some(Role::Firmware) => firmware_parts.push(text_of(r, value)),
                _ => {}
            }
        }
        if rejected {
            continue;
        }
        if refs.0 > 0 {
            score += 2 * refs.0;
            matched.push(if refs.0 == 1 {
                "1 reference".into()
            } else {
                format!("{} references", refs.0)
            });
        }
        if !firmware_parts.is_empty()
            && let Some(version) = firmware::find(&firmware_parts.join("."))
        {
            let range = map.meta.firmware_range();
            let shown = firmware::display(&version);
            if range.is_open() {
                // No range declared: nothing to check
            } else if range.contains(&version) {
                score += 1;
                matched.push(format!("firmware {shown}"));
            } else {
                warnings.push(format!("firmware {shown} is outside the map's range"));
            }
        }
        if score == 0 {
            continue;
        }
        matches.push(MapMatch {
            path: path.to_string(),
            vendor: map.meta.vendor.clone(),
            model: map.meta.model.clone(),
            score,
            matched,
            warnings,
        });
    }
    matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.model.cmp(&b.model)));
    MapSearch {
        matches,
        candidates: candidates.len(),
        reads: cache.len(),
    }
}

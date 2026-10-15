//! Values the simulator changes on its own while it runs, so charts, alarms,
//! recordings and the trend player can be tried without hardware. A
//! generator writes to a raw address, or to a register of the loaded map in
//! engineering units (its type, scale and offset applied).

use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::codec::{self, DataType, Value};
use crate::mbmap::Register;
use crate::modbus::RawData;
use crate::planner::Area;
use crate::simulator::Memory;

/// Shortest update period, in milliseconds.
pub const MIN_EVERY_MS: u64 = 50;
/// How often the running simulator checks which generators are due.
const TICK: Duration = Duration::from_millis(10);

/// Where a generator reads or writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Target {
    /// A register of the loaded map, by name, in engineering units.
    Register { register: String },
    /// One raw address: a bit, or a register as an unsigned 16-bit word.
    /// Where a register of the loaded map starts, that whole register (its
    /// type, scale and offset), as if named.
    Address { area: Area, address: u16 },
}

/// How a generator makes its next value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// Adds `step` each period; past `max` it starts again at `min` (past
    /// `min` at `max` when the step is negative).
    Increment {
        step: f64,
        #[serde(default)]
        min: f64,
        max: f64,
    },
    /// `offset + amplitude * sin(2π t / period_s)`.
    Sine {
        period_s: f64,
        amplitude: f64,
        #[serde(default)]
        offset: f64,
    },
    /// A uniform value from `min` to `max`.
    Random { min: f64, max: f64 },
    /// The value of another address or register, also across areas (a
    /// command reflected in a status).
    Copy { from: Target },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Generator {
    #[serde(flatten)]
    pub target: Target,
    #[serde(flatten)]
    pub kind: Kind,
    /// How often the value changes, in milliseconds.
    #[serde(default = "default_every")]
    pub every_ms: u64,
}

fn default_every() -> u64 {
    1000
}

/// The generators of a simulator, shared with the task that runs them.
pub type Shared = Arc<Mutex<Vec<Generator>>>;

/// What a target resolves to in the memory.
enum Slot<'a> {
    Bit(Area, u16),
    Word(Area, u16),
    Register(&'a Register, codec::ByteOrder),
}

fn resolve<'a>(memory: &'a Memory, target: &Target) -> Result<Slot<'a>, String> {
    match target {
        Target::Address { area, address } if area.is_bit() => Ok(Slot::Bit(*area, *address)),
        Target::Address { area, address } => {
            // Where a map register starts, the whole register: a uint32 at
            // this address must not get its value in the high word only.
            let starts = memory.map().and_then(|map| {
                map.registers
                    .iter()
                    .find(|r| {
                        r.area == *area
                            && r.address == *address
                            && r.bit.is_none()
                            && !matches!(r.ty, DataType::Bool | DataType::String)
                    })
                    .map(|r| Slot::Register(r, r.byte_order(&map.meta)))
            });
            Ok(starts.unwrap_or(Slot::Word(*area, *address)))
        }
        Target::Register { register } => {
            let Some(map) = memory.map() else {
                return Err(format!(
                    "\"{register}\" is a map register, and the simulator has no map loaded"
                ));
            };
            let key = register.trim().to_lowercase();
            let r = map
                .registers
                .iter()
                .find(|r| r.name.trim().to_lowercase() == key)
                .ok_or_else(|| format!("the loaded map has no register \"{register}\""))?;
            if r.ty == DataType::String {
                return Err(format!(
                    "\"{}\" holds text; generators make numbers",
                    r.name
                ));
            }
            Ok(Slot::Register(r, r.byte_order(&map.meta)))
        }
    }
}

fn number(value: Value) -> f64 {
    match value {
        Value::Bool(b) => f64::from(u8::from(b)),
        Value::Int(v) => v as f64,
        Value::Uint(v) => v as f64,
        Value::Float(v) => v,
        Value::Text(_) => 0.0,
    }
}

fn words(memory: &Memory, area: Area, address: u16, count: u16) -> Vec<u16> {
    match memory.read(area, address, count) {
        RawData::Registers(w) => w,
        RawData::Bits(b) => b.into_iter().map(u16::from).collect(),
    }
}

fn first_bit(memory: &Memory, area: Area, address: u16) -> bool {
    match memory.read(area, address, 1) {
        RawData::Bits(b) => b.first().copied().unwrap_or(false),
        RawData::Registers(w) => w.first().is_some_and(|w| *w != 0),
    }
}

/// The value at a target: 0 or 1 for a bit, the word for a raw register,
/// the engineering value for a map register.
pub fn read(memory: &Memory, target: &Target) -> Result<f64, String> {
    Ok(match resolve(memory, target)? {
        Slot::Bit(area, address) => f64::from(u8::from(first_bit(memory, area, address))),
        Slot::Word(area, address) => f64::from(words(memory, area, address, 1)[0]),
        Slot::Register(r, order) => {
            if r.area.is_bit() {
                return Ok(f64::from(u8::from(first_bit(memory, r.area, r.address))));
            }
            let regs = words(memory, r.area, r.address, r.count());
            let word = regs.first().copied().unwrap_or(0);
            match (r.ty, r.bit, r.bits) {
                (DataType::Bool, Some(bit), _) => f64::from(word >> bit & 1),
                (_, Some(bit), Some(width)) => {
                    let raw = Value::Uint(codec::extract_field(word, bit, width).into());
                    number(codec::to_engineering(raw, r.scale, r.offset))
                }
                _ => {
                    let raw =
                        codec::decode(&regs, r.ty, order, r.bit).map_err(|e| e.to_string())?;
                    number(codec::to_engineering(raw, r.scale, r.offset))
                }
            }
        }
    })
}

/// Integer range of a type, to clamp a generated value into.
fn range(ty: DataType) -> Option<(f64, f64)> {
    Some(match ty {
        DataType::Int16 => (i16::MIN.into(), i16::MAX.into()),
        DataType::Uint16 => (0.0, u16::MAX.into()),
        DataType::Int32 => (i32::MIN.into(), i32::MAX.into()),
        DataType::Uint32 => (0.0, u32::MAX.into()),
        DataType::Int64 => (i64::MIN as f64, i64::MAX as f64),
        DataType::Uint64 => (0.0, u64::MAX as f64),
        _ => return None,
    })
}

/// Writes a value to a target; integers are rounded and kept in range.
pub fn write(memory: &mut Memory, target: &Target, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err("the value is not a number".into());
    }
    let on = value >= 0.5;
    let (area, address, data) = match resolve(memory, target)? {
        Slot::Bit(area, address) => (area, address, RawData::Bits(vec![on])),
        Slot::Word(area, address) => {
            // Negative values wrap like an int16 would.
            let w = value.round().clamp(i16::MIN.into(), u16::MAX.into()) as i32 as u16;
            (area, address, RawData::Registers(vec![w]))
        }
        Slot::Register(r, order) => {
            let (area, address) = (r.area, r.address);
            if area.is_bit() {
                (area, address, RawData::Bits(vec![on]))
            } else {
                let word = words(memory, area, address, 1)[0];
                match (r.ty, r.bit, r.bits) {
                    (DataType::Bool, Some(bit), _) => {
                        let w = if on {
                            word | 1 << bit
                        } else {
                            word & !(1 << bit)
                        };
                        (area, address, RawData::Registers(vec![w]))
                    }
                    (_, Some(bit), Some(width)) => {
                        let raw = number(codec::from_engineering(
                            Value::Float(value),
                            r.scale,
                            r.offset,
                        ));
                        let max = f64::from(codec::field_mask(0, width));
                        let n = raw.round().clamp(0.0, max) as u16;
                        let w = codec::insert_field(word, bit, width, n);
                        (area, address, RawData::Registers(vec![w]))
                    }
                    _ => {
                        let mut raw = number(codec::from_engineering(
                            Value::Float(value),
                            r.scale,
                            r.offset,
                        ));
                        if let Some((lo, hi)) = range(r.ty) {
                            raw = raw.round().clamp(lo, hi);
                        }
                        let regs = codec::encode(&Value::Float(raw), r.ty, order, r.length)
                            .map_err(|e| e.to_string())?;
                        (area, address, RawData::Registers(regs))
                    }
                }
            }
        }
    };
    memory.set(area, address, &data)
}

/// Why a generator cannot run against this memory, or None when it can.
pub fn problem(memory: &Memory, g: &Generator) -> Option<String> {
    if g.every_ms < MIN_EVERY_MS {
        return Some(format!("update at most every {MIN_EVERY_MS} ms"));
    }
    if let Err(e) = resolve(memory, &g.target) {
        return Some(e);
    }
    match &g.kind {
        Kind::Increment { step, min, max } if *step == 0.0 || min >= max => {
            Some("the step must not be 0 and the limit must be above the start".into())
        }
        Kind::Sine { period_s, .. } if *period_s <= 0.0 => {
            Some("the period must be above 0".into())
        }
        Kind::Random { min, max } if min > max => {
            Some("the lowest value must not be above the highest".into())
        }
        Kind::Copy { from } => resolve(memory, from).err(),
        _ => None,
    }
}

/// A small xorshift generator: random enough for test values, no crate.
pub struct Rng(u64);

impl Rng {
    pub fn seeded() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0x2545_F491, |d| d.as_nanos() as u64);
        Self(nanos | 1)
    }

    /// A number from 0 (inclusive) to 1 (exclusive).
    pub fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Runs the generators that fall due between `before` and `now` (times since
/// the simulator started, in milliseconds). A generator that cannot run is
/// skipped.
pub fn run(memory: &mut Memory, list: &[Generator], before: u64, now: u64, rng: &mut Rng) {
    for g in list {
        if g.every_ms < MIN_EVERY_MS || now / g.every_ms == before / g.every_ms {
            continue;
        }
        let value = match &g.kind {
            Kind::Increment { step, min, max } => {
                let Ok(current) = read(memory, &g.target) else {
                    continue;
                };
                let next = if current < *min || current > *max {
                    if *step > 0.0 { *min } else { *max }
                } else {
                    current + step
                };
                if next > *max {
                    *min
                } else if next < *min {
                    *max
                } else {
                    next
                }
            }
            Kind::Sine {
                period_s,
                amplitude,
                offset,
            } => offset + amplitude * (TAU * now as f64 / 1000.0 / period_s).sin(),
            Kind::Random { min, max } => min + rng.unit() * (max - min),
            Kind::Copy { from } => match read(memory, from) {
                Ok(v) => v,
                Err(_) => continue,
            },
        };
        let _ = write(memory, &g.target, value);
    }
}

/// Runs the generators until the task is aborted.
pub async fn drive(memory: Arc<Mutex<Memory>>, generators: Shared) {
    let started = Instant::now();
    let mut before = 0;
    let mut rng = Rng::seeded();
    let mut ticks = tokio::time::interval(TICK);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        let now = started.elapsed().as_millis() as u64;
        let list = generators.lock().unwrap().clone();
        if !list.is_empty() {
            run(&mut memory.lock().unwrap(), &list, before, now, &mut rng);
        }
        before = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mbmap;

    fn memory() -> Memory {
        let text = "@mbmap=1\n@vendor=ACME\n@model=PM-1\n\
                    name,area,address,type,length,scale,offset,access\n\
                    Voltage,holding,0,float32,,1,0,R\n\
                    Current,input,2,int16,,0.1,0,R\n\
                    Temp,input,3,uint16,,0.5,-40,R\n\
                    Label,holding,10,string,4,1,0,R\n\
                    Utilization,input,20,uint32,,1,0,R\n";
        let parsed = mbmap::parse(text.as_bytes());
        let map = parsed
            .map
            .unwrap_or_else(|| panic!("{:?}", parsed.diagnostics));
        Memory::from_map(&map, false)
    }

    fn reg(name: &str) -> Target {
        Target::Register {
            register: name.into(),
        }
    }

    fn addr(area: Area, address: u16) -> Target {
        Target::Address { area, address }
    }

    #[test]
    fn an_address_where_a_register_starts_takes_the_whole_register() {
        let mut m = memory();
        write(&mut m, &addr(Area::Input, 20), 67.0).unwrap();
        assert_eq!(m.read(Area::Input, 20, 2), RawData::Registers(vec![0, 67]));
        assert_eq!(read(&m, &addr(Area::Input, 20)).unwrap(), 67.0);
        // Inside the register, or with no register there: one raw word.
        write(&mut m, &addr(Area::Input, 21), 5.0).unwrap();
        assert_eq!(m.read(Area::Input, 20, 2), RawData::Registers(vec![0, 5]));
        write(&mut m, &addr(Area::Input, 40), -1.0).unwrap();
        assert_eq!(m.read(Area::Input, 40, 1), RawData::Registers(vec![0xFFFF]));
    }

    #[test]
    fn map_registers_take_engineering_values() {
        let mut m = memory();
        write(&mut m, &reg("Voltage"), 231.5).unwrap();
        assert_eq!(read(&m, &reg("voltage")).unwrap(), 231.5);
        write(&mut m, &reg("Current"), -2.37).unwrap();
        assert_eq!(
            m.read(Area::Input, 2, 1),
            RawData::Registers(vec![(-24i16) as u16])
        );
        write(&mut m, &reg("Temp"), 25.0).unwrap();
        assert_eq!(m.read(Area::Input, 3, 1), RawData::Registers(vec![130]));
        assert!((read(&m, &reg("Temp")).unwrap() - 25.0).abs() < 1e-9);
        // Out of range is clamped, not refused.
        write(&mut m, &reg("Current"), 1e9).unwrap();
        assert_eq!(
            m.read(Area::Input, 2, 1),
            RawData::Registers(vec![i16::MAX as u16])
        );
    }

    #[test]
    fn problems_are_explained() {
        let m = memory();
        let g = |target, kind| Generator {
            target,
            kind,
            every_ms: 100,
        };
        let sine = Kind::Sine {
            period_s: 10.0,
            amplitude: 5.0,
            offset: 230.0,
        };
        assert_eq!(problem(&m, &g(reg("Voltage"), sine.clone())), None);
        assert!(problem(&m, &g(reg("Nope"), sine.clone())).is_some());
        assert!(problem(&m, &g(reg("Label"), sine.clone())).is_some());
        let flat = Kind::Increment {
            step: 1.0,
            min: 5.0,
            max: 5.0,
        };
        assert!(problem(&m, &g(addr(Area::Holding, 0), flat)).is_some());
        let fast = Generator {
            every_ms: 10,
            ..g(addr(Area::Coil, 0), sine)
        };
        assert!(problem(&m, &fast).is_some());
        assert!(
            problem(
                &Memory::default(),
                &g(reg("Voltage"), Kind::Random { min: 0.0, max: 1.0 })
            )
            .is_some()
        );
    }

    #[test]
    fn generators_run_when_due() {
        let mut m = memory();
        let mut rng = Rng::seeded();
        let list = vec![
            Generator {
                target: addr(Area::Holding, 20),
                kind: Kind::Increment {
                    step: 4.0,
                    min: 0.0,
                    max: 10.0,
                },
                every_ms: 100,
            },
            Generator {
                target: addr(Area::Discrete, 1),
                kind: Kind::Copy {
                    from: addr(Area::Coil, 1),
                },
                every_ms: 100,
            },
            Generator {
                target: reg("Voltage"),
                kind: Kind::Sine {
                    period_s: 4.0,
                    amplitude: 5.0,
                    offset: 230.0,
                },
                every_ms: 100,
            },
            Generator {
                target: addr(Area::Input, 30),
                kind: Kind::Random {
                    min: 10.0,
                    max: 20.0,
                },
                every_ms: 100,
            },
        ];
        m.set(Area::Coil, 1, &RawData::Bits(vec![true])).unwrap();
        // Not due yet: nothing changes.
        run(&mut m, &list, 0, 50, &mut rng);
        assert_eq!(m.read(Area::Holding, 20, 1), RawData::Registers(vec![0]));
        let mut seen = Vec::new();
        for i in 1..=4u64 {
            run(&mut m, &list, i * 100 - 1, i * 100, &mut rng);
            seen.push(read(&m, &addr(Area::Holding, 20)).unwrap());
            let r = read(&m, &addr(Area::Input, 30)).unwrap();
            assert!((10.0..=20.0).contains(&r));
        }
        assert_eq!(seen, [4.0, 8.0, 0.0, 4.0]);
        assert_eq!(m.read(Area::Discrete, 1, 1), RawData::Bits(vec![true]));
        // At t = 1 s, a quarter of a 4 s period: the crest.
        run(&mut m, &list, 999, 1000, &mut rng);
        assert!((read(&m, &reg("Voltage")).unwrap() - 235.0).abs() < 1e-3);
    }

    #[test]
    fn generators_read_from_json() {
        let text = r#"[
            {"register": "Voltage", "kind": "sine", "period_s": 10, "amplitude": 5, "offset": 230, "every_ms": 500},
            {"area": "input", "address": 0, "kind": "increment", "step": 1, "max": 100},
            {"area": "discrete", "address": 0, "kind": "copy", "from": {"area": "coil", "address": 0}}
        ]"#;
        let list: Vec<Generator> = serde_json::from_str(text).unwrap();
        assert_eq!(list[0].target, reg("Voltage"));
        assert_eq!(list[1].every_ms, 1000);
        assert_eq!(
            list[2].kind,
            Kind::Copy {
                from: addr(Area::Coil, 0)
            }
        );
        let back: Vec<Generator> =
            serde_json::from_str(&serde_json::to_string(&list).unwrap()).unwrap();
        assert_eq!(back, list);
    }
}

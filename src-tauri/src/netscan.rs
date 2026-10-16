//! Looks for Modbus TCP devices on a range of IPv4 addresses.
//!
//! Each address gets a TCP connection to the port; when it opens, the
//! scan reads one holding register. Any answer, an exception included,
//! shows a Modbus device; a port that opens but does not answer is
//! reported apart. Nothing is ever written.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::modbus::{ConnectionConfig, ModbusError, Session, Transport};
use crate::planner::Area;

/// Most addresses one scan may cover (a /16).
pub const MAX_HOSTS: usize = 65_536;
/// Addresses asked at the same time.
const PARALLEL: usize = 64;

#[derive(Debug, Clone, Deserialize)]
pub struct NetScan {
    /// Addresses: `192.168.1.0/24`, `192.168.1.10-192.168.1.50`,
    /// `192.168.1.10-50` or single addresses, separated by commas.
    pub targets: String,
    pub port: u16,
    /// Unit ID of the request sent to each address.
    pub unit: u8,
    /// How long to wait for the connection, and then for the answer.
    pub timeout_ms: u64,
    /// Also read the device identification (FC43) of Modbus devices.
    #[serde(default)]
    pub identify: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostStatus {
    /// It answered the request: a Modbus device (or gateway).
    Modbus,
    /// The port accepted the connection but no Modbus answer came.
    TcpOnly,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetHit {
    pub ip: String,
    pub port: u16,
    pub unit: u8,
    pub status: HostStatus,
    /// Time from sending the request to its answer, or to giving up.
    pub response_ms: f64,
    /// What came back: "responds", the exception, or why there was no answer.
    pub response: String,
    /// Vendor, product code and revision from FC43, when asked for and given.
    pub identity: Option<String>,
}

/// Where a scan is, for a progress bar.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct NetProgress {
    pub done: u32,
    pub total: u32,
    pub found: u32,
}

/// The addresses of a target list, in order and without repeats.
pub fn parse_targets(text: &str) -> Result<Vec<Ipv4Addr>, String> {
    let mut out: Vec<Ipv4Addr> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for part in text
        .split([',', ' ', '\n'])
        .filter(|p| !p.trim().is_empty())
    {
        let part = part.trim();
        let (first, last) = range_of(part)?;
        if (last - first) as usize >= MAX_HOSTS || out.len() + (last - first) as usize >= MAX_HOSTS
        {
            return Err(format!("at most {MAX_HOSTS} addresses (a /16) per scan"));
        }
        for n in first..=last {
            if seen.insert(n) {
                out.push(Ipv4Addr::from(n));
            }
        }
    }
    if out.is_empty() {
        return Err("no address to scan".into());
    }
    Ok(out)
}

fn ip(text: &str) -> Result<u32, String> {
    text.trim()
        .parse::<Ipv4Addr>()
        .map(u32::from)
        .map_err(|_| format!("'{text}' is not an IPv4 address"))
}

/// First and last address of one target, as numbers.
fn range_of(part: &str) -> Result<(u32, u32), String> {
    if let Some((base, bits)) = part.split_once('/') {
        let bits: u32 = bits
            .parse()
            .ok()
            .filter(|b| *b <= 32)
            .ok_or_else(|| format!("'{part}': the prefix length must be 0 to 32"))?;
        let mask = if bits == 0 {
            0
        } else {
            u32::MAX << (32 - bits)
        };
        let network = ip(base)? & mask;
        let broadcast = network | !mask;
        // Leave out the network and broadcast addresses, which have no host
        return Ok(if bits <= 30 {
            (network + 1, broadcast - 1)
        } else {
            (network, broadcast)
        });
    }
    if let Some((from, to)) = part.split_once('-') {
        let first = ip(from)?;
        let last = match to.trim().parse::<u8>() {
            // "192.168.1.10-50": the last part of the address
            Ok(octet) if !to.contains('.') => (first & !0xFF) | u32::from(octet),
            _ => ip(to)?,
        };
        if last < first {
            return Err(format!("'{part}': the range ends before it starts"));
        }
        return Ok((first, last));
    }
    let one = ip(part)?;
    Ok((one, one))
}

/// Scans the targets, calling `found` for each address that accepts the
/// connection and `progress` as addresses are done. Stops early when
/// `cancel` is set, keeping what was found.
pub async fn scan(
    scan: &NetScan,
    cancel: &AtomicBool,
    found: impl Fn(&NetHit),
    progress: impl Fn(NetProgress),
) -> Result<Vec<NetHit>, ModbusError> {
    let hosts = parse_targets(&scan.targets).map_err(ModbusError::Invalid)?;
    let total = hosts.len() as u32;
    let scan = Arc::new(scan.clone());
    let mut pending = hosts.into_iter();
    let mut running = JoinSet::new();
    let mut hits = Vec::new();
    let mut done = 0;
    progress(NetProgress {
        done,
        total,
        found: 0,
    });
    loop {
        while running.len() < PARALLEL && !cancel.load(Ordering::Relaxed) {
            let Some(host) = pending.next() else { break };
            running.spawn(probe(host, scan.clone()));
        }
        let Some(joined) = running.join_next().await else {
            break;
        };
        done += 1;
        if let Ok(Some(hit)) = joined {
            found(&hit);
            hits.push(hit);
        }
        progress(NetProgress {
            done,
            total,
            found: hits.len() as u32,
        });
    }
    hits.sort_by_key(|h| h.ip.parse::<Ipv4Addr>().map(u32::from).unwrap_or(0));
    Ok(hits)
}

/// Connects to one address and asks for one holding register. `None`
/// when the connection does not open.
async fn probe(host: Ipv4Addr, scan: Arc<NetScan>) -> Option<NetHit> {
    let config = ConnectionConfig {
        transport: Transport::Tcp {
            host: host.to_string(),
            port: scan.port,
        },
        unit_id: scan.unit,
        timeout_ms: scan.timeout_ms.max(1),
    };
    let mut session = Session::connect(&config, None).await.ok()?;
    let asked = Instant::now();
    let answer = session.read(Area::Holding, 0, 1).await;
    let response_ms = asked.elapsed().as_secs_f64() * 1000.0;
    let (status, response) = match answer {
        Ok(_) => (HostStatus::Modbus, "responds".to_string()),
        Err(ModbusError::Exception { code, name, .. }) => (
            HostStatus::Modbus,
            format!("responds with exception 0x{code:02X} ({name})"),
        ),
        Err(e) => (HostStatus::TcpOnly, e.to_string()),
    };
    let identity = match (status, scan.identify) {
        (HostStatus::Modbus, true) => identity(&mut session).await,
        _ => None,
    };
    session.disconnect().await;
    Some(NetHit {
        ip: host.to_string(),
        port: scan.port,
        unit: scan.unit,
        status,
        response_ms,
        response,
        identity,
    })
}

/// Vendor, product code and revision, from the basic identification.
async fn identity(session: &mut Session) -> Option<String> {
    let id = session.read_device_id(1).await.ok()?;
    let text = id
        .objects
        .iter()
        .map(|o| o.value.trim())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ips(text: &str) -> Vec<String> {
        parse_targets(text)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn targets_take_subnets_ranges_and_lists() {
        assert_eq!(ips("10.0.0.0/30"), ["10.0.0.1", "10.0.0.2"]);
        assert_eq!(ips("10.0.0.7/31"), ["10.0.0.6", "10.0.0.7"]);
        assert_eq!(ips("10.0.0.5/32"), ["10.0.0.5"]);
        assert_eq!(parse_targets("192.168.1.0/24").unwrap().len(), 254);
        assert_eq!(
            ips("192.168.1.10-12, 192.168.1.250-192.168.2.1"),
            [
                "192.168.1.10",
                "192.168.1.11",
                "192.168.1.12",
                "192.168.1.250",
                "192.168.1.251",
                "192.168.1.252",
                "192.168.1.253",
                "192.168.1.254",
                "192.168.1.255",
                "192.168.2.0",
                "192.168.2.1"
            ]
        );
        assert_eq!(ips("10.0.0.1 10.0.0.1,10.0.0.2"), ["10.0.0.1", "10.0.0.2"]);
    }

    #[test]
    fn bad_targets_say_why() {
        for (text, why) in [
            ("", "no address"),
            ("10.0.0.300", "not an IPv4"),
            ("10.0.0.0/33", "prefix length"),
            ("10.0.0.9-3", "ends before"),
            ("10.0.0.0/8", "at most 65536"),
            ("example.com", "not an IPv4"),
        ] {
            let err = parse_targets(text).unwrap_err();
            assert!(err.contains(why), "{text}: {err}");
        }
    }
}

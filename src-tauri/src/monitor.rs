//! A passive monitor of a serial Modbus line: it listens to a master and its
//! devices talking, without ever sending a byte, and pairs each request with
//! the answer that follows.
//!
//! RTU frames are told apart by the silence between them (3.5 characters).
//! Bytes that arrive together are also split where a CRC holds, since a
//! serial driver can hand over two frames at once. ASCII frames are lines.

use std::io;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_modbus::bytes::Bytes;
use tokio_modbus::{Request, Response};

use crate::ascii;
use crate::serve::{Framing, crc16};
use crate::simulator::where_of;

/// What became of one frame, or of a request and its answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// A request and its answer.
    Ok,
    /// A request answered with an exception.
    Exception,
    /// A request nothing answered in time.
    NoAnswer,
    /// A request to unit 0, which no device answers.
    Broadcast,
    /// A frame whose CRC (or LRC) does not hold.
    BadCheck,
    /// A whole frame that is neither a request nor the answer to one.
    Unknown,
}

/// One request and its answer, or one frame on its own.
#[derive(Debug, Clone, Serialize)]
pub struct MonitorEvent {
    /// When the request (or the frame) was seen.
    pub time_ms: u64,
    pub outcome: Outcome,
    pub unit: Option<u8>,
    pub function: Option<u8>,
    pub address: Option<u16>,
    pub count: Option<u16>,
    /// Registers read (or written), or bits.
    pub registers: Option<Vec<u16>>,
    pub bits: Option<Vec<bool>>,
    pub exception: Option<u8>,
    /// From the end of the request to the end of the answer.
    pub ms: Option<u64>,
    /// The request's bytes (unit ID to CRC), and the answer's, as hex.
    pub request: String,
    pub response: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Splits the bytes heard between two silences into RTU frames: the
/// shortest prefix whose CRC holds, again and again. What is left without a
/// CRC that holds is one bad frame.
pub fn split_rtu(mut buf: &[u8]) -> Vec<(Vec<u8>, bool)> {
    let mut out = Vec::new();
    while !buf.is_empty() {
        let found = (4..=buf.len()).find(|&n| {
            let (body, crc) = buf[..n].split_at(n - 2);
            body[0] <= 247 && crc16(body) == u16::from_le_bytes([crc[0], crc[1]])
        });
        match found {
            Some(n) => {
                out.push((buf[..n].to_vec(), true));
                buf = &buf[n..];
            }
            None => {
                out.push((buf.to_vec(), false));
                break;
            }
        }
    }
    out
}

/// A request waiting for its answer.
struct Pending {
    time_ms: u64,
    seen: Instant,
    unit: u8,
    request: Request<'static>,
    raw: Vec<u8>,
}

/// Pairs requests with answers, frame by frame.
pub struct Pairer {
    pending: Option<Pending>,
    /// How long a request may wait for its answer.
    timeout: Duration,
}

impl Pairer {
    pub fn new(timeout: Duration) -> Self {
        Self {
            pending: None,
            timeout,
        }
    }

    /// One frame: `raw` is all of it, `unit` and `pdu` its parts when its
    /// check holds.
    pub fn frame(&mut self, raw: &[u8], parts: Option<(u8, &[u8])>) -> Vec<MonitorEvent> {
        let mut out = Vec::new();
        let Some((unit, pdu)) = parts else {
            out.push(single(Outcome::BadCheck, raw, None, None));
            return out;
        };
        let function = pdu.first().copied();
        // The answer to the waiting request: same unit, same function (or
        // its exception).
        if let Some(p) = &self.pending
            && p.unit == unit
            && let Some(f) = function
            && f & 0x7F == p.request.function_code().value()
            && let Some(event) = answer(p, pdu, raw)
        {
            self.pending = None;
            out.push(event);
            return out;
        }
        match Request::try_from(Bytes::copy_from_slice(pdu)) {
            Ok(request) => {
                out.extend(self.expire_now());
                if unit == 0 {
                    let mut e = request_event(Outcome::Broadcast, now_ms(), 0, &request, raw);
                    written(&mut e, &request);
                    out.push(e);
                } else {
                    self.pending = Some(Pending {
                        time_ms: now_ms(),
                        seen: Instant::now(),
                        unit,
                        request,
                        raw: raw.to_vec(),
                    });
                }
            }
            Err(_) => out.push(single(Outcome::Unknown, raw, Some(unit), function)),
        }
        out
    }

    /// The waiting request, if its time to be answered has passed.
    pub fn expire(&mut self) -> Option<MonitorEvent> {
        match &self.pending {
            Some(p) if p.seen.elapsed() >= self.timeout => self.expire_now(),
            _ => None,
        }
    }

    /// The waiting request as unanswered, whatever the time.
    fn expire_now(&mut self) -> Option<MonitorEvent> {
        let p = self.pending.take()?;
        let mut e = request_event(Outcome::NoAnswer, p.time_ms, p.unit, &p.request, &p.raw);
        written(&mut e, &p.request);
        Some(e)
    }
}

fn single(outcome: Outcome, raw: &[u8], unit: Option<u8>, function: Option<u8>) -> MonitorEvent {
    MonitorEvent {
        time_ms: now_ms(),
        outcome,
        unit,
        function,
        address: None,
        count: None,
        registers: None,
        bits: None,
        exception: None,
        ms: None,
        request: hex(raw),
        response: None,
    }
}

fn request_event(
    outcome: Outcome,
    time_ms: u64,
    unit: u8,
    request: &Request<'_>,
    raw: &[u8],
) -> MonitorEvent {
    let (address, count) = where_of(request);
    MonitorEvent {
        time_ms,
        outcome,
        unit: Some(unit),
        function: Some(request.function_code().value()),
        address,
        count,
        registers: None,
        bits: None,
        exception: None,
        ms: None,
        request: hex(raw),
        response: None,
    }
}

/// The values a write request carries.
fn written(e: &mut MonitorEvent, request: &Request<'_>) {
    match request {
        Request::WriteSingleRegister(_, v) => e.registers = Some(vec![*v]),
        Request::WriteMultipleRegisters(_, v) => e.registers = Some(v.to_vec()),
        Request::WriteSingleCoil(_, v) => e.bits = Some(vec![*v]),
        Request::WriteMultipleCoils(_, v) => e.bits = Some(v.to_vec()),
        _ => {}
    }
}

/// The waiting request with `pdu` as its answer, when `pdu` reads as one.
fn answer(p: &Pending, pdu: &[u8], raw: &[u8]) -> Option<MonitorEvent> {
    let mut e = request_event(Outcome::Ok, p.time_ms, p.unit, &p.request, &p.raw);
    e.ms = Some(p.seen.elapsed().as_millis() as u64);
    e.response = Some(hex(raw));
    if pdu[0] & 0x80 != 0 {
        e.outcome = Outcome::Exception;
        e.exception = Some(*pdu.get(1)?);
        return Some(e);
    }
    let response = Response::try_from(Bytes::copy_from_slice(pdu)).ok()?;
    let wanted = usize::from(e.count.unwrap_or(0));
    match response {
        Response::ReadHoldingRegisters(v)
        | Response::ReadInputRegisters(v)
        | Response::ReadWriteMultipleRegisters(v) => {
            // A different count is another request's answer.
            if wanted != 0 && v.len() != wanted {
                return None;
            }
            e.registers = Some(v);
        }
        Response::ReadCoils(mut v) | Response::ReadDiscreteInputs(mut v) => {
            // Bits come in whole bytes: keep the ones asked for.
            if wanted > v.len() {
                return None;
            }
            v.truncate(wanted);
            e.bits = Some(v);
        }
        _ => written(&mut e, &p.request),
    }
    Some(e)
}

/// The silence that ends an RTU frame at a baud rate: 3.5 characters of 11
/// bits, and at least 2 ms (the Modbus spec fixes 1.75 ms above 19200 baud;
/// timers are not finer than a millisecond).
pub fn frame_gap(baudrate: u32) -> Duration {
    let micros = 3_500_000u64 * 11 / u64::from(baudrate.max(1));
    Duration::from_micros(micros.max(2_000))
}

/// Listens to `stream` until it closes or fails, reporting each request and
/// answer (or frame) to `sink`. Reads only: nothing is ever written.
pub async fn listen<S>(
    mut stream: S,
    framing: Framing,
    baudrate: u32,
    timeout: Duration,
    sink: impl Fn(MonitorEvent),
) -> io::Result<()>
where
    S: AsyncRead + Unpin,
{
    let gap = frame_gap(baudrate);
    let mut pairer = Pairer::new(timeout);
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        // Wait for a frame's end while bytes are coming; otherwise wake up
        // now and then to report a request nobody answered.
        let wait = if buf.is_empty() || framing == Framing::Ascii {
            Duration::from_millis(100)
        } else {
            gap
        };
        let n = match tokio::time::timeout(wait, stream.read(&mut chunk)).await {
            Ok(read) => read?,
            Err(_) => {
                if framing == Framing::Rtu && !buf.is_empty() {
                    for (frame, ok) in split_rtu(&std::mem::take(&mut buf)) {
                        let parts = ok.then(|| (frame[0], &frame[1..frame.len() - 2]));
                        pairer.frame(&frame, parts).into_iter().for_each(&sink);
                    }
                }
                if let Some(e) = pairer.expire() {
                    sink(e);
                }
                continue;
            }
        };
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if framing == Framing::Ascii {
            while let Some(end) = buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buf.drain(..=end).collect();
                let Some(start) = line.iter().rposition(|b| *b == b':') else {
                    continue;
                };
                let text = line[start + 1..]
                    .strip_suffix(b"\r\n")
                    .unwrap_or(&line[start + 1..]);
                let raw = &line[start..];
                match ascii::decode_frame(text) {
                    Ok((unit, pdu)) => pairer.frame(raw, Some((unit, &pdu))),
                    Err(_) => pairer.frame(raw, None),
                }
                .into_iter()
                .for_each(&sink);
            }
            // A line far longer than any frame is noise.
            if buf.len() > 1024 {
                buf.clear();
            }
        } else if buf.len() > 4096 {
            // No silence for that long: the line is not Modbus at this speed.
            for (frame, _) in split_rtu(&std::mem::take(&mut buf)) {
                pairer.frame(&frame, None).into_iter().for_each(&sink);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    fn rtu(bytes: &[u8]) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out.extend(crc16(bytes).to_le_bytes());
        out
    }

    fn frames(pairer: &mut Pairer, bytes: &[u8]) -> Vec<MonitorEvent> {
        split_rtu(bytes)
            .into_iter()
            .flat_map(|(f, ok)| {
                let parts = ok.then(|| (f[0], f[1..f.len() - 2].to_vec()));
                pairer.frame(&f, parts.as_ref().map(|(u, p)| (*u, p.as_slice())))
            })
            .collect()
    }

    #[test]
    fn frames_heard_together_are_split_where_the_crc_holds() {
        let mut both = rtu(&[1, 3, 0, 10, 0, 2]);
        both.extend(rtu(&[1, 3, 4, 0, 7, 0, 8]));
        both.extend([0x55, 0x66]);
        let split = split_rtu(&both);
        assert_eq!(split.len(), 3);
        assert!(split[0].1 && split[1].1 && !split[2].1);
    }

    #[test]
    fn a_read_is_paired_with_its_answer() {
        let mut p = Pairer::new(Duration::from_secs(1));
        let mut bytes = rtu(&[1, 3, 0, 10, 0, 2]);
        bytes.extend(rtu(&[1, 3, 4, 0, 7, 0, 8]));
        let events = frames(&mut p, &bytes);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.outcome, Outcome::Ok);
        assert_eq!(
            (e.unit, e.function, e.address, e.count),
            (Some(1), Some(3), Some(10), Some(2))
        );
        assert_eq!(e.registers, Some(vec![7, 8]));
        assert_eq!(
            e.response.as_deref(),
            Some(hex(&rtu(&[1, 3, 4, 0, 7, 0, 8])).as_str())
        );
    }

    #[test]
    fn exceptions_silence_broadcasts_and_bad_frames() {
        let mut p = Pairer::new(Duration::from_secs(1));
        let mut bytes = rtu(&[2, 4, 0, 0, 0, 1]);
        bytes.extend(rtu(&[2, 0x84, 2]));
        // Unit 3 never answers; the next request shows it.
        bytes.extend(rtu(&[3, 1, 0, 0, 0, 3]));
        bytes.extend(rtu(&[0, 6, 0, 5, 0, 9]));
        let events = frames(&mut p, &bytes);
        let outcomes: Vec<_> = events.iter().map(|e| e.outcome).collect();
        assert_eq!(
            outcomes,
            [Outcome::Exception, Outcome::NoAnswer, Outcome::Broadcast]
        );
        assert_eq!(events[0].exception, Some(2));
        assert_eq!(events[2].registers, Some(vec![9]));
        let bad = frames(&mut p, &[1, 3, 0, 0, 0, 1, 0, 0]);
        assert_eq!(bad[0].outcome, Outcome::BadCheck);
    }

    #[test]
    fn coils_keep_only_the_bits_asked_for() {
        let mut p = Pairer::new(Duration::from_secs(1));
        let mut bytes = rtu(&[1, 1, 0, 0, 0, 3]);
        bytes.extend(rtu(&[1, 1, 1, 0b101]));
        let e = &frames(&mut p, &bytes)[0];
        assert_eq!(e.bits, Some(vec![true, false, true]));
    }

    #[test]
    fn the_gap_is_three_and_a_half_characters() {
        assert_eq!(frame_gap(9600), Duration::from_micros(4010));
        assert_eq!(frame_gap(115_200), Duration::from_millis(2));
    }

    #[tokio::test]
    async fn listens_to_a_line_with_silences_and_never_writes() {
        let (mut line, monitor) = tokio::io::duplex(1024);
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = events.clone();
        let task = tokio::spawn(async move {
            listen(
                monitor,
                Framing::Rtu,
                19200,
                Duration::from_millis(200),
                move |e| seen.lock().unwrap().push(e),
            )
            .await
        });
        line.write_all(&rtu(&[1, 3, 0, 0, 0, 1])).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        line.write_all(&rtu(&[1, 3, 2, 0x12, 0x34])).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        line.write_all(&rtu(&[5, 3, 0, 0, 0, 1])).await.unwrap();
        // Nothing answers unit 5: after the timeout it is reported.
        tokio::time::sleep(Duration::from_millis(400)).await;
        drop(line);
        task.await.unwrap().unwrap();
        let log = events.lock().unwrap();
        let outcomes: Vec<_> = log.iter().map(|e| e.outcome).collect();
        assert_eq!(outcomes, [Outcome::Ok, Outcome::NoAnswer]);
        assert_eq!(log[0].registers, Some(vec![0x1234]));
    }

    #[tokio::test]
    async fn ascii_lines_are_read() {
        let (mut line, monitor) = tokio::io::duplex(1024);
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = events.clone();
        let task = tokio::spawn(async move {
            listen(
                monitor,
                Framing::Ascii,
                9600,
                Duration::from_millis(200),
                move |e| seen.lock().unwrap().push(e),
            )
            .await
        });
        line.write_all(&ascii::encode_frame(1, &[3, 0, 0, 0, 1]))
            .await
            .unwrap();
        line.write_all(&ascii::encode_frame(1, &[3, 2, 0, 9]))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(line);
        task.await.unwrap().unwrap();
        let log = events.lock().unwrap();
        assert_eq!(log[0].outcome, Outcome::Ok);
        assert_eq!(log[0].registers, Some(vec![9]));
    }
}

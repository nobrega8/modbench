//! Server side of Modbus RTU and ASCII framing, over any byte stream (a
//! serial port, or a TCP connection for RTU over TCP), and of MBAP and RTU
//! frames in UDP datagrams. tokio-modbus only serves RTU on a serial port
//! and has neither ASCII nor UDP, so the simulator and the gateway frame
//! requests and answers here, and listen with [`listen`].

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tokio::task::JoinHandle;
use tokio_modbus::bytes::Bytes;
use tokio_modbus::server::tcp::{Server, accept_tcp_connection};
use tokio_modbus::{ExceptionCode, Request, Response, SlaveRequest};

use crate::ascii;
use crate::modbus::{self, ModbusError, Transport};

/// How requests and answers are framed on the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    Rtu,
    Ascii,
}

/// Silence after which bytes that do not make a frame are dropped.
const GAP: Duration = Duration::from_millis(100);

/// What answers requests: given the unit ID and the request, the answer to
/// send back, or None to stay silent (another unit ID, or a broadcast).
pub trait Handler: Send + Sync {
    fn handle(
        &self,
        unit: u8,
        request: Request<'static>,
    ) -> impl Future<Output = Option<Result<Response, ExceptionCode>>> + Send;
}

/// Modbus CRC-16 of an RTU frame (sent low byte first).
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for b in bytes {
        crc ^= u16::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// Length of the RTU request at the start of `buf` (unit ID to CRC), from
/// its function code; for requests with a byte count, the length needed to
/// see it while it has not come yet. None for function codes whose length
/// is not known: those are taken as a frame at the next silence.
fn rtu_request_len(buf: &[u8]) -> Option<usize> {
    let counted =
        |at: usize, fixed: usize| Some(buf.get(at).map_or(at + 1, |n| fixed + usize::from(*n)));
    match *buf.get(1)? {
        1..=6 | 8 => Some(8),
        7 | 11 | 12 | 17 => Some(4),
        15 | 16 => counted(6, 9),
        20 | 21 => counted(2, 5),
        22 => Some(10),
        23 => counted(10, 13),
        24 => Some(6),
        43 => Some(7),
        _ => None,
    }
}

/// The PDU of an answer (function code and data), as on the wire.
pub fn response_pdu(response: &Response) -> Vec<u8> {
    use Response::*;
    let mut out = vec![response.function_code().value()];
    let mut put = |w: u16| out.extend_from_slice(&w.to_be_bytes());
    match response {
        ReadCoils(bits) | ReadDiscreteInputs(bits) => {
            let mut packed = vec![0u8; bits.len().div_ceil(8)];
            for (i, _) in bits.iter().enumerate().filter(|(_, on)| **on) {
                packed[i / 8] |= 1 << (i % 8);
            }
            out.push(packed.len() as u8);
            out.extend(packed);
        }
        ReadInputRegisters(words) | ReadHoldingRegisters(words) => {
            out.push((words.len() * 2) as u8);
            for w in words {
                out.extend_from_slice(&w.to_be_bytes());
            }
        }
        WriteSingleCoil(a, on) => {
            put(*a);
            put(if *on { 0xFF00 } else { 0 });
        }
        WriteMultipleCoils(a, n) | WriteMultipleRegisters(a, n) | WriteSingleRegister(a, n) => {
            put(*a);
            put(*n);
        }
        MaskWriteRegister(a, and, or) => {
            put(*a);
            put(*and);
            put(*or);
        }
        ReadDeviceIdentification(id) => {
            out.extend([
                0x0E,
                id.read_code.value(),
                id.conformity_level.value(),
                if id.more_follows { 0xFF } else { 0 },
                id.next_object_id,
                id.device_id_objects.len() as u8,
            ]);
            for o in &id.device_id_objects {
                out.extend([o.id, o.value.len() as u8]);
                out.extend_from_slice(&o.value);
            }
        }
        // The simulator answers nothing else.
        _ => return exception_pdu(out[0], ExceptionCode::IllegalFunction),
    }
    out
}

fn exception_pdu(function: u8, code: ExceptionCode) -> Vec<u8> {
    vec![function | 0x80, u8::from(code)]
}

/// The answer PDU for a request PDU sent to `unit`, or None for silence.
async fn answer(handler: &impl Handler, unit: u8, pdu: &[u8]) -> Option<Vec<u8>> {
    let function = *pdu.first()?;
    if function >= 0x80 {
        return None;
    }
    match Request::try_from(Bytes::copy_from_slice(pdu)) {
        Ok(request) => match handler.handle(unit, request).await? {
            Ok(response) => Some(response_pdu(&response)),
            Err(code) => Some(exception_pdu(function, code)),
        },
        // A request that cannot be decoded: still only the addressed unit
        // answers.
        Err(_) => handler
            .handle(unit, Request::Custom(function, Vec::new().into()))
            .await
            .map(|_| exception_pdu(function, ExceptionCode::IllegalDataValue)),
    }
}

/// Answers requests on `stream` until it closes or fails.
pub async fn serve<S>(mut stream: S, framing: Framing, handler: impl Handler) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let n = if buf.is_empty() {
            stream.read(&mut chunk).await?
        } else {
            match tokio::time::timeout(GAP, stream.read(&mut chunk)).await {
                Ok(read) => read?,
                Err(_) => {
                    // Silence: what is left is one frame of an unknown
                    // length, or noise.
                    let frame = std::mem::take(&mut buf);
                    if framing == Framing::Rtu
                        && let Some(reply) = rtu_frame(&handler, &frame).await
                    {
                        stream.write_all(&reply).await?;
                    }
                    continue;
                }
            }
        };
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        let replies = match framing {
            Framing::Rtu => rtu_frames(&handler, &mut buf).await,
            Framing::Ascii => ascii_frames(&handler, &mut buf).await,
        };
        for reply in replies {
            stream.write_all(&reply).await?;
        }
    }
}

/// The answer to one MBAP frame (transaction ID, protocol 0, length, unit
/// ID, PDU), as Modbus TCP sends it, when it is whole and the unit answers.
pub async fn mbap_frame(handler: &impl Handler, frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < 8 || frame[2..4] != [0, 0] {
        return None;
    }
    let len = usize::from(u16::from_be_bytes([frame[4], frame[5]]));
    if len < 2 || frame.len() != 6 + len {
        return None;
    }
    let unit = frame[6];
    let pdu = answer(handler, unit, &frame[7..]).await?;
    let mut out = frame[..4].to_vec();
    out.extend(((pdu.len() + 1) as u16).to_be_bytes());
    out.push(unit);
    out.extend(pdu);
    Some(out)
}

/// The answer to one whole RTU frame (unit ID to CRC), when its CRC holds
/// and the unit answers.
pub async fn rtu_frame(handler: &impl Handler, frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < 4 {
        return None;
    }
    let (body, crc) = frame.split_at(frame.len() - 2);
    if crc16(body) != u16::from_le_bytes([crc[0], crc[1]]) {
        return None;
    }
    let pdu = answer(handler, body[0], &body[1..]).await?;
    let mut out = vec![body[0]];
    out.extend(pdu);
    out.extend(crc16(&out).to_le_bytes());
    Some(out)
}

/// Takes every whole RTU request from the front of `buf`; returns the
/// answers to send.
async fn rtu_frames(handler: &impl Handler, buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut replies = Vec::new();
    while let Some(len) = rtu_request_len(buf) {
        if buf.len() < len {
            break;
        }
        let (body, crc) = buf[..len].split_at(len - 2);
        if crc16(body) != u16::from_le_bytes([crc[0], crc[1]]) {
            // Out of step with the frames: look for one a byte later.
            buf.remove(0);
            continue;
        }
        let frame: Vec<u8> = buf.drain(..len).collect();
        replies.extend(rtu_frame(handler, &frame).await);
    }
    replies
}

/// Takes every whole ASCII line from `buf`; returns the answers to send.
async fn ascii_frames(handler: &impl Handler, buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut replies = Vec::new();
    while let Some(end) = buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = buf.drain(..=end).collect();
        // A frame starts at its last ':'; anything before is noise.
        let Some(start) = line.iter().rposition(|b| *b == b':') else {
            continue;
        };
        let text = line[start + 1..]
            .strip_suffix(b"\r\n")
            .unwrap_or(&line[start + 1..]);
        let Ok((unit, pdu)) = ascii::decode_frame(text) else {
            continue;
        };
        if let Some(reply) = answer(handler, unit, &pdu).await {
            replies.push(ascii::encode_frame(unit, &reply));
        }
    }
    // A line far longer than any frame is noise.
    if buf.len() > 1024 {
        buf.clear();
    }
    replies
}

/// tokio-modbus's TCP server, answering through a [`Handler`].
struct TcpService<H>(H);

impl<H: Handler + Clone + 'static> tokio_modbus::server::Service for TcpService<H> {
    type Request = SlaveRequest<'static>;
    type Response = Option<Response>;
    type Exception = ExceptionCode;
    type Future = Pin<Box<dyn Future<Output = Result<Option<Response>, ExceptionCode>> + Send>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let handler = self.0.clone();
        Box::pin(async move {
            match handler.handle(req.slave, req.request).await {
                None => Ok(None),
                Some(answer) => answer.map(Some),
            }
        })
    }
}

/// Starts answering requests the way `listen` says: Modbus TCP or UDP, or
/// RTU over TCP or UDP, on a host and port (port 0 picks a free one), or RTU
/// or ASCII on a serial port. `handler(peer, broadcast)` makes the handler
/// for each client (or the serial line); `broadcast` is true where unit ID
/// 0 is a broadcast, as on a serial line and the gateways in front of one.
/// Returns where it listens, with the port it picked, and the task to abort
/// to stop.
pub async fn listen<H, F>(
    listen: Transport,
    handler: F,
) -> Result<(Transport, JoinHandle<()>), ModbusError>
where
    H: Handler + Clone + 'static,
    F: Fn(String, bool) -> H + Send + Sync + 'static,
{
    Ok(match listen {
        Transport::Tcp { host, port } => {
            let listener = TcpListener::bind((host.as_str(), port)).await?;
            let port = listener.local_addr()?.port();
            let server = Server::new(listener);
            let task = tokio::spawn(async move {
                let on_connected = |stream, peer: SocketAddr| {
                    let accepted = accept_tcp_connection(stream, peer, |peer| {
                        Ok(Some(TcpService(handler(peer.to_string(), false))))
                    });
                    async move { accepted }
                };
                // A client that sends garbage only loses its own connection.
                let _ = server.serve(&on_connected, |_| {}).await;
            });
            (Transport::Tcp { host, port }, task)
        }
        Transport::RtuOverTcp { host, port } => {
            let listener = TcpListener::bind((host.as_str(), port)).await?;
            let port = listener.local_addr()?.port();
            let task = tokio::spawn(async move {
                // Dropped with the task, which ends every connection.
                let mut clients = tokio::task::JoinSet::new();
                loop {
                    tokio::select! {
                        accepted = listener.accept() => match accepted {
                            Ok((stream, peer)) => {
                                // A gateway passes broadcasts on to its line.
                                let h = handler(peer.to_string(), true);
                                clients.spawn(serve(stream, Framing::Rtu, h));
                            }
                            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                        },
                        Some(_) = clients.join_next() => {}
                    }
                }
            });
            (Transport::RtuOverTcp { host, port }, task)
        }
        Transport::Udp { ref host, port } | Transport::RtuOverUdp { ref host, port } => {
            let socket = UdpSocket::bind((host.as_str(), port)).await?;
            let port = socket.local_addr()?.port();
            let rtu = matches!(listen, Transport::RtuOverUdp { .. });
            let task = tokio::spawn(async move {
                // One request per datagram; the longest ADU is 260 bytes.
                let mut buf = [0u8; 512];
                loop {
                    // On Windows a client that went away shows up here as
                    // an error: keep serving the others.
                    let Ok((n, peer)) = socket.recv_from(&mut buf).await else {
                        continue;
                    };
                    // RTU over UDP comes from gateways, which pass
                    // broadcasts on to their line.
                    let h = handler(peer.to_string(), rtu);
                    let reply = if rtu {
                        rtu_frame(&h, &buf[..n]).await
                    } else {
                        mbap_frame(&h, &buf[..n]).await
                    };
                    if let Some(reply) = reply {
                        let _ = socket.send_to(&reply, peer).await;
                    }
                }
            });
            let host = host.clone();
            let listen = if rtu {
                Transport::RtuOverUdp { host, port }
            } else {
                Transport::Udp { host, port }
            };
            (listen, task)
        }
        Transport::Rtu {
            ref port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        }
        | Transport::Ascii {
            ref port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        } => {
            let stream = modbus::open_serial(port, baudrate, parity, data_bits, stop_bits)?;
            let framing = match listen {
                Transport::Ascii { .. } => Framing::Ascii,
                _ => Framing::Rtu,
            };
            let h = handler(port.clone(), true);
            let task = tokio::spawn(async move {
                let _ = serve(stream, framing, h).await;
            });
            (listen, task)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Handler for Echo {
        async fn handle(
            &self,
            unit: u8,
            request: Request<'static>,
        ) -> Option<Result<Response, ExceptionCode>> {
            match (unit, request) {
                (1, Request::ReadHoldingRegisters(a, n)) => {
                    Some(Ok(Response::ReadHoldingRegisters((a..a + n).collect())))
                }
                (1, _) => Some(Err(ExceptionCode::IllegalFunction)),
                _ => None,
            }
        }
    }

    fn rtu(bytes: &[u8]) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out.extend(crc16(bytes).to_le_bytes());
        out
    }

    #[test]
    fn the_crc_matches_the_spec_example() {
        // Read 2 holding registers at 0 from unit 1: 01 03 00 00 00 02 C4 0B
        assert_eq!(crc16(&[1, 3, 0, 0, 0, 2]).to_le_bytes(), [0xC4, 0x0B]);
    }

    #[tokio::test]
    async fn rtu_frames_are_answered_and_noise_is_skipped() {
        let mut buf = vec![0x55];
        buf.extend(rtu(&[1, 3, 0, 4, 0, 2]));
        buf.extend(rtu(&[2, 3, 0, 0, 0, 1]));
        buf.extend(&rtu(&[1, 16, 0, 0, 0, 1, 2, 0, 9])[..5]);
        let replies = rtu_frames(&Echo, &mut buf).await;
        assert_eq!(replies, [rtu(&[1, 3, 4, 0, 4, 0, 5])]);
        // Half of a write is kept for when the rest comes.
        assert_eq!(buf.len(), 5);
        buf.extend(&rtu(&[1, 16, 0, 0, 0, 1, 2, 0, 9])[5..]);
        assert_eq!(rtu_frames(&Echo, &mut buf).await, [rtu(&[1, 0x90, 1])]);
        assert!(buf.is_empty());
    }

    #[tokio::test]
    async fn an_unknown_function_is_a_frame_after_silence() {
        let frame = rtu(&[1, 0x41, 7]);
        let mut buf = frame.clone();
        assert!(rtu_frames(&Echo, &mut buf).await.is_empty());
        assert_eq!(rtu_frame(&Echo, &frame).await, Some(rtu(&[1, 0xC1, 1])));
    }

    #[tokio::test]
    async fn ascii_lines_are_answered() {
        let mut buf = b"xx".to_vec();
        buf.extend(ascii::encode_frame(1, &[3, 0, 0, 0, 1]));
        buf.extend(ascii::encode_frame(9, &[3, 0, 0, 0, 1]));
        buf.extend(b":0103");
        let replies = ascii_frames(&Echo, &mut buf).await;
        assert_eq!(replies, [ascii::encode_frame(1, &[3, 2, 0, 0])]);
        assert_eq!(buf, b":0103");
    }

    #[tokio::test]
    async fn mbap_frames_keep_their_transaction_id() {
        let request = [0x12, 0x34, 0, 0, 0, 6, 1, 3, 0, 2, 0, 1];
        assert_eq!(
            mbap_frame(&Echo, &request).await,
            Some(vec![0x12, 0x34, 0, 0, 0, 5, 1, 3, 2, 0, 2])
        );
        // Another unit, a cut frame, or another protocol: no answer.
        let mut other = request;
        other[6] = 9;
        assert_eq!(mbap_frame(&Echo, &other).await, None);
        assert_eq!(mbap_frame(&Echo, &request[..10]).await, None);
        let mut protocol = request;
        protocol[3] = 1;
        assert_eq!(mbap_frame(&Echo, &protocol).await, None);
    }

    #[test]
    fn device_identification_is_encoded() {
        use tokio_modbus::prelude::{
            ConformityLevel, DeviceIdObject, ReadCode, ReadDeviceIdentificationResponse,
        };
        let r = Response::ReadDeviceIdentification(ReadDeviceIdentificationResponse {
            read_code: ReadCode::Basic,
            conformity_level: ConformityLevel::BasicIdentificationStreamOnly,
            more_follows: false,
            next_object_id: 0,
            device_id_objects: vec![DeviceIdObject {
                id: 0,
                value: b"AB".to_vec().into(),
            }],
        });
        assert_eq!(
            response_pdu(&r),
            [0x2B, 0x0E, 1, 1, 0, 0, 1, 0, 2, b'A', b'B']
        );
    }
}

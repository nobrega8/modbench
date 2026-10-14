//! Modbus ASCII client: each frame is `:`, the unit ID, the PDU and an LRC
//! as hexadecimal text, then CR LF. tokio-modbus has no ASCII framing, so
//! this plugs a client of its own into a tokio-modbus [`Context`].

use std::io;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_modbus::bytes::Bytes;
use tokio_modbus::client::{Client, Context};
use tokio_modbus::prelude::SlaveContext;
use tokio_modbus::{ExceptionResponse, Request, Response, Slave};

/// Longest frame: `:` + 2 x (unit + 253-byte PDU + LRC) + CR LF.
const MAX_FRAME: usize = 1 + 2 * 255 + 2;

/// Opens a Modbus ASCII client over a byte stream (a serial port).
pub fn attach_slave<T>(transport: T, slave: Slave) -> Context
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let client: Box<dyn Client> = Box::new(AsciiClient {
        transport,
        unit: slave.0,
    });
    Context::from(client)
}

/// Longitudinal redundancy check: the two's complement of the byte sum.
pub fn lrc(bytes: &[u8]) -> u8 {
    bytes
        .iter()
        .fold(0u8, |sum, b| sum.wrapping_add(*b))
        .wrapping_neg()
}

/// A whole ASCII frame for a unit ID and a PDU.
pub fn encode_frame(unit: u8, pdu: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(pdu.len() + 2);
    raw.push(unit);
    raw.extend_from_slice(pdu);
    raw.push(lrc(&raw));
    let mut out = Vec::with_capacity(raw.len() * 2 + 3);
    out.push(b':');
    for b in raw {
        out.extend_from_slice(format!("{b:02X}").as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out
}

/// The unit ID and PDU of a frame's text between `:` and CR LF, after
/// checking its LRC.
pub fn decode_frame(text: &[u8]) -> io::Result<(u8, Vec<u8>)> {
    let bad = |why: &str| io::Error::new(io::ErrorKind::InvalidData, why.to_string());
    if !text.len().is_multiple_of(2) || text.len() < 6 {
        return Err(bad("ASCII frame has a bad length"));
    }
    let bytes = text
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or_else(|| bad("ASCII frame has a character that is not hex"))
        })
        .collect::<io::Result<Vec<u8>>>()?;
    let (body, check) = bytes.split_at(bytes.len() - 1);
    if lrc(body) != check[0] {
        return Err(bad("ASCII frame failed its LRC check"));
    }
    Ok((body[0], body[1..].to_vec()))
}

/// The PDU of a request (function code and data), as on the wire.
pub fn request_pdu(request: &Request<'_>) -> io::Result<Vec<u8>> {
    use Request::*;
    let mut out = vec![request.function_code().value()];
    let mut put = |w: u16| out.extend_from_slice(&w.to_be_bytes());
    match request {
        ReadCoils(a, q)
        | ReadDiscreteInputs(a, q)
        | ReadInputRegisters(a, q)
        | ReadHoldingRegisters(a, q) => {
            put(*a);
            put(*q);
        }
        WriteSingleCoil(a, on) => {
            put(*a);
            put(if *on { 0xFF00 } else { 0 });
        }
        WriteSingleRegister(a, w) => {
            put(*a);
            put(*w);
        }
        WriteMultipleCoils(a, bits) => {
            put(*a);
            put(bits.len() as u16);
            let mut packed = vec![0u8; bits.len().div_ceil(8)];
            for (i, _) in bits.iter().enumerate().filter(|(_, on)| **on) {
                packed[i / 8] |= 1 << (i % 8);
            }
            out.push(packed.len() as u8);
            out.extend(packed);
        }
        WriteMultipleRegisters(a, words) => {
            put(*a);
            put(words.len() as u16);
            out.push((words.len() * 2) as u8);
            for w in words.iter() {
                out.extend_from_slice(&w.to_be_bytes());
            }
        }
        ReadDeviceIdentification(code, object) => {
            out.extend([0x0E, code.value(), *object]);
        }
        Custom(_, data) => out.extend_from_slice(data),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("{other:?} is not supported over Modbus ASCII"),
            ));
        }
    }
    Ok(out)
}

struct AsciiClient<T> {
    transport: T,
    unit: u8,
}

impl<T> std::fmt::Debug for AsciiClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AsciiClient")
    }
}

impl<T> SlaveContext for AsciiClient<T> {
    fn set_slave(&mut self, slave: Slave) {
        self.unit = slave.0;
    }
}

impl<T: AsyncRead + Unpin> AsciiClient<T> {
    /// Reads up to the next frame and returns its text between `:` and
    /// CR LF. Anything before the `:` is line noise and skipped.
    async fn read_frame(&mut self) -> io::Result<Vec<u8>> {
        let mut text = Vec::new();
        let mut started = false;
        loop {
            let b = self.transport.read_u8().await?;
            match b {
                b':' => {
                    // A new start: whatever came before was cut short.
                    started = true;
                    text.clear();
                }
                b'\n' if started => {
                    if text.last() == Some(&b'\r') {
                        text.pop();
                    }
                    return Ok(text);
                }
                _ if started => {
                    text.push(b);
                    if text.len() > MAX_FRAME {
                        started = false;
                        text.clear();
                    }
                }
                _ => {}
            }
        }
    }
}

#[async_trait]
impl<T> Client for AsciiClient<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send,
{
    async fn call(&mut self, request: Request<'_>) -> tokio_modbus::Result<Response> {
        let frame = encode_frame(self.unit, &request_pdu(&request)?);
        self.transport.write_all(&frame).await?;
        self.transport.flush().await?;
        let function = request.function_code().value();
        loop {
            // A frame with a bad LRC, or one from another unit, is not the
            // answer: keep listening until it comes or the caller times out.
            let Ok((unit, pdu)) = decode_frame(&self.read_frame().await?) else {
                continue;
            };
            if unit != self.unit || pdu.first().map(|f| f & 0x7F) != Some(function) {
                continue;
            }
            let bytes = Bytes::from(pdu);
            if bytes[0] & 0x80 != 0 {
                return Ok(Err(ExceptionResponse::try_from(bytes)?.exception));
            }
            return Ok(Ok(Response::try_from(bytes)?));
        }
    }

    async fn disconnect(&mut self) -> io::Result<()> {
        self.transport.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn frames_round_trip_with_their_lrc() {
        // Read 2 holding registers from 0x006B of unit 17 (spec example).
        let pdu = request_pdu(&Request::ReadHoldingRegisters(0x006B, 2)).unwrap();
        assert_eq!(pdu, [0x03, 0x00, 0x6B, 0x00, 0x02]);
        let frame = encode_frame(17, &pdu);
        assert_eq!(frame, b":1103006B00027F\r\n");
        let text = &frame[1..frame.len() - 2];
        assert_eq!(decode_frame(text).unwrap(), (17, pdu));
    }

    #[test]
    fn a_bad_lrc_or_text_is_refused() {
        assert!(decode_frame(b"1103006B000280").is_err());
        assert!(decode_frame(b"1103006B0002ZZ").is_err());
        assert!(decode_frame(b"110").is_err());
    }

    #[test]
    fn write_requests_are_encoded() {
        let coils = Request::WriteMultipleCoils(19, Cow::Owned(vec![true, false, true, true]));
        assert_eq!(request_pdu(&coils).unwrap(), [0x0F, 0, 19, 0, 4, 1, 0x0D]);
        let regs = Request::WriteMultipleRegisters(1, Cow::Owned(vec![0x000A, 0x0102]));
        assert_eq!(
            request_pdu(&regs).unwrap(),
            [0x10, 0, 1, 0, 2, 4, 0, 0x0A, 1, 2]
        );
        let coil = Request::WriteSingleCoil(172, true);
        assert_eq!(request_pdu(&coil).unwrap(), [0x05, 0, 172, 0xFF, 0]);
    }

    #[tokio::test]
    async fn a_client_reads_registers_over_a_stream() {
        let (client_side, mut device) = tokio::io::duplex(1024);
        let mut ctx = attach_slave(client_side, Slave(1));
        let device = tokio::spawn(async move {
            let mut buf = vec![0u8; 64];
            let n = device.read(&mut buf).await.unwrap();
            assert_eq!(&buf[..n], encode_frame(1, &[3, 0, 0, 0, 1]).as_slice());
            // Noise and another unit's answer come first; both are skipped.
            device.write_all(b"\x00garbage").await.unwrap();
            device
                .write_all(&encode_frame(2, &[3, 2, 0, 9]))
                .await
                .unwrap();
            device
                .write_all(&encode_frame(1, &[3, 2, 0x12, 0x34]))
                .await
                .unwrap();
        });
        use tokio_modbus::client::Reader;
        let words = ctx.read_holding_registers(0, 1).await.unwrap().unwrap();
        assert_eq!(words, [0x1234]);
        device.await.unwrap();
    }

    #[tokio::test]
    async fn an_exception_is_returned_as_such() {
        let (client_side, mut device) = tokio::io::duplex(1024);
        let mut ctx = attach_slave(client_side, Slave(1));
        tokio::spawn(async move {
            let mut buf = vec![0u8; 64];
            let _ = device.read(&mut buf).await.unwrap();
            device
                .write_all(&encode_frame(1, &[0x83, 0x02]))
                .await
                .unwrap();
        });
        use tokio_modbus::client::Reader;
        let answer = ctx.read_holding_registers(0, 1).await.unwrap();
        assert_eq!(answer, Err(tokio_modbus::ExceptionCode::IllegalDataAddress));
    }
}

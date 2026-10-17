//! A UDP socket seen as a byte stream, so tokio-modbus's TCP (MBAP) and RTU
//! framing work over UDP: each write is one datagram and each read takes
//! one. UDP has no connection to lose, but datagrams get lost: a request
//! with no answer is sent again, and answers that come too late are dropped
//! before the next request so they are not taken for its answer.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UdpSocket;
use tokio::time::Sleep;

/// Times a request is sent again when no answer comes.
pub const RESENDS: u32 = 2;

pub struct UdpStream {
    socket: UdpSocket,
    /// The last request, to send again.
    last: Vec<u8>,
    resends: u32,
    /// Wait before sending again; the timeout split over every try.
    wait: Duration,
    timer: Option<Pin<Box<Sleep>>>,
}

impl std::fmt::Debug for UdpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UdpStream")
    }
}

impl UdpStream {
    /// Sends to and receives from `peer` only. A request is sent up to
    /// [`RESENDS`] more times within `timeout`.
    /// Opens a socket bound to `local` (port 0 for any) that talks only to
    /// `peer`.
    pub async fn connect(
        peer: std::net::SocketAddr,
        local: std::net::SocketAddr,
        timeout: Duration,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(local).await?;
        socket.connect(peer).await?;
        Ok(Self {
            socket,
            last: Vec::new(),
            resends: 0,
            wait: timeout / (RESENDS + 1),
            timer: None,
        })
    }
}

impl AsyncRead for UdpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            match self.socket.poll_recv(cx, buf) {
                Poll::Ready(result) => {
                    self.timer = None;
                    return Poll::Ready(result);
                }
                Poll::Pending => {}
            }
            let this = &mut *self;
            let Some(timer) = this.timer.as_mut() else {
                return Poll::Pending;
            };
            if timer.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            if this.resends >= RESENDS {
                this.timer = None;
                return Poll::Pending;
            }
            // No answer yet: the request or its answer may be lost.
            this.resends += 1;
            let _ = this.socket.try_send(&this.last);
            let next = tokio::time::Instant::now() + this.wait;
            timer.as_mut().reset(next);
        }
    }
}

impl AsyncWrite for UdpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        // Answers to earlier requests that came after their timeout.
        let mut stale = [0u8; 512];
        while self.socket.try_recv(&mut stale).is_ok() {}
        let n = std::task::ready!(self.socket.poll_send(cx, data))?;
        let this = &mut *self;
        this.last = data[..n].to_vec();
        this.resends = 0;
        this.timer = Some(Box::pin(tokio::time::sleep(this.wait)));
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn a_lost_answer_is_asked_for_again() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        let mut stream =
            UdpStream::connect(addr, ([0, 0, 0, 0], 0).into(), Duration::from_millis(300))
                .await
                .unwrap();
        stream.write_all(b"ask").await.unwrap();
        let mut buf = [0u8; 16];
        let mut got = [0u8; 16];
        // The request is sent again while the client waits for an answer:
        // the first goes unanswered, as if its answer were lost.
        let device = async {
            let (n, peer) = server.recv_from(&mut buf).await.unwrap();
            assert_eq!(&buf[..n], b"ask");
            let (n, _) = server.recv_from(&mut buf).await.unwrap();
            assert_eq!(&buf[..n], b"ask");
            server.send_to(b"answer", peer).await.unwrap();
            peer
        };
        let (peer, n) = tokio::join!(device, stream.read(&mut got));
        assert_eq!(&got[..n.unwrap()], b"answer");

        // A late duplicate answer is dropped before the next request.
        server.send_to(b"late", peer).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        stream.write_all(b"next").await.unwrap();
        let (n, _) = server.recv_from(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"next");
        server.send_to(b"fresh", peer).await.unwrap();
        let n = stream.read(&mut got).await.unwrap();
        assert_eq!(&got[..n], b"fresh");
    }
}

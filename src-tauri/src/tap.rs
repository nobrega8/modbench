//! Transport wrapper that reports every byte sent and received, so the UI
//! can show the raw Modbus frames.

use std::fmt;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Out,
    In,
}

pub type RawSink = Arc<dyn Fn(Direction, &[u8]) + Send + Sync>;

pub struct Tap<T> {
    inner: T,
    sink: Option<RawSink>,
}

impl<T> Tap<T> {
    pub fn new(inner: T, sink: Option<RawSink>) -> Self {
        Self { inner, sink }
    }
}

impl<T> fmt::Debug for Tap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tap")
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for Tap<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let (Poll::Ready(Ok(())), Some(sink)) = (&result, &self.sink) {
            let new = &buf.filled()[before..];
            if !new.is_empty() {
                sink(Direction::In, new);
            }
        }
        result
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for Tap<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, data);
        if let (Poll::Ready(Ok(n)), Some(sink)) = (&result, &self.sink)
            && *n > 0
        {
            sink(Direction::Out, &data[..*n]);
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

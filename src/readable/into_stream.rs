use core::pin::Pin;

use futures::future::Future;
use futures::ready;
use futures::stream::{FusedStream, Stream};
use futures::task::{Context, Poll};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::sys::ReadableStreamReadResult;
use super::ReadableStreamDefaultReader;

/// A [`Stream`](Stream) for the [`into_stream`](super::ReadableStream::into_stream) method.
///
/// This stream holds a reader, and therefore locks the [`ReadableStream`](super::ReadableStream).
/// When this stream is dropped, it also drops its reader which in turn
/// [releases its lock](https://streams.spec.whatwg.org/#release-a-lock).
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct IntoStream<'reader> {
    reader: Option<ReadableStreamDefaultReader<'reader>>,
    fut: Option<JsFuture>,
}

impl<'reader> IntoStream<'reader> {
    #[inline]
    pub(super) fn new(reader: ReadableStreamDefaultReader) -> IntoStream {
        IntoStream {
            reader: Some(reader),
            fut: None,
        }
    }
}

impl FusedStream for IntoStream<'_> {
    fn is_terminated(&self) -> bool {
        self.reader.is_none() && self.fut.is_none()
    }
}

impl<'reader> Stream for IntoStream<'reader> {
    type Item = Result<JsValue, JsValue>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.fut.is_none() {
            // No pending read, start reading the next chunk
            match self.reader.as_ref() {
                Some(reader) => {
                    // Read a chunk and store its future
                    let fut = JsFuture::from(reader.as_raw().read());
                    self.as_mut().fut = Some(fut);
                }
                None => {
                    // Reader was already dropped
                    return Poll::Ready(None);
                }
            }
        }

        // Poll the future for the pending read
        let js_result = ready!(Pin::new(self.as_mut().fut.as_mut().unwrap_throw()).poll(cx));
        self.as_mut().fut = None;

        // Read completed
        Poll::Ready(match js_result {
            Ok(js_value) => {
                let result = ReadableStreamReadResult::from(js_value);
                if result.is_done() {
                    // End of stream, drop reader
                    self.as_mut().reader = None;
                    None
                } else {
                    Some(Ok(result.value()))
                }
            }
            Err(js_value) => {
                // Error, drop reader
                self.as_mut().reader = None;
                Some(Err(js_value))
            }
        })
    }
}

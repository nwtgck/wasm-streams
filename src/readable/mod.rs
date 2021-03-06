//! Bindings and conversions for
//! [readable streams](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStream).
use std::marker::PhantomData;

use futures::stream::Stream;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{throw_val, JsCast};
use wasm_bindgen_futures::JsFuture;

pub use into_stream::IntoStream;
use into_underlying_source::IntoUnderlyingSource;
pub use pipe_options::PipeOptions;

use crate::queuing_strategy::QueuingStrategy;
use crate::util::promise_to_void_future;
use crate::writable::WritableStream;

mod into_stream;
mod into_underlying_source;
mod pipe_options;
pub mod sys;

/// A [`ReadableStream`](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStream).
///
/// `ReadableStream`s can be created from a [raw JavaScript stream](sys::ReadableStream) with
/// [`from_raw`](Self::from_raw), or from a Rust [`Stream`](Stream)
/// with [`from_stream`](Self::from_stream).
///
/// They can be converted into a [raw JavaScript stream](sys::ReadableStream) with
/// [`into_raw`](Self::into_raw), or into a Rust [`Stream`](Stream)
/// with [`into_stream`](Self::into_stream).
#[derive(Debug)]
pub struct ReadableStream {
    raw: sys::ReadableStream,
}

impl ReadableStream {
    /// Creates a new `ReadableStream` from a [JavaScript stream](sys::ReadableStream).
    #[inline]
    pub fn from_raw(raw: sys::ReadableStream) -> Self {
        Self { raw }
    }

    /// Creates a new `ReadableStream` from a [`Stream`](Stream).
    ///
    /// Items and errors must be represented as raw [`JsValue`](JsValue)s.
    /// Use [`map`](futures::StreamExt::map), [`map_ok`](futures::TryStreamExt::map_ok) and/or
    /// [`map_err`](futures::TryStreamExt::map_err) to convert a stream's items to a `JsValue`
    /// before passing it to this function.
    pub fn from_stream<St>(stream: St) -> Self
    where
        St: Stream<Item = Result<JsValue, JsValue>> + 'static,
    {
        let source = IntoUnderlyingSource::new(Box::new(stream));
        // Set HWM to 0 to prevent the JS ReadableStream from buffering chunks in its queue,
        // since the original Rust stream is better suited to handle that.
        let strategy = QueuingStrategy::new(0.0);
        let raw = sys::ReadableStream::new_with_source(source, strategy);
        Self { raw }
    }

    /// Acquires a reference to the underlying [JavaScript stream](sys::ReadableStream).
    #[inline]
    pub fn as_raw(&self) -> &sys::ReadableStream {
        &self.raw
    }

    /// Consumes this `ReadableStream`, returning the underlying [JavaScript stream](sys::ReadableStream).
    #[inline]
    pub fn into_raw(self) -> sys::ReadableStream {
        self.raw
    }

    /// Returns `true` if the stream is [locked to a reader](https://streams.spec.whatwg.org/#lock).
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.as_raw().is_locked()
    }

    /// [Cancels](https://streams.spec.whatwg.org/#cancel-a-readable-stream) the stream,
    /// signaling a loss of interest in the stream by a consumer.
    ///
    /// If the stream is currently locked to a reader, then this returns an error.
    pub async fn cancel(&mut self) -> Result<(), JsValue> {
        promise_to_void_future(self.as_raw().cancel()).await
    }

    /// [Cancels](https://streams.spec.whatwg.org/#cancel-a-readable-stream) the stream,
    /// signaling a loss of interest in the stream by a consumer.
    ///
    /// The supplied `reason` will be given to the underlying source, which may or may not use it.
    ///
    /// If the stream is currently locked to a reader, then this returns an error.
    pub async fn cancel_with_reason(&mut self, reason: &JsValue) -> Result<(), JsValue> {
        promise_to_void_future(self.as_raw().cancel_with_reason(reason)).await
    }

    /// Creates a [default reader](ReadableStreamDefaultReader) and
    /// [locks](https://streams.spec.whatwg.org/#lock) the stream to the new reader.
    ///
    /// While the stream is locked, no other reader can be acquired until this one is released.
    ///
    /// **Panics** if the stream is already locked to a reader. For a non-panicking variant,
    /// use [`try_get_reader`](Self::try_get_reader).
    #[inline]
    pub fn get_reader(&mut self) -> ReadableStreamDefaultReader {
        self.try_get_reader()
            .expect_throw("already locked to a reader")
    }

    /// Try to create a [default reader](ReadableStreamDefaultReader) and
    /// [lock](https://streams.spec.whatwg.org/#lock) the stream to the new reader.
    ///
    /// While the stream is locked, no other reader can be acquired until this one is released.
    ///
    /// If the stream is already locked to a reader, then this returns an error.
    pub fn try_get_reader(&mut self) -> Result<ReadableStreamDefaultReader, js_sys::Error> {
        Ok(ReadableStreamDefaultReader {
            raw: self.as_raw().get_reader()?,
            _stream: PhantomData,
        })
    }

    /// [Pipes](https://streams.spec.whatwg.org/#piping) this readable stream to a given
    /// writable stream.
    ///
    /// Piping a stream will [lock](https://streams.spec.whatwg.org/#lock) it for the duration
    /// of the pipe, preventing any other consumer from acquiring a reader.
    ///
    /// This returns `()` if the pipe completes successfully, or `Err(error)` if any `error`
    /// was encountered during the process.
    pub async fn pipe_to<'a>(&'a mut self, dest: &'a mut WritableStream) -> Result<(), JsValue> {
        self.pipe_to_with_options(dest, &PipeOptions::default())
            .await
    }

    /// [Pipes](https://streams.spec.whatwg.org/#piping) this readable stream to a given
    /// writable stream.
    ///
    /// Piping a stream will [lock](https://streams.spec.whatwg.org/#lock) it for the duration
    /// of the pipe, preventing any other consumer from acquiring a reader.
    ///
    /// Errors and closures of the source and destination streams propagate as follows:
    /// * An error in the source readable stream will [abort](https://streams.spec.whatwg.org/#abort-a-writable-stream)
    ///   the destination writable stream, unless [`options.prevent_abort`](PipeOptions::prevent_abort)
    ///   is `true`.
    /// * An error in the destination writable stream will [cancel](https://streams.spec.whatwg.org/#cancel-a-readable-stream)
    ///   the source readable stream, unless [`options.prevent_cancel`](PipeOptions::prevent_cancel)
    ///   is `true`.
    /// * When the source readable stream closes, the destination writable stream will be closed,
    ///   unless [`options.prevent_close`](PipeOptions::prevent_close) is `true`.
    /// * If the destination writable stream starts out closed or closing, the source readable stream
    ///   will be [canceled](https://streams.spec.whatwg.org/#cancel-a-readable-stream),
    ///   unless unless [`options.prevent_cancel`](PipeOptions::prevent_cancel) is `true`.
    ///
    /// This returns `()` if the pipe completes successfully, or `Err(error)` if any `error`
    /// was encountered during the process.
    pub async fn pipe_to_with_options<'a>(
        &'a mut self,
        dest: &'a mut WritableStream,
        options: &PipeOptions,
    ) -> Result<(), JsValue> {
        let promise = self
            .as_raw()
            .pipe_to(dest.as_raw(), options.clone().into_raw());
        promise_to_void_future(promise).await
    }

    /// [Tees](https://streams.spec.whatwg.org/#tee-a-readable-stream) this readable stream,
    /// returning the two resulting branches as new [`ReadableStream`](ReadableStream) instances.
    ///
    /// Teeing a stream will [lock](https://streams.spec.whatwg.org/#lock) it, preventing any other
    /// consumer from acquiring a reader.
    /// To [cancel](https://streams.spec.whatwg.org/#cancel-a-readable-stream) the stream,
    /// cancel both of the resulting branches; a composite cancellation reason will then be
    /// propagated to the stream's underlying source.
    ///
    /// Note that the chunks seen in each branch will be the same object.
    /// If the chunks are not immutable, this could allow interference between the two branches.
    ///
    /// **Panics** if the stream is already locked to a reader. For a non-panicking variant,
    /// use [`try_tee`](Self::try_tee).
    pub fn tee(self) -> (ReadableStream, ReadableStream) {
        self.try_tee().expect_throw("already locked to a reader")
    }

    /// Tries to [tee](https://streams.spec.whatwg.org/#tee-a-readable-stream) this readable stream,
    /// returning the two resulting branches as new [`ReadableStream`](ReadableStream) instances.
    ///
    /// Teeing a stream will [lock](https://streams.spec.whatwg.org/#lock) it, preventing any other
    /// consumer from acquiring a reader.
    /// To [cancel](https://streams.spec.whatwg.org/#cancel-a-readable-stream) the stream,
    /// cancel both of the resulting branches; a composite cancellation reason will then be
    /// propagated to the stream's underlying source.
    ///
    /// Note that the chunks seen in each branch will be the same object.
    /// If the chunks are not immutable, this could allow interference between the two branches.
    ///
    /// If the stream is already locked to a reader, then this returns an error
    /// along with the original `ReadableStream`.
    pub fn try_tee(self) -> Result<(ReadableStream, ReadableStream), (js_sys::Error, Self)> {
        let branches = match self.as_raw().tee() {
            Ok(branches) => branches,
            Err(err) => return Err((err, self)),
        };
        debug_assert_eq!(branches.length(), 2);
        let (left, right) = (branches.get(0), branches.get(1));
        Ok((
            Self::from_raw(left.unchecked_into()),
            Self::from_raw(right.unchecked_into()),
        ))
    }

    /// Converts this `ReadableStream` into a [`Stream`](Stream).
    ///
    /// Items and errors are represented by their raw [`JsValue`](JsValue).
    /// Use [`map`](futures::StreamExt::map), [`map_ok`](futures::TryStreamExt::map_ok) and/or
    /// [`map_err`](futures::TryStreamExt::map_err) on the returned stream to convert them to a more
    /// appropriate type.
    ///
    /// **Panics** if the stream is already locked to a reader. For a non-panicking variant,
    /// use [`try_into_stream`](Self::try_into_stream).
    #[inline]
    pub fn into_stream(self) -> IntoStream<'static> {
        self.try_into_stream()
            .expect_throw("already locked to a reader")
    }

    /// Try to convert this `ReadableStream` into a [`Stream`](Stream).
    ///
    /// Items and errors are represented by their raw [`JsValue`](JsValue).
    /// Use [`map`](futures::StreamExt::map), [`map_ok`](futures::TryStreamExt::map_ok) and/or
    /// [`map_err`](futures::TryStreamExt::map_err) on the returned stream to convert them to a more
    /// appropriate type.
    ///
    /// If the stream is already locked to a reader, then this returns an error
    /// along with the original `ReadableStream`.
    pub fn try_into_stream(self) -> Result<IntoStream<'static>, (js_sys::Error, Self)> {
        let raw_reader = match self.as_raw().get_reader() {
            Ok(raw_reader) => raw_reader,
            Err(err) => return Err((err, self)),
        };
        let reader = ReadableStreamDefaultReader {
            raw: raw_reader,
            _stream: PhantomData,
        };
        Ok(reader.into_stream())
    }
}

impl<St> From<St> for ReadableStream
where
    St: Stream<Item = Result<JsValue, JsValue>> + 'static,
{
    /// Equivalent to [`from_stream`](Self::from_stream).
    #[inline]
    fn from(stream: St) -> Self {
        Self::from_stream(stream)
    }
}

/// A [`ReadableStreamDefaultReader`](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStreamDefaultReader)
/// that can be used to read chunks from a [`ReadableStream`](ReadableStream).
///
/// This is returned by the [`get_reader`](ReadableStream::get_reader) method.
///
/// When the reader is dropped, it automatically [releases its lock](https://streams.spec.whatwg.org/#release-a-lock).
#[derive(Debug)]
pub struct ReadableStreamDefaultReader<'stream> {
    raw: sys::ReadableStreamDefaultReader,
    _stream: PhantomData<&'stream mut ReadableStream>,
}

impl<'stream> ReadableStreamDefaultReader<'stream> {
    /// Acquires a reference to the underlying [JavaScript reader](sys::ReadableStreamDefaultReader).
    #[inline]
    pub fn as_raw(&self) -> &sys::ReadableStreamDefaultReader {
        &self.raw
    }

    /// Waits for the stream to become closed.
    ///
    /// This returns an error if the stream ever errors, or if the reader's lock is
    /// [released](https://streams.spec.whatwg.org/#release-a-lock) before the stream finishes
    /// closing.
    pub async fn closed(&self) -> Result<(), JsValue> {
        promise_to_void_future(self.as_raw().closed()).await
    }

    /// [Cancels](https://streams.spec.whatwg.org/#cancel-a-readable-stream) the stream,
    /// signaling a loss of interest in the stream by a consumer.
    ///
    /// Equivalent to [`ReadableStream.cancel`](ReadableStream::cancel).
    pub async fn cancel(&mut self) -> Result<(), JsValue> {
        promise_to_void_future(self.as_raw().cancel()).await
    }

    /// [Cancels](https://streams.spec.whatwg.org/#cancel-a-readable-stream) the stream,
    /// signaling a loss of interest in the stream by a consumer.
    ///
    /// Equivalent to [`ReadableStream.cancel_with_reason`](ReadableStream::cancel_with_reason).
    pub async fn cancel_with_reason(&mut self, reason: &JsValue) -> Result<(), JsValue> {
        promise_to_void_future(self.as_raw().cancel_with_reason(reason)).await
    }

    /// Reads the next chunk from the stream's internal queue.
    ///
    /// * If a next `chunk` becomes available, this returns `Ok(Some(chunk))`.
    /// * If the stream closes and no more chunks are available, this returns `Ok(None)`.
    /// * If the stream encounters an `error`, this returns `Err(error)`.
    pub async fn read(&mut self) -> Result<Option<JsValue>, JsValue> {
        let promise = self.as_raw().read();
        let js_value = JsFuture::from(promise).await?;
        let result = sys::ReadableStreamReadResult::from(js_value);
        if result.is_done() {
            Ok(None)
        } else {
            Ok(Some(result.value()))
        }
    }

    /// [Releases](https://streams.spec.whatwg.org/#release-a-lock) this reader's lock on the
    /// corresponding stream.
    ///
    /// **Panics** if the reader still has a pending read request, i.e. if a future returned
    /// by [`read`](Self::read) is not yet ready. For a non-panicking variant,
    /// use [`try_release_lock`](Self::try_release_lock).
    #[inline]
    pub fn release_lock(mut self) {
        self.release_lock_mut()
    }

    fn release_lock_mut(&mut self) {
        self.as_raw()
            .release_lock()
            .unwrap_or_else(|error| throw_val(error.into()))
    }

    /// Try to [release](https://streams.spec.whatwg.org/#release-a-lock) this reader's lock on the
    /// corresponding stream.
    ///
    /// The lock cannot be released while the reader still has a pending read request, i.e.
    /// if a future returned by [`read`](Self::read) is not yet ready. Attempting to do so will
    /// return an error and leave the reader locked to the stream.
    #[inline]
    pub fn try_release_lock(self) -> Result<(), (js_sys::Error, Self)> {
        self.as_raw().release_lock().map_err(|error| (error, self))
    }

    /// Converts this `ReadableStreamDefaultReader` into a [`Stream`](Stream).
    ///
    /// This is similar to [`ReadableStream.into_stream`](ReadableStream::into_stream),
    /// except that after the returned `Stream` is dropped, the original `ReadableStream` is still
    /// usable. This allows reading only a few chunks from the `Stream`, while still allowing
    /// another reader to read the remaining chunks later on.
    #[inline]
    pub fn into_stream(self) -> IntoStream<'stream> {
        IntoStream::new(self)
    }
}

impl Drop for ReadableStreamDefaultReader<'_> {
    fn drop(&mut self) {
        self.release_lock_mut();
    }
}

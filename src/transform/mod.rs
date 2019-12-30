use std::marker::PhantomData;

use wasm_bindgen::JsValue;

use crate::readable::ReadableStream;
use crate::writable::WritableStream;

pub mod sys;

pub struct TransformStream<I, O> {
    raw: sys::TransformStream,
    _writable_type: PhantomData<I>,
    _readable_type: PhantomData<O>,
}

impl<I, O> TransformStream<I, O> {
    #[inline]
    pub fn as_raw(&self) -> &sys::TransformStream {
        &self.raw
    }

    #[inline]
    pub fn into_raw(self) -> sys::TransformStream {
        self.raw
    }
}

impl<I: Into<JsValue>, O: From<JsValue>> TransformStream<I, O> {
    pub fn readable(&self) -> ReadableStream<O> {
        ReadableStream::from(self.raw.readable())
    }

    pub fn writable(&self) -> WritableStream<I> {
        WritableStream::from(self.raw.writable())
    }
}

impl<I: Into<JsValue>, O: From<JsValue>> From<sys::TransformStream> for TransformStream<I, O> {
    fn from(raw: sys::TransformStream) -> TransformStream<I, O> {
        TransformStream {
            raw,
            _writable_type: PhantomData,
            _readable_type: PhantomData,
        }
    }
}

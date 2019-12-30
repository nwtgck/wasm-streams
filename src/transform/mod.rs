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

impl<I, O> TransformStream<I, O>
where
    I: Into<JsValue>,
    O: From<JsValue>,
{
    #[inline]
    pub fn as_raw(&self) -> &sys::TransformStream {
        &self.raw
    }

    pub fn readable(&self) -> ReadableStream<O> {
        ReadableStream::from(self.raw.readable())
    }

    pub fn writable(&self) -> WritableStream {
        WritableStream::from(self.raw.writable())
    }

    pub fn into_raw(self) -> sys::TransformStream {
        self.raw
    }
}

impl<I, O> From<sys::TransformStream> for TransformStream<I, O>
where
    I: Into<JsValue>,
    O: From<JsValue>,
{
    fn from(raw: sys::TransformStream) -> TransformStream<I, O> {
        TransformStream {
            raw,
            _writable_type: PhantomData,
            _readable_type: PhantomData,
        }
    }
}

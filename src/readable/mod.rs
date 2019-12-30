use std::marker::PhantomData;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

pub use into_stream::IntoStream;

mod into_stream;
pub mod sys;

pub struct ReadableStream<T> {
    raw: sys::ReadableStream,
    _item_type: PhantomData<T>,
}

impl<T: From<JsValue>> ReadableStream<T> {
    #[inline]
    pub fn as_raw(&self) -> &sys::ReadableStream {
        &self.raw
    }

    #[inline]
    pub fn into_raw(self) -> sys::ReadableStream {
        self.raw
    }

    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    pub async fn cancel(&mut self) -> Result<(), JsValue> {
        let js_value = JsFuture::from(self.raw.cancel()).await?;
        debug_assert!(js_value.is_undefined());
        Ok(())
    }

    pub async fn cancel_with_reason(&mut self, reason: &JsValue) -> Result<(), JsValue> {
        let js_value = JsFuture::from(self.raw.cancel_with_reason(reason)).await?;
        debug_assert!(js_value.is_undefined());
        Ok(())
    }

    pub fn get_reader(&mut self) -> Result<ReadableStreamDefaultReader<'_, T>, JsValue> {
        Ok(ReadableStreamDefaultReader {
            raw: Some(self.raw.get_reader()?),
            _stream: PhantomData,
        })
    }
}

impl<T: From<JsValue>> From<sys::ReadableStream> for ReadableStream<T> {
    fn from(raw: sys::ReadableStream) -> ReadableStream<T> {
        ReadableStream {
            raw,
            _item_type: PhantomData,
        }
    }
}

pub struct ReadableStreamDefaultReader<'stream, T> {
    raw: Option<sys::ReadableStreamDefaultReader>,
    _stream: PhantomData<&'stream mut ReadableStream<T>>,
}

impl<'stream, T> ReadableStreamDefaultReader<'stream, T> {
    #[inline]
    pub fn as_raw(&self) -> &sys::ReadableStreamDefaultReader {
        self.raw.as_ref().unwrap()
    }

    pub async fn closed(&self) -> Result<(), JsValue> {
        let js_value = JsFuture::from(self.as_raw().closed()).await?;
        debug_assert!(js_value.is_undefined());
        Ok(())
    }

    pub async fn cancel(&mut self) -> Result<(), JsValue> {
        let js_value = JsFuture::from(self.as_raw().cancel()).await?;
        debug_assert!(js_value.is_undefined());
        Ok(())
    }

    pub async fn cancel_with_reason(&mut self, reason: &JsValue) -> Result<(), JsValue> {
        let js_value = JsFuture::from(self.as_raw().cancel_with_reason(reason)).await?;
        debug_assert!(js_value.is_undefined());
        Ok(())
    }

    pub fn release_lock(&mut self) -> Result<(), JsValue> {
        if let Some(raw) = self.raw.as_ref() {
            raw.release_lock()?;
            self.raw.take();
        }
        Ok(())
    }
}

impl<'stream, T: From<JsValue>> ReadableStreamDefaultReader<'stream, T> {
    pub async fn read(&mut self) -> Result<Option<T>, JsValue> {
        let js_value = JsFuture::from(self.as_raw().read()).await?;
        let result = sys::ReadableStreamReadResult::from(js_value);
        if result.is_done() {
            Ok(None)
        } else {
            Ok(Some(T::from(result.value())))
        }
    }

    pub fn into_stream(self) -> IntoStream<'stream, T> {
        IntoStream::new(self)
    }
}

impl<T> Drop for ReadableStreamDefaultReader<'_, T> {
    fn drop(&mut self) {
        // TODO Error handling?
        self.release_lock().unwrap();
    }
}

use std::cell::RefCell;
use std::rc::Rc;

use futures::{Stream, TryFutureExt, TryStreamExt};
use futures::stream::unfold;
use js_sys::{Object, Promise};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use async_trait::async_trait;
use sys::{
    ReadableStream as RawReadableStream,
    ReadableStreamDefaultReader as RawReadableStreamDefaultReader,
    ReadableStreamReadResult,
    UnderlyingSource as RawUnderlyingSource,
};
pub use sys::ReadableStreamDefaultController;

pub mod sys;

pub struct ReadableStream {
    inner: RawReadableStream,
    _source: JsUnderlyingSource,
}

impl ReadableStream {
    pub fn new(source: Box<dyn UnderlyingSource + 'static>) -> ReadableStream {
        let source = JsUnderlyingSource::new(source);
        let inner = RawReadableStream::new_with_source(source.as_raw());
        ReadableStream {
            inner,
            _source: source,
        }
    }

    #[inline]
    pub fn as_raw(&self) -> &RawReadableStream {
        &self.inner
    }

    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    pub async fn cancel(&mut self) -> Result<(), JsValue> {
        let js_value = JsFuture::from(self.inner.cancel()).await?;
        debug_assert!(js_value.is_undefined());
        Ok(())
    }

    pub async fn cancel_with_reason(&mut self, reason: &JsValue) -> Result<(), JsValue> {
        let js_value = JsFuture::from(self.inner.cancel_with_reason(reason)).await?;
        debug_assert!(js_value.is_undefined());
        Ok(())
    }

    pub fn get_reader(&mut self) -> Result<ReadableStreamDefaultReader, JsValue> {
        Ok(ReadableStreamDefaultReader {
            inner: Some(self.inner.get_reader()?)
        })
    }

    pub fn forget(self) -> RawReadableStream {
        self._source.forget();
        self.inner
    }
}

#[async_trait(? Send)]
pub trait UnderlyingSource {
    async fn start(&mut self, controller: &ReadableStreamDefaultController) -> Result<(), JsValue> {
        let _ = controller;
        Ok(())
    }

    async fn pull(&mut self, controller: &ReadableStreamDefaultController) -> Result<(), JsValue> {
        let _ = controller;
        Ok(())
    }

    async fn cancel(&mut self, reason: &JsValue) -> Result<(), JsValue> {
        let _ = reason;
        Ok(())
    }
}

struct JsUnderlyingSource {
    inner: RawUnderlyingSource,
    start_closure: Closure<dyn FnMut(ReadableStreamDefaultController) -> Promise>,
    pull_closure: Closure<dyn FnMut(ReadableStreamDefaultController) -> Promise>,
    cancel_closure: Closure<dyn FnMut(JsValue) -> Promise>,
}

impl JsUnderlyingSource {
    pub fn new(source: Box<dyn UnderlyingSource + 'static>) -> JsUnderlyingSource {
        let source = Rc::new(RefCell::new(source));

        let start_closure = {
            let source = source.clone();
            Closure::wrap(Box::new(move |controller: ReadableStreamDefaultController| {
                let source = source.clone();
                future_to_promise(async move {
                    // This mutable borrow can never panic, since the ReadableStream always
                    // queues each operation on the underlying source.
                    let mut source = source.borrow_mut();
                    source.start(&controller).await?;
                    Ok(JsValue::undefined())
                })
            }) as Box<dyn FnMut(ReadableStreamDefaultController) -> Promise>)
        };
        let pull_closure = {
            let source = source.clone();
            Closure::wrap(Box::new(move |controller: ReadableStreamDefaultController| {
                let source = source.clone();
                future_to_promise(async move {
                    let mut source = source.borrow_mut();
                    source.pull(&controller).await?;
                    Ok(JsValue::undefined())
                })
            }) as Box<dyn FnMut(ReadableStreamDefaultController) -> Promise>)
        };
        let cancel_closure = {
            let source = source.clone();
            Closure::wrap(Box::new(move |reason: JsValue| {
                let source = source.clone();
                future_to_promise(async move {
                    let mut source = source.borrow_mut();
                    source.cancel(&reason).await?;
                    Ok(JsValue::undefined())
                })
            }) as Box<dyn FnMut(JsValue) -> Promise>)
        };

        let inner = RawUnderlyingSource::from(JsValue::from(Object::new()));
        inner.set_start(&start_closure);
        inner.set_pull(&pull_closure);
        inner.set_cancel(&cancel_closure);

        JsUnderlyingSource {
            inner,
            start_closure,
            pull_closure,
            cancel_closure,
        }
    }

    #[inline]
    pub fn as_raw(&self) -> &RawUnderlyingSource {
        &self.inner
    }

    pub fn forget(self) -> RawUnderlyingSource {
        self.start_closure.forget();
        self.pull_closure.forget();
        self.cancel_closure.forget();
        self.inner
    }
}

pub struct ReadableStreamDefaultReader {
    inner: Option<RawReadableStreamDefaultReader>
}

impl ReadableStreamDefaultReader {
    #[inline]
    pub fn as_raw(&self) -> &RawReadableStreamDefaultReader {
        self.inner.as_ref().unwrap()
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

    pub async fn read(&mut self) -> Result<Option<JsValue>, JsValue> {
        let js_value = JsFuture::from(self.as_raw().read()).await?;
        let result = ReadableStreamReadResult::from(js_value);
        if result.is_done() {
            Ok(None)
        } else {
            Ok(Some(result.value()))
        }
    }

    pub fn release_lock(&mut self) -> Result<(), JsValue> {
        if let Some(inner) = self.inner.as_ref() {
            inner.release_lock()?;
            self.inner.take();
        }
        Ok(())
    }
}

impl Drop for ReadableStreamDefaultReader {
    fn drop(&mut self) {
        // TODO Error handling?
        self.release_lock().unwrap();
    }
}

impl ReadableStream {
    pub fn into_stream(self) -> impl Stream<Item=Result<JsValue, JsValue>> {
        self.into_stream_fut().try_flatten_stream().into_stream()
    }

    async fn into_stream_fut(mut self) -> Result<impl Stream<Item=Result<JsValue, JsValue>>, JsValue> {
        let reader = self.get_reader()?;
        let stream = unfold(Some(reader), |state| async move {
            let mut reader = state?;
            match reader.read().await {
                Ok(Some(value)) => Some((Ok(value), Some(reader))),
                Ok(None) => None,
                Err(error) => Some((Err(error), None))
            }
        });
        Ok(stream)
    }
}
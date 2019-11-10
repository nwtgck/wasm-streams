use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use js_sys::{Object, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use async_trait::async_trait;
pub use into_stream::IntoStream;

mod into_stream;
pub mod sys;

pub struct ReadableStream<T: From<JsValue> + AsRef<JsValue> + 'static = JsValue> {
    raw: sys::ReadableStream,
    _source: Option<JsUnderlyingSource<T>>,
}

impl<T: From<JsValue> + AsRef<JsValue> + 'static> ReadableStream<T> {
    pub fn new(source: Box<dyn UnderlyingSource<Item=T> + 'static>) -> ReadableStream<T> {
        let source = JsUnderlyingSource::new(source);
        let raw = sys::ReadableStream::new_with_source(source.as_raw());
        ReadableStream {
            raw,
            _source: Some(source),
        }
    }

    #[inline]
    pub fn from_raw(raw: sys::ReadableStream) -> ReadableStream<T> {
        ReadableStream {
            raw,
            _source: None,
        }
    }

    #[inline]
    pub fn as_raw(&self) -> &sys::ReadableStream {
        &self.raw
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

    pub fn forget(self) -> sys::ReadableStream {
        if let Some(source) = self._source {
            source.forget();
        }
        self.raw
    }
}

impl From<sys::ReadableStream> for ReadableStream<JsValue> {
    fn from(raw: sys::ReadableStream) -> ReadableStream<JsValue> {
        ReadableStream::from_raw(raw)
    }
}

pub struct ReadableStreamDefaultController<T: AsRef<JsValue> = JsValue> {
    raw: sys::ReadableStreamDefaultController,
    item_type: PhantomData<T>,
}

impl<T: AsRef<JsValue>> ReadableStreamDefaultController<T> {
    #[inline]
    pub fn from_raw(raw: sys::ReadableStreamDefaultController) -> ReadableStreamDefaultController<T> {
        ReadableStreamDefaultController {
            raw,
            item_type: PhantomData,
        }
    }

    #[inline]
    pub fn as_raw(&self) -> &sys::ReadableStreamDefaultController {
        &self.raw
    }

    pub fn desired_size(&self) -> Option<f64> {
        self.raw.desired_size()
    }

    pub fn close(&self) {
        self.raw.close()
    }

    pub fn enqueue(&self, chunk: &T) {
        self.raw.enqueue(chunk.as_ref())
    }

    pub fn error(&self, error: &JsValue) {
        self.raw.error(error)
    }
}

#[async_trait(? Send)]
pub trait UnderlyingSource {
    type Item: AsRef<JsValue>;

    async fn start(&mut self, controller: &ReadableStreamDefaultController<Self::Item>) -> Result<(), JsValue> {
        let _ = controller;
        Ok(())
    }

    async fn pull(&mut self, controller: &ReadableStreamDefaultController<Self::Item>) -> Result<(), JsValue> {
        let _ = controller;
        Ok(())
    }

    async fn cancel(&mut self, reason: &JsValue) -> Result<(), JsValue> {
        let _ = reason;
        Ok(())
    }
}

struct JsUnderlyingSource<T: AsRef<JsValue> + 'static> {
    raw: sys::UnderlyingSource,
    start_closure: Closure<dyn FnMut(sys::ReadableStreamDefaultController) -> Promise>,
    pull_closure: Closure<dyn FnMut(sys::ReadableStreamDefaultController) -> Promise>,
    cancel_closure: Closure<dyn FnMut(JsValue) -> Promise>,
    _item_type: PhantomData<T>,
}

impl<T: AsRef<JsValue> + 'static> JsUnderlyingSource<T> {
    pub fn new(source: Box<dyn UnderlyingSource<Item=T> + 'static>) -> JsUnderlyingSource<T> {
        let source = Rc::new(RefCell::new(source));

        let start_closure = {
            let source = source.clone();
            Closure::wrap(Box::new(move |controller: sys::ReadableStreamDefaultController| {
                let source = source.clone();
                future_to_promise(async move {
                    // This mutable borrow can never panic, since the ReadableStream always
                    // queues each operation on the underlying source.
                    let mut source = source.borrow_mut();
                    source.start(&ReadableStreamDefaultController::from_raw(controller)).await?;
                    Ok(JsValue::undefined())
                })
            }) as Box<dyn FnMut(sys::ReadableStreamDefaultController) -> Promise>)
        };
        let pull_closure = {
            let source = source.clone();
            Closure::wrap(Box::new(move |controller: sys::ReadableStreamDefaultController| {
                let source = source.clone();
                future_to_promise(async move {
                    let mut source = source.borrow_mut();
                    source.pull(&ReadableStreamDefaultController::from_raw(controller)).await?;
                    Ok(JsValue::undefined())
                })
            }) as Box<dyn FnMut(sys::ReadableStreamDefaultController) -> Promise>)
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

        let raw = sys::UnderlyingSource::from(JsValue::from(Object::new()));
        raw.set_start(&start_closure);
        raw.set_pull(&pull_closure);
        raw.set_cancel(&cancel_closure);

        JsUnderlyingSource {
            raw,
            start_closure,
            pull_closure,
            cancel_closure,
            _item_type: PhantomData as PhantomData<T>,
        }
    }

    #[inline]
    pub fn as_raw(&self) -> &sys::UnderlyingSource {
        &self.raw
    }

    pub fn forget(self) -> sys::UnderlyingSource {
        self.start_closure.forget();
        self.pull_closure.forget();
        self.cancel_closure.forget();
        self.raw
    }
}

pub struct ReadableStreamDefaultReader<'stream, T: From<JsValue> + AsRef<JsValue> + 'static = JsValue> {
    raw: Option<sys::ReadableStreamDefaultReader>,
    _stream: PhantomData<&'stream mut ReadableStream<T>>,
}

impl<'stream, T: From<JsValue> + AsRef<JsValue> + 'static> ReadableStreamDefaultReader<'stream, T> {
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

    pub async fn read(&mut self) -> Result<Option<T>, JsValue> {
        let js_value = JsFuture::from(self.as_raw().read()).await?;
        let result = sys::ReadableStreamReadResult::from(js_value);
        if result.is_done() {
            Ok(None)
        } else {
            Ok(Some(T::from(result.value())))
        }
    }

    pub fn release_lock(&mut self) -> Result<(), JsValue> {
        if let Some(raw) = self.raw.as_ref() {
            raw.release_lock()?;
            self.raw.take();
        }
        Ok(())
    }

    pub fn into_stream(self) -> IntoStream<'stream, T> {
        IntoStream::new(self)
    }
}

impl<T: From<JsValue> + AsRef<JsValue> + 'static> Drop for ReadableStreamDefaultReader<'_, T> {
    fn drop(&mut self) {
        // TODO Error handling?
        self.release_lock().unwrap();
    }
}

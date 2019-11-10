use std::fmt::{Error, Formatter};

use futures::future::{abortable, Aborted, join};
use futures::stream::StreamExt;
use pin_utils::pin_mut;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use async_trait::async_trait;
use wasm_streams::readable::*;

struct NoopSource;

struct HelloWorldSource;

#[derive(Debug, Eq, PartialEq)]
struct JsStringWrapper {
    value: JsValue
}

impl<'a> From<&'a str> for JsStringWrapper {
    fn from(s: &'a str) -> JsStringWrapper {
        JsStringWrapper { value: JsValue::from(s) }
    }
}

impl From<JsValue> for JsStringWrapper {
    fn from(value: JsValue) -> Self {
        JsStringWrapper { value }
    }
}

impl AsRef<JsValue> for JsStringWrapper {
    fn as_ref(&self) -> &JsValue {
        &self.value
    }
}

#[async_trait(? Send)]
impl UnderlyingSource for NoopSource {
    type Item = JsValue;
}

#[async_trait(? Send)]
impl UnderlyingSource for HelloWorldSource {
    type Item = JsStringWrapper;

    async fn start(&mut self, controller: &ReadableStreamDefaultController<JsStringWrapper>) -> Result<(), JsValue> {
        controller.enqueue(&JsStringWrapper::from("Hello"));
        controller.enqueue(&JsStringWrapper::from("world!"));
        controller.close();
        Ok(())
    }
}

#[wasm_bindgen_test]
async fn test_readable_stream_new() {
    let mut readable = ReadableStream::new(Box::new(HelloWorldSource));
    assert!(!readable.is_locked());

    let mut reader = readable.get_reader().unwrap();
    assert_eq!(reader.read().await.unwrap(), Some(JsStringWrapper::from("Hello")));
    assert_eq!(reader.read().await.unwrap(), Some(JsStringWrapper::from("world!")));
    assert_eq!(reader.read().await.unwrap(), None);
    reader.closed().await.unwrap();
}

#[wasm_bindgen_test]
async fn test_readable_stream_into_stream() {
    let mut readable = ReadableStream::new(Box::new(HelloWorldSource));
    assert!(!readable.is_locked());

    let reader = readable.get_reader().unwrap();
    let stream = reader.into_stream();
    pin_mut!(stream);

    assert_eq!(stream.next().await, Some(Ok(JsStringWrapper::from("Hello"))));
    assert_eq!(stream.next().await, Some(Ok(JsStringWrapper::from("world!"))));
    assert_eq!(stream.next().await, None);
}

#[wasm_bindgen_test]
async fn test_readable_stream_multiple_release_lock() {
    let mut readable = ReadableStream::new(Box::new(NoopSource));

    let mut reader = readable.get_reader().unwrap();
    reader.release_lock().unwrap();
    reader.release_lock().unwrap();
    reader.release_lock().unwrap();
}

#[wasm_bindgen_test]
async fn test_readable_stream_abort_read() {
    let mut readable = ReadableStream::new(Box::new(NoopSource));

    let mut reader = readable.get_reader().unwrap();

    // Start reading, but abort the future immediately
    // Use `join` to poll the future at least once
    let (fut, handle) = abortable(reader.read());
    let (result, _) = join(fut, async {
        handle.abort();
    }).await;
    assert_eq!(result, Err(Aborted));

    // Must cancel any pending reads before releasing the reader's lock
    reader.cancel().await.unwrap();
}
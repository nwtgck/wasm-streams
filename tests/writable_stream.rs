extern crate wasm_bindgen_test;

use futures::sink::SinkExt;
use js_sys::JsString;
use pin_utils::pin_mut;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use wasm_streams::writable::*;

#[wasm_bindgen(module = "/tests/writable_stream.js")]
extern "C" {
    fn new_noop_writable_stream() -> sys::WritableStream;
    fn new_logging_writable_stream() -> sys::WritableStream;
}

#[wasm_bindgen_test]
async fn test_writable_stream_new() {
    let mut writable: WritableStream<JsValue> = From::from(new_noop_writable_stream());
    assert!(!writable.is_locked());

    let mut writer = writable.get_writer().unwrap();
    assert_eq!(writer.write(JsValue::from("Hello")).await.unwrap(), ());
    assert_eq!(writer.write(JsValue::from("world!")).await.unwrap(), ());
    assert_eq!(writer.close().await.unwrap(), ());
    writer.closed().await.unwrap();
}

#[wasm_bindgen_test]
async fn test_writable_stream_into_sink() {
    let mut writable: WritableStream<JsString> = From::from(new_logging_writable_stream());
    assert!(!writable.is_locked());

    let writer = writable.get_writer().unwrap();
    let sink = writer.into_sink();
    pin_mut!(sink);
    assert_eq!(sink.send(JsString::from("Hello")).await, Ok(()));
    assert_eq!(sink.send(JsString::from("world!")).await, Ok(()));
    assert_eq!(sink.close().await, Ok(()));
}

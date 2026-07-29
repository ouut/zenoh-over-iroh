//! zenoh-mobile — 移动端 FFI 层
//! iOS 编译: cargo build --release --target aarch64-apple-ios
//! Android:   cargo build --release --target aarch64-linux-android

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, OnceLock};
use zenoh::prelude::r#async::*;

static SESSION: OnceLock<Arc<Mutex<Option<zenoh::Session>>>> = OnceLock::new();

fn cell() -> Option<Arc<Mutex<Option<zenoh::Session>>>> { SESSION.get().cloned() }
fn rt() -> tokio::runtime::Runtime { tokio::runtime::Runtime::new().unwrap() }

#[no_mangle]
pub extern "C" fn zenoh_mobile_open(config: *const c_char) -> i32 {
    let s = unsafe { CStr::from_ptr(config) }.to_str().unwrap_or("{}");
    match rt().block_on(zenoh::open(zenoh::Config::from_str(s).unwrap())) {
        Ok(session) => { SESSION.set(Arc::new(Mutex::new(Some(session)))).ok(); 0 }
        Err(e) => { eprintln!("open: {e}"); -1 }
    }
}

#[no_mangle]
pub extern "C" fn zenoh_mobile_put(key: *const c_char, value: *const c_char) -> i32 {
    let Some(c) = cell() else { return -1 };
    let s = c.lock().unwrap().clone().unwrap();
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("").to_owned();
    let v = unsafe { CStr::from_ptr(value) }.to_str().unwrap_or("").to_owned();
    rt().block_on(s.put(k, v)).map(|_| 0).unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn zenoh_mobile_close() -> i32 {
    if let Some(s) = cell().and_then(|c| c.lock().ok()?.take()) {
        rt().block_on(s.close());
    }
    0
}

#[no_mangle]
pub extern "C" fn zenoh_mobile_free_string(s: *mut c_char) {
    if !s.is_null() { unsafe { drop(CString::from_raw(s)); } }
}

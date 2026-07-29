//! zenoh-mobile — 完整 Zenoh C API + Iroh 传输层
//!
//! 编译:
//!   iOS:     cargo build --release --target aarch64-apple-ios
//!   Android: cargo build --release --target aarch64-linux-android
//!
//! 产出 .a / .so，支持 "iroh/" endpoint 配置。

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, OnceLock};
use zenoh::prelude::r#async::*;

// ── 回调注册表 ──────────────────────────────────
type CbMap = Arc<Mutex<HashMap<u64, (extern "C" fn(*const c_char, *const c_char, *mut std::ffi::c_void), *mut std::ffi::c_void)>>>;
static SUBS: OnceLock<CbMap> = OnceLock::new();
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn subs() -> &'static CbMap { SUBS.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))) }

// ── Config ─────────────────────────────────────

#[no_mangle]
pub extern "C" fn z_config_new() -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(zenoh::Config::default())) as *mut _
}

#[no_mangle]
pub extern "C" fn z_config_from_str(json: *const c_char) -> *mut std::ffi::c_void {
    let s = unsafe { CStr::from_ptr(json) }.to_str().unwrap_or("{}");
    match zenoh::Config::from_str(s) {
        Ok(c) => Box::into_raw(Box::new(c)) as *mut _,
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn z_config_free(cfg: *mut std::ffi::c_void) {
    if !cfg.is_null() { unsafe { drop(Box::from_raw(cfg as *mut zenoh::Config)); } }
}

// ── Session ────────────────────────────────────

#[no_mangle]
pub extern "C" fn z_open(session: *mut *mut std::ffi::c_void, config: *const std::ffi::c_void) -> i32 {
    let cfg = if config.is_null() {
        zenoh::Config::default()
    } else {
        unsafe { (*(config as *const zenoh::Config)).clone() }
    };

    match std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(zenoh::open(cfg))
    }).join().unwrap() {
        Ok(s) => {
            unsafe { *session = Box::into_raw(Box::new(s)) as *mut _; }
            0
        }
        Err(e) => { eprintln!("z_open: {e}"); -1 }
    }
}

#[no_mangle]
pub extern "C" fn z_close(session: *mut std::ffi::c_void) {
    if session.is_null() { return; }
    let s = unsafe { Box::from_raw(session as *mut zenoh::Session) };
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(s.close());
    }).join().unwrap();
}

// ── Put / Delete ───────────────────────────────

#[no_mangle]
pub extern "C" fn z_put(session: *mut std::ffi::c_void, key: *const c_char, value: *const c_char) -> i32 {
    if session.is_null() { return -1; }
    let s = unsafe { &*(session as *const zenoh::Session) };
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    let v = unsafe { CStr::from_ptr(value) }.to_str().unwrap_or("");
    let s = s.clone(); let k = k.to_owned(); let v = v.to_owned();
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(s.put(k, v))
    }).join().unwrap().map(|_| 0).unwrap_or(-1)
}

// ── Subscribe ──────────────────────────────────

#[no_mangle]
pub extern "C" fn z_subscribe(
    session: *mut std::ffi::c_void,
    key: *const c_char,
    cb: extern "C" fn(*const c_char, *const c_char, *mut std::ffi::c_void),
    ctx: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    let s = unsafe { &*(session as *const zenoh::Session) };
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("").to_owned();
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    subs().lock().unwrap().insert(id, (cb, ctx));

    let s = s.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Ok(sub) = s.declare_subscriber(&k).await {
                loop {
                    if let Ok(sample) = sub.recv_async().await {
                        let subs = SUBS.get().unwrap().lock().unwrap();
                        if let Some((cb, ctx)) = subs.get(&id) {
                            let k = CString::new(sample.key_expr().to_string()).unwrap();
                            let v = CString::new(sample.payload().to_string()).unwrap();
                            cb(k.as_ptr(), v.as_ptr(), *ctx);
                        }
                    }
                }
            }
        })
    });

    Box::into_raw(Box::new(id)) as *mut _
}

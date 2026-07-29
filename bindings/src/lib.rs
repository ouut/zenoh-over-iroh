use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, OnceLock, Mutex};
use zenoh::Session;


struct State {
    rt: tokio::runtime::Runtime,
    session: Option<Session>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State {
        rt: tokio::runtime::Runtime::new().unwrap(),
        session: None,
    }))
}

#[no_mangle]
pub extern "C" fn z_open(config_str: *const c_char) -> i32 {
    let json = unsafe { CStr::from_ptr(config_str) }.to_str().unwrap_or("{}");
    let cfg: zenoh::Config = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    let mut s = state().lock().unwrap();
    let f = async { zenoh::open(cfg).await };
    match s.rt.block_on(f) {
        Ok(session) => { s.session = Some(session); 0 }
        Err(_) => -1
    }
}

#[no_mangle]
pub extern "C" fn z_put(key: *const c_char, value: *const c_char) -> i32 {
    let mut s = state().lock().unwrap();
    let Some(ref session) = s.session else { return -1 };
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    let v = unsafe { CStr::from_ptr(value) }.to_str().unwrap_or("").to_string();
    let f = async { session.put(k, v).await };
    s.rt.block_on(f).map(|_| 0).unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn z_close() -> i32 {
    let mut s = state().lock().unwrap();
    if let Some(session) = s.session.take() {
        s.rt.block_on(async { session.close().await });
    }
    0
}

#[no_mangle]
pub extern "C" fn z_zid() -> *mut c_char {
    let s = state().lock().unwrap();
    s.session.as_ref().map(|s| {
        CString::new(s.zid().to_string()).unwrap().into_raw()
    }).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn z_free_string(ptr: *mut c_char) {
    if !ptr.is_null() { unsafe { drop(CString::from_raw(ptr)); } }
}

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Mutex, OnceLock};

struct State {
    rt: tokio::runtime::Runtime,
    session: Option<zenoh::Session>,
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
    let cfg: zenoh::Config = serde_json::from_str(json).unwrap_or_default();
    let mut s = state().lock().unwrap();
    s.session = match s.rt.block_on(async { zenoh::open(cfg).await }) {
        Ok(session) => Some(session),
        Err(e) => { eprintln!("open: {e}"); return -1; }
    };
    0
}

#[no_mangle]
pub extern "C" fn z_put(key: *const c_char, value: *const c_char) -> i32 {
    let mut s = state().lock().unwrap();
    let session = match &s.session { Some(x) => x.clone(), None => return -1 };
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("").to_owned();
    let v = unsafe { CStr::from_ptr(value) }.to_str().unwrap_or("").to_owned();
    s.rt.block_on(async { session.put(k, v.as_str()).await }).map(|_| 0).unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn z_get(key: *const c_char, cb: extern "C" fn(*const c_char, *const c_char)) -> i32 {
    let mut s = state().lock().unwrap();
    let session = match &s.session { Some(x) => x.clone(), None => return -1 };
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("").to_owned();

    s.rt.block_on(async {
        if let Ok(mut replies) = session.get(k).await {
            loop {
                match replies.recv_async().await {
                    Ok(reply) => {
                        if let Ok(sample) = reply.result() {
                            let k = CString::new(sample.key_expr().to_string()).unwrap_or_default();
                            let v = String::from_utf8_lossy(&sample.payload().to_bytes()).to_string();
                            let v = CString::new(v).unwrap_or_default();
                            cb(k.as_ptr(), v.as_ptr());
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
    0
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

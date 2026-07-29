use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// A wrapper around `*mut c_void` that is `Send` and `Sync`.
///
/// Safety: The raw pointer is stored in a global static Mutex and is never
/// moved across threads — it is only looked up by id in the background task
/// and used to call the callback. The caller must ensure the pointed-to data
/// outlives the subscription.
#[repr(transparent)]
#[derive(Copy, Clone)]
struct CtxPtr(*mut c_void);
unsafe impl Send for CtxPtr {}
unsafe impl Sync for CtxPtr {}

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

/// Zenoh callback type matching zenoh_mobile.h:
///     void (*)(const char* key, const char* value, void* ctx)
type ZenohCallback = unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void);

// ── Subscriber tracking ──────────────────────────────────────────────
// Stores (callback, ctx, cancellation_sender) keyed by subscriber id.
// The ctx raw pointer is never moved into an async task — only the u64 id
// crosses the Send boundary, and the background task looks up the pointer
// from this static map at callback time.

static NEXT_SUB_ID: AtomicU64 = AtomicU64::new(1);
static SUBS: OnceLock<Mutex<HashMap<u64, (ZenohCallback, CtxPtr, tokio::sync::watch::Sender<bool>)>>> =
    OnceLock::new();
fn subs() -> &'static Mutex<HashMap<u64, (ZenohCallback, CtxPtr, tokio::sync::watch::Sender<bool>)>> {
    SUBS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── Publisher tracking ──────────────────────────────────────────────
// Stores the key expression string for each declared publisher handle.

static NEXT_PUB_ID: AtomicU64 = AtomicU64::new(1);
static PUBS: OnceLock<Mutex<HashMap<u64, String>>> = OnceLock::new();
fn pubs() -> &'static Mutex<HashMap<u64, String>> {
    PUBS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[no_mangle]
pub extern "C" fn z_open(config_str: *const c_char) -> i32 {
    let json = unsafe { CStr::from_ptr(config_str) }.to_str().unwrap_or("{}");
    let cfg: zenoh::Config = serde_json::from_str(json).unwrap_or_default();
    let mut s = state().lock().unwrap();
    s.session = match s.rt.block_on(async { zenoh::open(cfg).await }) {
        Ok(session) => Some(session),
        Err(e) => {
            eprintln!("open: {e}");
            return -1;
        }
    };
    0
}

#[no_mangle]
pub extern "C" fn z_put(key: *const c_char, value: *const c_char) -> i32 {
    let mut s = state().lock().unwrap();
    let session = match &s.session {
        Some(x) => x.clone(),
        None => return -1,
    };
    let k = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("")
        .to_owned();
    let v = unsafe { CStr::from_ptr(value) }
        .to_str()
        .unwrap_or("")
        .to_owned();
    s.rt
        .block_on(async { session.put(k, v.as_str()).await })
        .map(|_| 0)
        .unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn z_get(
    key: *const c_char,
    cb: ZenohCallback,
    ctx: *mut c_void,
) -> i32 {
    let mut s = state().lock().unwrap();
    let session = match &s.session {
        Some(x) => x.clone(),
        None => return -1,
    };
    let k = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("")
        .to_owned();

    s.rt.block_on(async {
        if let Ok(mut replies) = session.get(k).await {
            loop {
                match replies.recv_async().await {
                    Ok(reply) => {
                        if let Ok(sample) = reply.result() {
                            let k = CString::new(sample.key_expr().to_string())
                                .unwrap_or_default();
                            let v = String::from_utf8_lossy(&sample.payload().to_bytes())
                                .to_string();
                            let v = CString::new(v).unwrap_or_default();
                            unsafe { cb(k.as_ptr(), v.as_ptr(), ctx) };
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
pub extern "C" fn z_subscribe(
    key: *const c_char,
    cb: ZenohCallback,
    ctx: *mut c_void,
) -> u64 {
    let id = NEXT_SUB_ID.fetch_add(1, Ordering::Relaxed);

    // Create cancellation channel — initial value false means "keep running"
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    subs().lock().unwrap().insert(id, (cb, CtxPtr(ctx), cancel_tx));

    let s = state().lock().unwrap();
    let session = match &s.session {
        Some(x) => x.clone(),
        None => {
            subs().lock().unwrap().remove(&id);
            return 0;
        }
    };
    let k = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("")
        .to_owned();

    let handle = s.rt.handle().clone();
    handle.spawn(async move {
        let subscriber = match session.declare_subscriber(k).await {
            Ok(sub) => sub,
            Err(e) => {
                eprintln!("subscribe: {e}");
                subs().lock().unwrap().remove(&id);
                return;
            }
        };

        let mut cancel_rx = cancel_rx;
        loop {
            tokio::select! {
                _ = cancel_rx.changed() => break,
                result = subscriber.recv_async() => {
                    match result {
                        Ok(sample) => {
                            let k = CString::new(sample.key_expr().to_string())
                                .unwrap_or_default();
                            let v = String::from_utf8_lossy(&sample.payload().to_bytes())
                                .to_string();
                            let v = CString::new(v).unwrap_or_default();
                            // Look up callback/ctx from the static map.
                            // Only the u64 id crosses the Send boundary — the raw
                            // *mut c_void ctx is retrieved here in the spawned task.
                            let entry = {
                                let guard = subs().lock().unwrap();
                                guard.get(&id).map(|&(cb, ctx, _)| (cb, ctx.0))
                            };
                            if let Some((cb, ctx)) = entry {
                                unsafe { cb(k.as_ptr(), v.as_ptr(), ctx) };
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
        subs().lock().unwrap().remove(&id);
    });

    id
}

#[no_mangle]
pub extern "C" fn z_unsubscribe(handle: u64) -> i32 {
    if let Some((_, _, cancel_tx)) = subs().lock().unwrap().remove(&handle) {
        let _ = cancel_tx.send(true);
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn z_declare_publisher(key: *const c_char) -> u64 {
    let id = NEXT_PUB_ID.fetch_add(1, Ordering::Relaxed);
    let k = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("")
        .to_owned();
    pubs().lock().unwrap().insert(id, k);
    id
}

#[no_mangle]
pub extern "C" fn z_publisher_put(pub_id: u64, value: *const c_char) -> i32 {
    let v = unsafe { CStr::from_ptr(value) }
        .to_str()
        .unwrap_or("")
        .to_owned();
    let key = {        let guard = pubs().lock().unwrap();
        guard.get(&pub_id).cloned()
    };
    let key = match key {
        Some(k) => k,
        None => return -1,
    };
    let mut s = state().lock().unwrap();
    let session = match &s.session {
        Some(x) => x.clone(),
        None => return -1,
    };
    s.rt
        .block_on(async { session.put(key, v.as_str()).await })
        .map(|_| 0)
        .unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn z_undeclare_publisher(pub_id: u64) -> i32 {
    pubs().lock().unwrap().remove(&pub_id);
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
    s.session
        .as_ref()
        .map(|s| CString::new(s.zid().to_string()).unwrap().into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn z_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

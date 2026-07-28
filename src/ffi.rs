//! # FFI 层 — 面向移动端的 C API
//!
//! `LinkStateMachine` 可被任何支持 FFI 的语言调用。
//! 本模块提供 `extern "C"` 函数供 Swift (iOS) 和 Kotlin/Java (Android) 使用。
//!
//! ## 对象模型
//!
//! 所有函数通过 opaque pointer 操作:
//!   - `zenoh_lsm_t` — LinkStateMachine handle
//!
//! ## 生命周期
//!
//! ```c
//! zenoh_lsm_t* lsm = zenoh_lsm_new();
//! // ... 使用 ...
//! zenoh_lsm_free(lsm);
//! ```

use std::ffi::{c_char, c_void, CStr, CString};
use std::os::raw::c_int;
use std::ptr;

use crate::link_state::{LinkEvent, LinkError, LinkStateMachine, WriteStatus};

/// Opaque handle to LinkStateMachine.
pub struct ZenohLsm(LinkStateMachine);

/// 创建新的状态机 (初始 Connected)。
#[no_mangle]
pub extern "C" fn zenoh_lsm_new() -> *mut c_void {
    let lsm = Box::new(ZenohLsm(LinkStateMachine::new()));
    Box::into_raw(lsm) as *mut c_void
}

/// 创建带背压的状态机。
#[no_mangle]
pub extern "C" fn zenoh_lsm_new_with_backpressure(max_queue: u32) -> *mut c_void {
    let lsm = Box::new(ZenohLsm(LinkStateMachine::with_backpressure(
        max_queue as usize,
    )));
    Box::into_raw(lsm) as *mut c_void
}

/// 释放状态机。
#[no_mangle]
pub extern "C" fn zenoh_lsm_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(ptr as *mut ZenohLsm);
    }
}

/// 路径变化通知。
///
/// connected=1 表示路径恢复，connected=0 表示路径失联。
/// 返回事件代码: 0=None, 1=PathMigrated, 2=PathRestored, 3=MigrationTimeout
#[no_mangle]
pub extern "C" fn zenoh_lsm_on_path_change(ptr: *mut c_void, connected: c_int) -> c_int {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    match lsm.0.on_path_change(connected != 0) {
        Some(LinkEvent::PathMigrated) => 1,
        Some(LinkEvent::PathRestored) => 2,
        Some(LinkEvent::MigrationTimeout) => 3,
        None => 0,
    }
}

/// 写入数据。
///
/// 返回状态: 0=Sent, 1=Queued, 2=Backpressure, -1=Disconnected
#[no_mangle]
pub extern "C" fn zenoh_lsm_write(
    ptr: *mut c_void,
    data: *const u8,
    len: u32,
) -> c_int {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    match lsm.0.write(bytes.to_vec()) {
        Ok(WriteStatus::Sent) => 0,
        Ok(WriteStatus::Queued) => 1,
        Ok(WriteStatus::Backpressure) => 2,
        Err(_) => -1,
    }
}

/// 检查读取能力。返回 0=OK, -1=Disconnected
#[no_mangle]
pub extern "C" fn zenoh_lsm_can_read(ptr: *mut c_void) -> c_int {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    match lsm.0.read() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 定时轮询。
/// 返回事件代码: 0=None, 3=MigrationTimeout
#[no_mangle]
pub extern "C" fn zenoh_lsm_tick(ptr: *mut c_void) -> c_int {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    match lsm.0.tick() {
        Some(LinkEvent::MigrationTimeout) => 3,
        _ => 0,
    }
}

/// 排出排队数据。
///
/// 将排队数据写入 buf，返回实际写入的字节数。
/// 若 buf 不够大，数据会被截断。
/// 返回 -1 表示无排队数据。
#[no_mangle]
pub extern "C" fn zenoh_lsm_drain(
    ptr: *mut c_void,
    buf: *mut u8,
    buf_len: u32,
) -> i32 {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    let drained = lsm.0.drain_queue();
    if drained.is_empty() {
        return -1;
    }

    let mut written: u32 = 0;
    let buf = unsafe { std::slice::from_raw_parts_mut(buf, buf_len as usize) };
    for item in drained.iter() {
        let remaining = buf_len.saturating_sub(written) as usize;
        if remaining == 0 {
            break;
        }
        let copy_len = item.len().min(remaining);
        buf[written as usize..written as usize + copy_len].copy_from_slice(&item[..copy_len]);
        written += copy_len as u32;
    }
    written as i32
}

/// 获取排队队列长度。
#[no_mangle]
pub extern "C" fn zenoh_lsm_queue_len(ptr: *mut c_void) -> u32 {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    lsm.0.queue_len() as u32
}

/// 是否处于 Connected 状态。
#[no_mangle]
pub extern "C" fn zenoh_lsm_is_connected(ptr: *mut c_void) -> c_int {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    lsm.0.is_connected() as c_int
}

/// 是否处于 Migrating 状态。
#[no_mangle]
pub extern "C" fn zenoh_lsm_is_migrating(ptr: *mut c_void) -> c_int {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    lsm.0.is_migrating() as c_int
}

/// 显式断开连接。
#[no_mangle]
pub extern "C" fn zenoh_lsm_disconnect(ptr: *mut c_void) {
    let lsm = unsafe { &mut *(ptr as *mut ZenohLsm) };
    lsm.0.disconnect();
}

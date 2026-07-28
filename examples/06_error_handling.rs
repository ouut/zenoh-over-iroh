//! 示例: 错误处理模式
//!
//! 演示 LinkStateMachine 在各种异常路径下的行为:
//!   - Connected 态重入路径事件 (noop)
//!   - Migrating 态重入路径失联 (noop)
//!   - Disconnected 态忽略路径事件
//!   - Disconnected 态 write/read/tick 全部拒绝
//!   - 超时快速触发 (tick 立即生效)
//!
//! 运行: cargo run --example 06_error_handling

use zenoh_link_state::link_state::{LinkError, LinkStateMachine, LinkEvent, WriteStatus};

fn main() {
    let mut sm = LinkStateMachine::new();

    // ── 1. Connected 态重复恢复事件 (noop) ──
    let event = sm.on_path_change(true);
    assert!(event.is_none());
    println!("[1] Connected + path restore → noop ✓");

    // ── 2. 进入 Migrating ────────────────────
    let event = sm.on_path_change(false);
    assert!(matches!(event, Some(LinkEvent::PathMigrated)));
    println!("[2] Connected → Migrating ✓");

    // ── 3. Migrating 重复失联 (noop) ─────────
    let event = sm.on_path_change(false);
    assert!(event.is_none());
    println!("[3] Migrating + path loss → noop ✓");

    // ── 4. 恢复 → Connected ──────────────────
    sm.on_path_change(true);
    assert!(sm.is_connected());
    println!("[4] Migrating → Connected (recovery) ✓");

    // ── 5. 显式断连 ──────────────────────────
    sm.disconnect();
    assert!(sm.is_disconnected());
    println!("[5] Conn → Disconnected (explicit) ✓");

    // ── 6. Disconnected 态 write 报错 ────────
    assert_eq!(sm.write(b"data".to_vec()), Err(LinkError::Disconnected));
    println!("[6] Disconnected write() → Err ✓");

    // ── 7. Disconnected 态 read 报错 ─────────
    assert_eq!(sm.read(), Err(LinkError::Disconnected));
    println!("[7] Disconnected read() → Err ✓");

    // ── 8. Disconnected 态 tick 无事件 ───────
    assert!(sm.tick().is_none());
    println!("[8] Disconnected tick() → None ✓");

    // ── 9. Disconnected 态 ignore path ───────
    assert!(sm.on_path_change(true).is_none());
    assert!(sm.on_path_change(false).is_none());
    println!("[9] Disconnected path events → ignored ✓");

    // ── 10. drain_queue 在 Disconnected 态返回空 ───
    let drained = sm.drain_queue();
    assert!(drained.is_empty());
    println!("[10] Disconnected drain_queue() → empty ✓");

    // ── 11. 新建连接验证清洁状态 ─────────────
    let mut new_sm = LinkStateMachine::new();
    assert!(new_sm.is_connected());
    assert_eq!(new_sm.queue_len(), 0);
    assert_eq!(new_sm.write(b"clean".to_vec()), Ok(WriteStatus::Sent));
    println!("[11] New LinkStateMachine → clean state ✓");

    // ── 12. Default trait ────────────────────
    let default_sm = LinkStateMachine::default();
    assert!(default_sm.is_connected());
    println!("[12] Default::default() → Connected ✓");

    println!();
    println!("=== 06_error_handling: PASS ===");
}

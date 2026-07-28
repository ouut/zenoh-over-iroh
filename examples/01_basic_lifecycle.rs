//! 示例: 基本状态机生命周期
//!
//! 演示 LinkStateMachine 的完整生命周期:
//!   Connected → Migrating → Connected (恢复)
//!   Connected → Migrating → Disconnected (超时)
//!
//! 运行: cargo run --example 01_basic_lifecycle

use zenoh_link_state::link_state::{LinkStateMachine, WriteStatus};

fn main() {
    let mut sm = LinkStateMachine::new();
    println!("[init] State: Connected ✓");

    // ── 正常写入 ────────────────────────────
    assert_eq!(sm.write(b"hello".to_vec()), Ok(WriteStatus::Sent));
    println!("[write] Data sent immediately ✓");

    // ── 路径失联 → Migrating ───────────────
    sm.on_path_change(false);
    assert!(sm.is_migrating());
    println!("[path]  Entering Migrating state...");

    // ── Migrating 期间写入排队（不报错）─────
    for i in 0..5 {
        let result = sm.write(format!("msg_{}", i).into_bytes());
        assert_eq!(result, Ok(WriteStatus::Queued));
    }
    println!("[write] 5 messages queued during migration ✓");
    assert_eq!(sm.queue_len(), 5);

    // ── 路径恢复 → Connected ────────────────
    sm.on_path_change(true);
    assert!(sm.is_connected());
    println!("[path]  Path restored → Connected");

    // ── 排出排队数据 ────────────────────────
    let drained: Vec<_> = sm.drain_queue().into_iter().collect();
    println!("[drain] {} messages recovered:", drained.len());
    for (i, data) in drained.iter().enumerate() {
        println!("        [{i}] {}", String::from_utf8_lossy(data));
    }
    assert_eq!(drained.len(), 5);
    assert_eq!(sm.queue_len(), 0);

    // ── 模拟超时路径 ────────────────────────
    println!();
    println!("[demo] Now showing the timeout path...");
    sm.on_path_change(false);
    sm.write(b"will_timeout".to_vec()).unwrap();
    println!("[write] Data queued, waiting for timeout...");

    // tick() 立即超时（因为进入 Migrating 的 Instant 在过去，
    // 但这里用 disconnect() 直接模拟超时效果）
    sm.disconnect();
    assert!(sm.is_disconnected());
    assert_eq!(sm.queue_len(), 0);
    println!("[timeout] Entered Disconnected, queue discarded ✓");

    // ── 验证重连后无旧数据 ──────────────────
    let mut new_conn = LinkStateMachine::new();
    assert_eq!(new_conn.queue_len(), 0);
    assert_eq!(
        new_conn.write(b"fresh_start".to_vec()),
        Ok(WriteStatus::Sent)
    );
    println!("[reconnect] New connection: clean slate ✓");

    println!();
    println!("=== 01_basic_lifecycle: PASS ===");
}

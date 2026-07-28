//! 示例: 背压机制 (Backpressure)
//!
//! 演示 LinkStateMachine 的 max_queue_depth 功能:
//!   队列满 → WriteStatus::Backpressure
//!
//! 运行: cargo run --example 02_backpressure

use zenoh_link_state::link_state::{LinkStateMachine, WriteStatus};

fn main() {
    let max_depth = 5;
    let mut sm = LinkStateMachine::with_backpressure(max_depth);
    println!("[init] Backpressure limit: {} messages", max_depth);

    // Connected 态不受背压限制
    for i in 0..3 {
        assert_eq!(sm.write(format!("connected_{}", i).into_bytes()), Ok(WriteStatus::Sent));
    }
    println!("[connected] 3 writes → all Sent ✓");

    // 进入 Migrating
    sm.on_path_change(false);
    assert!(sm.is_migrating());
    println!("[path] Entering Migrating...");

    // 排满到 max_depth=5
    for i in 1..=5 {
        let result = sm.write(format!("queued_{}", i).into_bytes());
        assert_eq!(result, Ok(WriteStatus::Queued));
        println!("[queue]  {}: Queued (depth: {})", i, sm.queue_len());
    }
    assert_eq!(sm.queue_len(), 5);

    // 第 6 条 → 触发背压
    let result = sm.write(b"overflow".to_vec());
    assert_eq!(result, Ok(WriteStatus::Backpressure));
    println!("[queue]  6: Backpressure! (queue full) ✓");
    assert_eq!(sm.queue_len(), 5);

    // 继续尝试都会背压
    for _i in 0..3 {
        assert_eq!(sm.write(b"blocked".to_vec()), Ok(WriteStatus::Backpressure));
    }
    println!("[queue]  3 more attempts → all Backpressure ✓");

    // 恢复后排出
    sm.on_path_change(true);
    assert!(sm.is_connected());
    let drained: Vec<_> = sm.drain_queue().into_iter().collect();
    println!("[drain] Recovered {} messages after recovery", drained.len());
    assert_eq!(drained.len(), 5);

    // 恢复后正常写入
    assert_eq!(sm.write(b"after_recovery".to_vec()), Ok(WriteStatus::Sent));
    println!("[write] Post-recovery write → Sent ✓");

    println!();
    println!("=== 02_backpressure: PASS ===");
}

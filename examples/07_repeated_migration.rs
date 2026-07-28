//! 示例: 反复迁移周期 (Wi-Fi ↔ 蜂窝切换模拟)
//!
//! 模拟移动设备在 Wi-Fi 和蜂窝网络间反复切换 10 次，
//! 每次切换 500ms 失联窗口，验证数据零丢失。
//!
//! 运行: cargo run --example 07_repeated_migration

use std::time::Duration;
use zenoh_link_state::link_state::{LinkStateMachine, WriteStatus};

#[tokio::main]
async fn main() {
    let mut sm = LinkStateMachine::new();
    let total_cycles = 10;
    let msgs_per_cycle = 5;

    println!(
        "[init] {} migration cycles, {} msgs/cycle",
        total_cycles, msgs_per_cycle
    );

    for cycle in 0..total_cycles {
        // 失联
        sm.on_path_change(false);
        assert!(sm.is_migrating());

        // 持续发布（应用层不感知）
        for j in 0..msgs_per_cycle {
            let data = format!("c{:02}_m{:02}", cycle, j).into_bytes();
            assert_eq!(sm.write(data), Ok(WriteStatus::Queued));
        }

        // 模拟 500ms 网络切换窗口
        tokio::time::sleep(Duration::from_millis(500)).await;

        // tick 不应超时 (500ms < 4000ms)
        assert!(sm.tick().is_none());

        // 恢复
        sm.on_path_change(true);
        assert!(sm.is_connected());

        // 验证数据完整
        let drained: Vec<_> = sm.drain_queue().into_iter().collect();
        assert_eq!(drained.len(), msgs_per_cycle);
        for (j, data) in drained.iter().enumerate() {
            let expected = format!("c{:02}_m{:02}", cycle, j).into_bytes();
            assert_eq!(*data, expected);
        }

        println!(
            "[cycle {:02}] {} msgs queued → recovered ✓",
            cycle, msgs_per_cycle
        );
    }

    println!();
    println!(
        "Total: {} cycles × {} msgs = {} total, zero loss ✓",
        total_cycles,
        msgs_per_cycle,
        total_cycles * msgs_per_cycle
    );
    println!("=== 07_repeated_migration: PASS ===");
}

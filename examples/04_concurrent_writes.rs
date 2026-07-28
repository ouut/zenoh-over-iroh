//! 示例: 并发写入安全
//!
//! 演示多个 tokio task 同时写 Migrating 态状态机的并发安全性。
//!
//! 运行: cargo run --example 04_concurrent_writes

use std::sync::Arc;
use tokio::sync::Mutex;
use zenoh_link_state::link_state::{LinkStateMachine, WriteStatus};

#[tokio::main]
async fn main() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    // 进入 Migrating
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(false);
        println!("[init] Entering Migrating, spawning 20 concurrent writers...");
    }

    // 20 个并发 writer
    let mut handles = vec![];
    for i in 0..20 {
        let sm = Arc::clone(&sm);
        handles.push(tokio::spawn(async move {
            let mut guard = sm.lock().await;
            let data = format!("concurrent_{:02}", i).into_bytes();
            let result = guard.write(data);
            (i, result)
        }));
    }

    // 收集结果
    let mut queued = 0;
    for handle in handles {
        let (i, result) = handle.await.unwrap();
        match result {
            Ok(WriteStatus::Queued) => {
                queued += 1;
                println!("[writer {:02}] Queued", i);
            }
            _ => {
                println!("[writer {:02}] {:?}", i, result);
            }
        }
    }

    println!();
    println!("[result] {} / 20 writers queued successfully", queued);

    // 恢复并验证数据完整性
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(true);
        let drained: Vec<_> = guard.drain_queue().into_iter().collect();
        println!("[drain]  Recovered {} messages", drained.len());
        assert_eq!(drained.len(), 20);
    }

    println!("=== 04_concurrent_writes: PASS ===");
}

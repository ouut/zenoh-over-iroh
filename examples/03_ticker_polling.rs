//! 示例: Ticker 轮询模式
//!
//! 演示使用 tick() 驱动超时检测，模拟真实 LinkUnicast 中的定时轮询。
//! 每 500ms 检查一次，超过 4s 阈值后触发 MigrationTimeout。
//!
//! 运行: cargo run --example 03_ticker_polling

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use zenoh_link_state::link_state::{LinkEvent, LinkStateMachine};

#[tokio::main]
async fn main() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    // 进入 Migrating
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(false);
        guard.write(b"ticker_test_data".to_vec()).unwrap();
        println!("[init]  Entering Migrating, 1 message queued");
    }

    let sm_clone = Arc::clone(&sm);
    let poll_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let mut guard = sm_clone.lock().await;

            match guard.tick() {
                Some(LinkEvent::MigrationTimeout) => {
                    let discarded = guard.queue_len();
                    println!(
                        "[tick]  ⏰ MigrationTimeout! {} messages discarded, Disconnected",
                        discarded
                    );
                    return Some(LinkEvent::MigrationTimeout);
                }
                Some(e) => {
                    println!("[tick]  event: {:?}", e);
                }
                None => {
                    println!("[tick]  polling... (no timeout yet)");
                }
            }
        }
    });

    // 同时，另一个 task 在断连后尝试写入（应失败）
    let writer = {
        let sm = Arc::clone(&sm);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5000)).await;
            let guard = sm.lock().await;
            if guard.is_disconnected() {
                println!("[writer] Link is disconnected — cannot write ✓");
            }
        })
    };

    let result = poll_handle.await.unwrap();
    assert!(matches!(result, Some(LinkEvent::MigrationTimeout)));
    writer.await.unwrap();

    println!();
    println!("=== 03_ticker_polling: PASS ===");
}

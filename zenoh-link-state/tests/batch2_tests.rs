//! # 第二批网络模拟测试（用例 5/6/8 场景扩展）
//!
//! 基于 tokio 异步运行时，验证：
//! - 用例 5：多次 IP 变化 / NAT 刷新 + 数据完整性
//! - 用例 6：消息完整性 — 迁移恢复 vs 超时作废
//! - 用例 8：大报文排队行为
//!
//! 注意：涉及实际时间等待（最长 ~10s），建议 `cargo test -- --nocapture`

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use zenoh_link_state::link_state::{LinkStateMachine, WriteStatus};

// ── 用例 5：多次 IP 变化 / NAT 刷新，数据完整性 ──────────────
// 模拟移动设备在 Wi-Fi / 蜂窝间反复切换

#[tokio::test]
async fn test_case5_repeated_nat_refresh_with_data() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    for cycle in 0..5 {
        // IP 变化：进入 Migrating
        {
            let mut guard = sm.lock().await;
            guard.on_path_change(false);
            assert!(guard.is_migrating());

            // 断网期间持续写入（模拟应用层不感知）
            for i in 0..20 {
                let data = format!("c{}_m{}", cycle, i).into_bytes();
                assert_eq!(guard.write(data).unwrap(), WriteStatus::Queued);
            }
        }

        // 短暂失联（2s，远短于超时 4s）
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // tick 不应超时
        {
            let mut guard = sm.lock().await;
            assert!(
                guard.tick().is_none(),
                "cycle {}: should not timeout",
                cycle
            );
        }

        // IP 恢复
        {
            let mut guard = sm.lock().await;
            guard.on_path_change(true);
            assert!(guard.is_connected());

            // 验证排队数据完整且有序
            let drained: Vec<_> = guard.drain_queue().into_iter().collect();
            assert_eq!(drained.len(), 20, "cycle {}: expected 20 messages", cycle);
            for (i, data) in drained.iter().enumerate() {
                assert_eq!(
                    *data,
                    format!("c{}_m{}", cycle, i).into_bytes(),
                    "cycle {}: message {} mismatch",
                    cycle,
                    i
                );
            }
            assert_eq!(guard.queue_len(), 0);
        }
    }
}

// ── 用例 6：消息完整性 — 迁移恢复 vs 超时作废 ────────────
// 核心验证：
//   - 迁移未超时 → 排队数据完整排出（丢失率=0）
//   - 迁移超时 → 排队数据作废，重连后不误发（重复率=0）

#[tokio::test]
async fn test_case6_message_integrity_recovery_vs_timeout() {
    // Part A: 快速恢复 — 完整性保持
    {
        let mut sm = LinkStateMachine::new();

        sm.on_path_change(false);
        for i in 0..100 {
            sm.write(format!("msg_{:04}", i).into_bytes()).unwrap();
        }

        // 短等待后恢复
        tokio::time::sleep(Duration::from_millis(500)).await;
        sm.on_path_change(true);

        let drained: Vec<_> = sm.drain_queue().into_iter().collect();
        assert_eq!(drained.len(), 100, "all 100 messages must be preserved");
        for (i, data) in drained.iter().enumerate() {
            assert_eq!(*data, format!("msg_{:04}", i).into_bytes());
        }
    }

    // Part B: 超时作废 — 防止重复
    {
        let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

        {
            let mut guard = sm.lock().await;
            guard.on_path_change(false);
            for i in 0..50 {
                guard
                    .write(format!("stale_{}", i).into_bytes())
                    .unwrap();
            }
            assert_eq!(guard.queue_len(), 50);
        }

        // 等待超时
        tokio::time::sleep(Duration::from_millis(4500)).await;

        {
            let mut guard = sm.lock().await;
            guard.tick(); // 触发超时 → Disconnected
            assert!(guard.is_disconnected());
            assert_eq!(guard.queue_len(), 0, "stale data must be discarded");
        }

        // 模拟重连后新连接 — 不应有旧数据残留
        let mut new_sm = LinkStateMachine::new();
        assert_eq!(new_sm.queue_len(), 0);
        assert_eq!(
            new_sm.write(b"fresh_data".to_vec()).unwrap(),
            WriteStatus::Sent
        );
    }
}

// ── 用例 8：大报文排队行为 ──────────────────────────────────
// 验证大 payload（模拟 1MB）在 Migrating 期间排队不阻塞

#[tokio::test]
async fn test_case8_large_payload_queueing() {
    let mut sm = LinkStateMachine::new();

    // 进入 Migrating
    sm.on_path_change(false);

    // 模拟 1MB 大报文排队（10 条 = 10MB 总排队量）
    let large_payload = vec![0xABu8; 1024 * 1024]; // 1MB

    let start = std::time::Instant::now();
    for i in 0..10 {
        let mut data = large_payload.clone();
        data[0] = i as u8; // 标记序号
        assert_eq!(sm.write(data).unwrap(), WriteStatus::Queued);
    }
    let write_elapsed = start.elapsed();
    assert_eq!(sm.queue_len(), 10);

    println!("10 × 1MB writes queued in {:?}", write_elapsed);

    // 短等待后恢复
    tokio::time::sleep(Duration::from_millis(200)).await;
    sm.on_path_change(true);
    assert!(sm.is_connected());

    // 验证大报文完整排出
    let drained: Vec<_> = sm.drain_queue().into_iter().collect();
    assert_eq!(drained.len(), 10);
    for (i, data) in drained.iter().enumerate() {
        assert_eq!(data.len(), 1024 * 1024, "payload {} size mismatch", i);
        assert_eq!(data[0], i as u8, "payload {} marker mismatch", i);
        assert_eq!(data[1], 0xAB, "payload {} content corruption", i);
    }
}

// ── 用例 5/6 综合：长时间迁移中的持续写入 ────────────────────
// 模拟 10s 的迁移期，每秒写入一批数据，验证全部经过状态机

#[tokio::test]
async fn test_sustained_write_during_long_migration() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    // 进入 Migrating
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(false);
    }

    let total_batches = 10usize;
    let msgs_per_batch = 5usize;

    // 后台持续写入（10 批，每批 5 条，每秒一批）
    // 注意：tick 在 ~4s 后超时 → 后续写入应返回 Disconnected 错误
    let writer = {
        let sm = Arc::clone(&sm);
        tokio::spawn(async move {
            let mut total_written = 0usize;
            let mut total_rejected = 0usize;
            for batch in 0..total_batches {
                tokio::time::sleep(Duration::from_millis(1000)).await;
                let mut guard = sm.lock().await;
                for j in 0..msgs_per_batch {
                    match guard
                        .write(format!("b{}_m{}", batch, j).into_bytes())
                    {
                        Ok(_) => total_written += 1,
                        Err(_) => total_rejected += 1, // 超时后正常拒绝
                    }
                }
            }
            (total_written, total_rejected)
        })
    };

    // 轮询 tick
    let ticker = {
        let sm = Arc::clone(&sm);
        tokio::spawn(async move {
            for _ in 0..50 {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let mut guard = sm.lock().await;
                if let Some(_event) = guard.tick() {
                    return Some(_event);
                }
            }
            None
        })
    };

    let (written, rejected) = writer.await.unwrap();
    let timeout_event = ticker.await.unwrap();

    let total_attempted = total_batches * msgs_per_batch;
    assert_eq!(
        written + rejected,
        total_attempted,
        "all attempted writes should complete (either accepted or rejected)"
    );

    // 10s 窗口内 tick 应在 ~4s 时超时
    assert!(
        timeout_event.is_some(),
        "ticker must detect MigrationTimeout within 10s"
    );
    if let Some(event) = timeout_event {
        use zenoh_link_state::link_state::LinkEvent;
        assert!(matches!(event, LinkEvent::MigrationTimeout));
    }

    // 验证最终状态
    let guard = sm.lock().await;
    let queue_size = guard.queue_len();
    println!(
        "Written: {}, rejected (after timeout): {}, queue at end: {}",
        written, rejected, queue_size
    );
    // 超时后队列清空 + 连接断开
    assert!(guard.is_disconnected(), "should be Disconnected after timeout");
    assert_eq!(queue_size, 0, "queue must be discarded on timeout");
}

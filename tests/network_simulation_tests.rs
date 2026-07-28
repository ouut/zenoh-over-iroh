//! # 网络模拟测试 — Phase 3 第一批用例验证
//!
//! 基于 tokio 异步运行时，验证状态机在时间约束下的行为。
//!
//! 注意：以下测试涉及实际时间等待（最长 ~5s），运行时间较普通单元测试长。

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use zenoh_link_state::link_state::{LinkError, LinkStateMachine, WriteStatus};

// ── 测试 1：tick() 超时后进入 Disconnected ──────────────────
// 用例 4 核心验证：Migrating 超时 → Disconnected → 排队数据作废

#[tokio::test]
async fn test_tick_timeout_enters_disconnected() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    // 进入 Migrating
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(false);
        assert!(guard.is_migrating());
        guard.write(b"data_during_migration".to_vec()).unwrap();
        assert_eq!(guard.queue_len(), 1);
    }

    // 等待超过超时阈值 (4000ms + 500ms 安全裕度)
    // TODO: 待用例4/5实测数据标定后调整此值
    tokio::time::sleep(Duration::from_millis(4500)).await;

    // tick() 应触发超时
    {
        let mut guard = sm.lock().await;
        let event = guard.tick();
        assert!(
            event.is_some(),
            "tick() should return MigrationTimeout after 4.5s"
        );
        assert!(guard.is_disconnected(), "should be Disconnected after timeout");
        assert_eq!(guard.queue_len(), 0, "queued data must be discarded on timeout");
    }

    // Disconnected 后 write 应报错
    {
        let mut guard = sm.lock().await;
        assert_eq!(guard.write(b"post_timeout".to_vec()), Err(LinkError::Disconnected));
        assert_eq!(guard.read(), Err(LinkError::Disconnected));
    }
}

// ── 测试 2：快速恢复（未超时）回到 Connected ──────────────────
// 用例 4 核心验证：短时间失联 → Migrating → 恢复 → Connected

#[tokio::test]
async fn test_quick_recovery_within_timeout() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    // 进入 Migrating
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(false);
        assert!(guard.is_migrating());
        guard.write(b"quick_recovery_data".to_vec()).unwrap();
        guard.write(b"more_data".to_vec()).unwrap();
        assert_eq!(guard.queue_len(), 2);
    }

    // 仅等待 1 秒（远短于超时），模拟快速网络切换
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // tick() 不应超时
    {
        let mut guard = sm.lock().await;
        let event = guard.tick();
        assert!(event.is_none(), "tick() should not timeout after 1s");
        assert!(guard.is_migrating(), "still Migrating after short wait");
    }

    // 路径恢复
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(true);
        assert!(guard.is_connected(), "should recover to Connected");
    }

    // 排队数据可正确排出
    {
        let mut guard = sm.lock().await;
        let drained: Vec<_> = guard.drain_queue().into_iter().collect();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], b"quick_recovery_data".to_vec());
        assert_eq!(drained[1], b"more_data".to_vec());
        assert_eq!(guard.queue_len(), 0);
    }
}

// ── 测试 3：并发写入 Migrating 态 ─────────────────────────────
// 多个 task 同时在 Migrating 态 write，验证并发安全

#[tokio::test]
async fn test_concurrent_writes_during_migration() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    // 进入 Migrating
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(false);
    }

    // 10 个并发 task 同时 write
    let mut handles = vec![];
    for i in 0..10 {
        let sm_clone = Arc::clone(&sm);
        handles.push(tokio::spawn(async move {
            let mut guard = sm_clone.lock().await;
            let data = format!("concurrent_{}", i).into_bytes();
            guard.write(data).unwrap()
        }));
    }

    // 等待全部完成
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // 所有写入都应返回 Queued
    for r in &results {
        assert_eq!(*r, WriteStatus::Queued);
    }

    // 验证队列中有 10 条数据
    {
        let guard = sm.lock().await;
        assert_eq!(guard.queue_len(), 10);
    }

    // 恢复并排出
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(true);
        let drained: Vec<_> = guard.drain_queue().into_iter().collect();
        assert_eq!(drained.len(), 10);
    }
}

// ── 测试 4：tick() 轮询循环 ──────────────────────────────────
// 模拟真实 LinkUnicast 中的定时轮询模式

#[tokio::test]
async fn test_tick_polling_loop() {
    let sm = Arc::new(Mutex::new(LinkStateMachine::new()));

    // 进入 Migrating
    {
        let mut guard = sm.lock().await;
        guard.on_path_change(false);
        guard.write(b"poll_data".to_vec()).unwrap();
    }

    let sm_clone = Arc::clone(&sm);
    let poll_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut guard = sm_clone.lock().await;
            if let Some(event) = guard.tick() {
                return event;
            }
        }
    });

    // 轮询应在 ~4s 后检测到超时
    let result = tokio::time::timeout(Duration::from_secs(6), poll_handle)
        .await
        .expect("poll should complete within 6s")
        .expect("poll task should not panic");

    use zenoh_link_state::link_state::LinkEvent;
    assert!(matches!(result, LinkEvent::MigrationTimeout));

    // 验证最终状态
    {
        let guard = sm.lock().await;
        assert!(guard.is_disconnected());
        assert_eq!(guard.queue_len(), 0);
    }
}

// ── 测试 5：反复迁移周期（多次快速切换）──────────────────────

#[tokio::test]
async fn test_multiple_migration_cycles() {
    let mut sm = LinkStateMachine::new();

    for cycle in 0..3 {
        // 失联
        sm.on_path_change(false);
        assert!(sm.is_migrating(), "cycle {}: should be Migrating", cycle);

        // 写入排队
        sm.write(format!("cycle_{}", cycle).into_bytes()).unwrap();

        // 短等待（模拟 Quick Recovery）
        tokio::time::sleep(Duration::from_millis(500)).await;

        // tick 不应超时
        assert!(sm.tick().is_none(), "cycle {}: should not timeout", cycle);

        // 恢复
        sm.on_path_change(true);
        assert!(sm.is_connected(), "cycle {}: should recover", cycle);

        // 排出验证
        let drained: Vec<_> = sm.drain_queue().into_iter().collect();
        assert_eq!(drained.len(), 1, "cycle {}: should have 1 queued item", cycle);
        assert_eq!(drained[0], format!("cycle_{}", cycle).into_bytes());
    }
}

// ── 测试 6：极短失联 + 超快恢复（模拟网络微闪）───────────────

#[tokio::test]
async fn test_micro_flash_migration() {
    let mut sm = LinkStateMachine::new();

    // 失联
    sm.on_path_change(false);
    sm.write(b"micro_flash".to_vec()).unwrap();

    // 极短等待 50ms
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 恢复
    sm.on_path_change(true);
    assert!(sm.is_connected());

    let drained: Vec<_> = sm.drain_queue().into_iter().collect();
    assert_eq!(drained, vec![b"micro_flash".to_vec()]);
}

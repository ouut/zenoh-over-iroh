//! # LinkState 三态状态机集成测试
//!
//! 对应需求文档 1.4 节的验收标准：
//! - 迁移成功回到 Connected（Zenoh 无感知）
//! - 迁移超时进入 Disconnected（排队数据正确作废）
//! - Migrating 期间写阻塞排队不报错
//! - 恢复后排队数据正确发送
//! - Disconnected 态操作全部报错

use zenoh_link_state::link_state::{LinkError, LinkStateMachine, WriteStatus};

// ── 测试 1：正常迁移路径 ──────────────────────────────────────
// Connected → Migrating → Connected（Zenoh 无感知）

#[test]
fn test_normal_migration_cycle() {
    let mut sm = LinkStateMachine::new();

    // 阶段 1：Connected 正常读写
    assert!(sm.is_connected());
    assert_eq!(sm.read(), Ok(()));
    assert_eq!(sm.write(b"pre_migration".to_vec()), Ok(WriteStatus::Sent));

    // 阶段 2：路径失联 → Migrating
    let event = sm.on_path_change(false);
    assert!(event.is_some());
    assert!(sm.is_migrating());

    // 阶段 3：Migrating 期间写入排队不报错
    for i in 0..5 {
        let data = format!("msg_{}", i).into_bytes();
        let result = sm.write(data);
        assert!(result.is_ok(), "write during migration should not fail");
        assert_eq!(
            result.unwrap(),
            WriteStatus::Queued,
            "write during migration should queue"
        );
    }
    assert_eq!(sm.queue_len(), 5);

    // 阶段 4：路径恢复 → Connected
    let event = sm.on_path_change(true);
    assert!(event.is_some());
    assert!(sm.is_connected());

    // 阶段 5：排队数据可正确排出
    let drained: Vec<_> = sm.drain_queue().into_iter().collect();
    assert_eq!(drained.len(), 5);
    for (i, data) in drained.iter().enumerate() {
        assert_eq!(*data, format!("msg_{}", i).into_bytes());
    }

    // 阶段 6：恢复后正常写入
    assert_eq!(
        sm.write(b"post_migration".to_vec()),
        Ok(WriteStatus::Sent)
    );
}

// ── 测试 2：迁移超时路径 ──────────────────────────────────────
// Connected → Migrating → (tick 超时) → Disconnected

#[test]
fn test_migration_timeout_discards_data() {
    let mut sm = LinkStateMachine::new();

    // 进入 Migrating
    sm.on_path_change(false);
    assert!(sm.is_migrating());

    // 排队数据
    sm.write(b"will_be_discarded_1".to_vec()).unwrap();
    sm.write(b"will_be_discarded_2".to_vec()).unwrap();
    sm.write(b"will_be_discarded_3".to_vec()).unwrap();
    assert_eq!(sm.queue_len(), 3);

    // 模拟超时断开
    sm.disconnect();
    assert!(sm.is_disconnected());
    assert_eq!(sm.queue_len(), 0, "queue must be cleared on disconnect");

    // 验证断开后写入报错
    assert_eq!(sm.write(b"new_data".to_vec()), Err(LinkError::Disconnected));
    assert_eq!(sm.read(), Err(LinkError::Disconnected));
}

// ── 测试 3：迁移中路径反复切换 ────────────────────────────────
// Connected → Migrating → Connected → Migrating → Connected

#[test]
fn test_repeated_migration_cycles() {
    let mut sm = LinkStateMachine::new();

    for cycle in 0..3 {
        // 失联
        sm.on_path_change(false);
        assert!(
            sm.is_migrating(),
            "cycle {}: should be migrating after path loss",
            cycle
        );

        // 排队
        sm.write(format!("cycle_{}_data", cycle).into_bytes())
            .unwrap();
        assert_eq!(sm.queue_len(), 1);

        // 恢复
        sm.on_path_change(true);
        assert!(
            sm.is_connected(),
            "cycle {}: should be connected after recovery",
            cycle
        );

        // 排出
        let drained: Vec<_> = sm.drain_queue().into_iter().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0], format!("cycle_{}_data", cycle).into_bytes());
    }
}

// ── 测试 4：Disconnected 态所有操作报错 ────────────────────────

#[test]
fn test_disconnected_rejects_all_operations() {
    let mut sm = LinkStateMachine::new();
    sm.disconnect();

    // write 报错
    assert_eq!(sm.write(b"data".to_vec()), Err(LinkError::Disconnected));
    // read 报错
    assert_eq!(sm.read(), Err(LinkError::Disconnected));
    // drain_queue 返回空
    assert!(sm.drain_queue().is_empty());
    // 路径事件无操作
    assert!(sm.on_path_change(true).is_none());
    assert!(sm.on_path_change(false).is_none());
}

// ── 测试 5：空迁移序列（立即恢复） ─────────────────────────────

#[test]
fn test_instant_recovery_no_data_loss() {
    let mut sm = LinkStateMachine::new();

    // 失联后立即恢复（模拟极短暂网络抖动）
    sm.on_path_change(false);
    assert!(sm.is_migrating());

    // 恢复（无数据排队）
    sm.on_path_change(true);
    assert!(sm.is_connected());
    assert_eq!(sm.queue_len(), 0);

    // 后续写入正常
    assert_eq!(sm.write(b"after_jitter".to_vec()), Ok(WriteStatus::Sent));
}

// ── 测试 6：write 在 Connected 态正确返回 Sent ─────────────────

#[test]
fn test_write_connected_returns_sent() {
    let mut sm = LinkStateMachine::new();
    for i in 0..10 {
        let result = sm.write(vec![i as u8; 100]);
        assert_eq!(
            result,
            Ok(WriteStatus::Sent),
            "write {} in connected state should return Sent",
            i
        );
    }
    assert_eq!(sm.queue_len(), 0, "no data should be queued in connected state");
}

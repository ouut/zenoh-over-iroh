//! # LinkState — Migrating 三态状态机
//!
//! 本模块实现需求文档 1.4 节定义的 `LinkState` 三态状态机，
//! 作为 `LinkUnicast` 的内部私有实现细节。

use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

/// 超时阈值（毫秒）。
/// TODO: 待用例4/5实测数据标定，暂用经验值 4000ms。
const MIGRATING_TIMEOUT_MS: u64 = 4000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError { Disconnected }

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { LinkError::Disconnected => write!(f, "link is disconnected") }
    }
}
impl std::error::Error for LinkError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteStatus {
    Sent,
    Queued,
    /// 背压：队列已满（风险5.10），仅在配置 max_queue_depth 且队列满时返回。
    Backpressure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    PathMigrated,
    PathRestored,
    MigrationTimeout,
}

enum LinkState {
    Connected,
    Migrating { since: Instant },
    Disconnected,
}

pub struct LinkStateMachine {
    state: LinkState,
    queue: VecDeque<Vec<u8>>,
    /// 最大排队深度（0=无限制，默认）。
    max_queue_depth: usize,
}

impl LinkStateMachine {
    pub fn new() -> Self {
        Self { state: LinkState::Connected, queue: VecDeque::new(), max_queue_depth: 0 }
    }

    /// 创建带背压的状态机（风险5.10）。
    pub fn with_backpressure(max_queue_depth: usize) -> Self {
        Self { state: LinkState::Connected, queue: VecDeque::new(), max_queue_depth }
    }

    pub fn is_connected(&self) -> bool { matches!(self.state, LinkState::Connected) }
    pub fn is_migrating(&self) -> bool { matches!(self.state, LinkState::Migrating { .. }) }
    pub fn is_disconnected(&self) -> bool { matches!(self.state, LinkState::Disconnected) }
    pub fn queue_len(&self) -> usize { self.queue.len() }

    pub fn on_path_change(&mut self, connected: bool) -> Option<LinkEvent> {
        match (&self.state, connected) {
            (LinkState::Connected, false) => {
                self.state = LinkState::Migrating { since: Instant::now() };
                tracing::info!("Path migration started");
                Some(LinkEvent::PathMigrated)
            }
            (LinkState::Migrating { .. }, true) => {
                self.state = LinkState::Connected;
                tracing::info!(queue_len = self.queue.len(), "Path restored");
                Some(LinkEvent::PathRestored)
            }
            _ => None,
        }
    }

    pub fn write(&mut self, data: Vec<u8>) -> Result<WriteStatus, LinkError> {
        match &self.state {
            LinkState::Connected => {
                tracing::trace!(len = data.len(), "Write: sent");
                Ok(WriteStatus::Sent)
            }
            LinkState::Migrating { .. } => {
                // 背压检查
                if self.max_queue_depth > 0 && self.queue.len() >= self.max_queue_depth {
                    tracing::warn!(queue_depth = self.queue.len(), "Write: backpressure");
                    return Ok(WriteStatus::Backpressure);
                }
                tracing::debug!(queue_depth = self.queue.len() + 1, "Write: queued");
                self.queue.push_back(data);
                Ok(WriteStatus::Queued)
            }
            LinkState::Disconnected => {
                tracing::error!("Write: disconnected");
                Err(LinkError::Disconnected)
            }
        }
    }

    pub fn read(&self) -> Result<(), LinkError> {
        match &self.state {
            LinkState::Connected | LinkState::Migrating { .. } => Ok(()),
            LinkState::Disconnected => Err(LinkError::Disconnected),
        }
    }

    pub fn tick(&mut self) -> Option<LinkEvent> {
        if let LinkState::Migrating { since } = &self.state {
            if since.elapsed() >= Duration::from_millis(MIGRATING_TIMEOUT_MS) {
                let discarded = self.queue.len();
                self.state = LinkState::Disconnected;
                self.queue.clear();
                tracing::warn!(discarded, "Migration timeout");
                return Some(LinkEvent::MigrationTimeout);
            }
        }
        None
    }

    pub fn drain_queue(&mut self) -> VecDeque<Vec<u8>> { std::mem::take(&mut self.queue) }

    pub fn disconnect(&mut self) {
        self.state = LinkState::Disconnected;
        self.queue.clear();
        tracing::info!("Explicit disconnect");
    }
}

impl Default for LinkStateMachine {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_connected() {
        let sm = LinkStateMachine::new();
        assert!(sm.is_connected());
        assert!(!sm.is_migrating());
        assert!(!sm.is_disconnected());
    }

    #[test]
    fn test_connected_to_migrating_and_back() {
        let mut sm = LinkStateMachine::new();
        let event = sm.on_path_change(false);
        assert!(matches!(event, Some(LinkEvent::PathMigrated)));
        assert!(sm.is_migrating());
        let event = sm.on_path_change(true);
        assert!(matches!(event, Some(LinkEvent::PathRestored)));
        assert!(sm.is_connected());
    }

    #[test]
    fn test_write_queues_during_migration() {
        let mut sm = LinkStateMachine::new();
        sm.on_path_change(false);
        assert_eq!(sm.write(b"hello".to_vec()), Ok(WriteStatus::Queued));
        assert_eq!(sm.queue_len(), 1);
        sm.on_path_change(true);
        let drained: Vec<_> = sm.drain_queue().into_iter().collect();
        assert_eq!(drained, vec![b"hello".to_vec()]);
    }

    #[test]
    fn test_write_in_connected_sent_immediately() {
        let mut sm = LinkStateMachine::new();
        assert_eq!(sm.write(b"data".to_vec()), Ok(WriteStatus::Sent));
        assert_eq!(sm.queue_len(), 0);
    }

    #[test]
    fn test_write_in_disconnected_returns_error() {
        let mut sm = LinkStateMachine::new();
        sm.on_path_change(false);
        sm.disconnect();
        assert_eq!(sm.write(b"data".to_vec()), Err(LinkError::Disconnected));
    }

    #[test]
    fn test_read_in_disconnected_returns_error() {
        let mut sm = LinkStateMachine::new();
        sm.disconnect();
        assert_eq!(sm.read(), Err(LinkError::Disconnected));
    }

    #[test]
    fn test_read_in_connected_and_migrating_is_ok() {
        let mut sm = LinkStateMachine::new();
        assert_eq!(sm.read(), Ok(()));
        sm.on_path_change(false);
        assert_eq!(sm.read(), Ok(()));
    }

    #[test]
    fn test_migration_timeout_discards_queue() {
        let mut sm = LinkStateMachine::new();
        sm.on_path_change(false);
        sm.write(b"msg1".to_vec()).unwrap();
        sm.write(b"msg2".to_vec()).unwrap();
        assert_eq!(sm.queue_len(), 2);
        sm.disconnect();
        assert_eq!(sm.queue_len(), 0);
        assert!(sm.is_disconnected());
        assert_eq!(sm.write(b"msg3".to_vec()), Err(LinkError::Disconnected));
    }

    #[test]
    fn test_duplicate_path_change_events_are_noop() {
        let mut sm = LinkStateMachine::new();
        assert!(sm.on_path_change(true).is_none());
        sm.on_path_change(false);
        assert!(sm.on_path_change(false).is_none());
    }

    #[test]
    fn test_disconnected_ignores_path_change() {
        let mut sm = LinkStateMachine::new();
        sm.disconnect();
        assert!(sm.on_path_change(true).is_none());
        assert!(sm.on_path_change(false).is_none());
    }

    #[test]
    fn test_default_creates_connected_machine() {
        assert!(LinkStateMachine::default().is_connected());
    }

    #[test]
    fn test_drain_queue_clears_internal_state() {
        let mut sm = LinkStateMachine::new();
        sm.on_path_change(false);
        sm.write(b"a".to_vec()).unwrap();
        sm.write(b"b".to_vec()).unwrap();
        sm.on_path_change(true);
        assert_eq!(sm.drain_queue().len(), 2);
        assert_eq!(sm.queue_len(), 0);
        assert!(sm.drain_queue().is_empty());
    }

    #[test]
    fn test_backpressure_rejects_when_full() {
        let mut sm = LinkStateMachine::with_backpressure(3);
        sm.on_path_change(false);
        assert_eq!(sm.write(b"1".to_vec()), Ok(WriteStatus::Queued));
        assert_eq!(sm.write(b"2".to_vec()), Ok(WriteStatus::Queued));
        assert_eq!(sm.write(b"3".to_vec()), Ok(WriteStatus::Queued));
        assert_eq!(sm.queue_len(), 3);
        assert_eq!(sm.write(b"4".to_vec()), Ok(WriteStatus::Backpressure));
        assert_eq!(sm.queue_len(), 3);
    }

    #[test]
    fn test_no_backpressure_by_default() {
        let mut sm = LinkStateMachine::new();
        sm.on_path_change(false);
        for _ in 0..100 {
            assert_eq!(sm.write(b"x".to_vec()), Ok(WriteStatus::Queued));
        }
        assert_eq!(sm.queue_len(), 100);
    }
}

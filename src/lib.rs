//! # zenoh-link-state
//!
//! `zenoh-link-iroh` 项目内部使用的 Migrating 三态状态机独立模块。
//!
//! 本 crate 实现需求文档 1.4 节定义的 `LinkState` 三态状态机，
//! 用于在 `LinkUnicast` 内部过滤 QUIC 路径迁移噪声，
//! 避免将短暂路径切换误判为 Zenoh 断连事件。
//!
//! ## 模块结构
//!
//! - [`link_state`]：核心状态机实现（`LinkStateMachine`、`LinkError`、`LinkEvent` 等）。
//!
//! ## 状态转换概览
//!
//! ```text
//! Connected ──(on_path_change(false))──> Migrating
//! Migrating ──(on_path_change(true))───> Connected
//! Migrating ──(tick() 超时)────────────> Disconnected
//! ```
//!
//! ## 使用示例
//!
//! ```rust
//! use zenoh_link_state::link_state::LinkStateMachine;
//!
//! let mut sm = LinkStateMachine::new();
//! assert!(sm.is_connected());
//!
//! // 路径变化触发迁移
//! sm.on_path_change(false);
//! assert!(sm.is_migrating());
//!
//! // 路径恢复
//! sm.on_path_change(true);
//! assert!(sm.is_connected());
//! ```

pub mod link_state;

/// Iroh 传输层集成模块（Phase 2 → Phase 3 桥梁）。
///
/// 此模块展示 `LinkStateMachine` 如何嵌入到 `zenoh-link-iroh` 的
/// LinkUnicast 实现中，作为 zenoh transport trait 和实际 Iroh IO 之间的
/// 非侵入式中间层。
pub mod iroh_integration;

/// C FFI 层 — 面向 iOS / Android 的 C ABI。
///
/// 通过 `extern "C"` 函数暴露 `LinkStateMachine`，
/// 可被 Swift (iOS) 和 Kotlin/Java (Android) 直接调用。
pub mod ffi;

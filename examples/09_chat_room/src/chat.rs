//! 聊天室用户身份与会话管理。
//!
//! 每个用户拥有唯一的 NodeID（Iroh 公钥指纹），
//! 加入房间后通过 zenoh pub/sub 收发消息。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 聊天用户。
#[derive(Debug, Clone)]
pub struct User {
    /// 用户名（用户自定义，可重复）。
    pub name: String,
    /// Iroh NodeID（公钥指纹，全局唯一）。
    pub node_id: String,
}

/// 聊天消息。
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// 发送者。
    pub sender: User,
    /// 消息正文。
    pub text: String,
    /// 时间戳（Unix 毫秒）。
    pub timestamp_ms: u64,
}

/// 聊天室状态。
pub struct ChatRoom {
    /// 房间名称 / zenoh key prefix。
    pub room_name: String,
    /// 当前用户。
    pub me: User,
    /// 已知用户列表（NodeID → User）。
    pub members: HashMap<String, User>,
    /// 消息历史（最近 N 条）。
    pub history: Vec<ChatMessage>,
    /// 是否已连接。
    pub connected: bool,
}

impl ChatRoom {
    pub fn new(room_name: String, me: User) -> Self {
        Self {
            room_name,
            me,
            members: HashMap::new(),
            history: Vec::with_capacity(200),
            connected: true,
        }
    }

    /// 构建 pub/sub 使用的 zenoh key。
    ///
    /// 格式: `chat/<room_name>/messages`
    pub fn message_key(&self) -> String {
        format!("chat/{}/messages", self.room_name)
    }

    /// 构建用户公告的 zenoh key。
    ///
    /// 格式: `chat/<room_name>/presence/<node_id>`
    pub fn presence_key(&self) -> String {
        format!("chat/{}/presence/{}", self.room_name, self.me.node_id)
    }

    /// 记录收到的新消息。
    pub fn record_message(&mut self, msg: ChatMessage) {
        // 保持最近 200 条
        if self.history.len() >= 200 {
            self.history.remove(0);
        }
        self.history.push(msg);
    }

    /// 注册新用户或更新已有用户。
    pub fn upsert_member(&mut self, user: User) {
        self.members.insert(user.node_id.clone(), user);
    }

    /// 移除离开的用户。
    pub fn remove_member(&mut self, node_id: &str) {
        self.members.remove(node_id);
    }

    /// 在线人数。
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

/// 聊天室管理器（线程安全）。
pub struct ChatManager {
    pub room: Arc<Mutex<ChatRoom>>,
}

impl ChatManager {
    pub fn new(room_name: &str, user_name: &str, node_id: &str) -> Self {
        let me = User {
            name: user_name.to_string(),
            node_id: node_id.to_string(),
        };
        Self {
            room: Arc::new(Mutex::new(ChatRoom::new(room_name.to_string(), me))),
        }
    }

    /// 格式化消息用于显示（不获取锁，由调用方提供自己的 node_id）。
    pub fn format_message(me_node_id: &str, msg: &ChatMessage) -> String {
        let ts = msg.timestamp_ms % 100000;
        let sender = if msg.sender.node_id == me_node_id {
            "👤 我".to_string()
        } else {
            format!("👤 {}", msg.sender.name)
        };
        format!("[{}] {}: {}", ts, sender, msg.text)
    }
}

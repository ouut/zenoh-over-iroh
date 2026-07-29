use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub from:      String,
    pub from_name: String,
    pub text:      String,
    pub time:      String,
    pub msg_type:  String,
}

pub struct Commands;

impl Commands {
    pub const HELP: &str = r#"
命令:
  /msg <user> <text>  发送私信
  /rooms              查看房间成员
  /quit               退出
"#;
}

pub fn format_msg(msg: &ChatMessage, is_me: bool) -> String {
    let prefix = if is_me { "→" } else { "←" };
    let tag = match msg.msg_type.as_str() {
        "dm"   => "🔒",
        _      => "📢",
    };
    format!("[{}] {} {} {}: {}", msg.time, prefix, tag, msg.from_name, msg.text)
}

# example 01 — iroh P2P 命令行聊天室

> 两个终端通过 Iroh QUIC 直连聊天。
> 包含 `LinkStateMachine` 状态管理示例。

## 快速开始

```bash
cd examples/01_chat_room

# 终端 A
cargo run -- Alice

# 终端 B  
cargo run -- Bob

# 终端 B 输入: /connect <Alice 的 NodeID>
# 然后双方即可聊天
```

## 命令

| 输入 | 行为 |
|------|------|
| 任何文字 | 发送给对方 |
| `/connect <NodeID>` | 连接到对方 |
| `/quit` | 退出 |

## 编译

```bash
cd examples/01_chat_room
cargo build              # 首次 ~3min (iroh 依赖)
```

## 文件结构

```
examples/01_chat_room/
├── Cargo.toml             # 依赖 iroh + zenoh-link-state
├── src/
│   ├── chat.rs            # ChatMessage 类型
│   └── main.rs            # iroh P2P 聊天主程序
├── send_test.sh           # 双终端自动测试脚本
└── README.md              # 本文档
```

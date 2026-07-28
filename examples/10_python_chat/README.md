# Python 聊天室 — LinkStateMachine 集成示例

> 使用 Python + TCP + LinkStateMachine 实现双向 P2P 聊天

## 快速开始

```bash
# 终端 1 (先启动，监听 9000)
cd examples/10_python_chat
python chat.py Alice --port 9000

# 终端 2 (连接 Alice)
python chat.py Bob --connect localhost:9000 --port 9001
```

双方输入文字即可互通。

## 命令

| 命令 | 说明 |
|------|------|
| `/connect host:port` | 连接到远端 |
| `/status` | 查看连接状态和排队数量 |
| `/demo` | 演示 LinkStateMachine 断网恢复 |
| `/quit` | 退出 |

## 架构

```
终端 A (chat.py Alice)              终端 B (chat.py Bob)
    │                                     │
    ├─ TCP Server :9000  ←──── 连接 ──── TCP Client
    ├─ TCP Client ──── 连接 ────→ TCP Server :9001
    │                                     │
    ├─ LinkStateMachine                    ├─ LinkStateMachine
    │   ├─ Connected → write() Sent        │   ├─ Connected
    │   ├─ Migrating → write() Queued      │   ├─ Migrating
    │   └─ Disconnected → write() Error    │   └─ Disconnected
    │                                     │
    └─ stdin reader (thread)              └─ stdin reader (thread)
```

## LinkStateMachine 行为

| 网络状态 | write() 结果 | 聊天体验 |
|---------|:---:|------|
| TCP 正常 | `sent` | 消息立即送达 ✅ |
| TCP 断开 | `queued` | 消息排队，恢复后自动发送 ⏳ |
| 排队溢出 (backpressure) | `backpressure` | 提示"队列已满" 🚫 |
| 显式断连 | `disconnected` | 提示"连接已断开" ❌ |

## 依赖

- Python 3.7+
- `libzenoh_link_state.so` (编译 Rust crate)
- 将 `.so` 和 `zenoh_link_state.py` 放在 Python path 中

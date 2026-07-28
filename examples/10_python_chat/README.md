# Python 聊天室 — iroh P2P + LinkStateMachine

> Example 09 的 Python 复刻：spawn Rust iroh 二进制处理 QUIC P2P，
> Python 侧提供 UI + LinkStateMachine 状态管理。

## 快速开始

```bash
# 1. 编译 Rust iroh 聊天二进制
cd examples/09_chat_room
cargo build --release

# 2. 终端 1
cd examples/10_python_chat
python chat.py Alice

# 3. 终端 2 (连接 Alice 的 NodeID)
cd examples/10_python_chat
python chat.py Bob
# > /connect <Alice的NodeID>
```

## 架构

```
┌─────────────────────────────────┐
│  Python chat.py                 │  ← 本文件
│  ├─ LinkStateMachine (FFI)      │     连接状态管理
│  ├─ UI (stdin/stdout)           │     命令行交互
│  └─ Subprocess                  │
│       │ stdin/stdout             │
│       ▼                          │
│  Rust chat binary (example 09)  │  ← 真实 iroh QUIC P2P
│  ├─ Iroh Endpoint               │
│  ├─ QUIC P2P / Relay            │
│  └─ Message routing             │
└─────────────────────────────────┘
```

## 通信流程

1. Python 启动 Rust `chat` 二进制作为子进程
2. Python 从 Rust stdout 提取 NodeID
3. 用户输入 → Python 写入子进程 stdin
4. 子进程通过 iroh QUIC 发送到对端
5. 对端消息 → Rust stdout → Python 解析显示

## LinkStateMachine 行为

| 场景 | write() | 效果 |
|------|:---:|------|
| 正常 | sent | 消息通过 iroh 送达 |
| 断网 | queued | 消息排队 |
| 恢复 | drain() | 排队消息自动排出 |
| 溢出 | backpressure | 提示用户稍候 |

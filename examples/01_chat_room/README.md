# example 01 — P2P 聊天室 (Zenoh API + Iroh 传输层)

> 命令行聊天室，支持群聊+私信。
> 传输层可一键切换 TCP ↔ Iroh P2P（配置改一行即可）。

---

## 快速开始

```bash
cd examples/01_chat_room

# 终端 1
cargo run --release -- Alice lobby

# 终端 2
cargo run --release -- Bob lobby

# 输入文字 → 群发
# /msg Bob 你好 → 私信
```

## 配置切换传输层

```rust
// 当前用 TCP（稳定可用）
"listen": { "endpoints": ["tcp/127.0.0.1:0"] },

// 改为 Iroh P2P（需插件，改一行即可）
"listen": { "endpoints": ["iroh/0.0.0.0:0"] },
```

其他代码零改动——这就是 Zenoh 传输层可插拔的设计。

## 编译

```bash
cd examples/01_chat_room
cargo build --release    # 首次 ~5min
```

## 命令

| 输入 | 行为 |
|------|------|
| 任何文字 | 群发到房间 |
| `/msg Bob hello` | 私信 Bob |
| `/rooms` | 查看成员 |
| `/quit` | 退出 |

# example 09 — Zenoh 聊天室 (Iroh 传输层)

> 演示 Zenoh API 在 Iroh 传输层上的使用。
> 用户只写 Zenoh API，Iroh 和 LinkStateMachine 完全透明。

---

## 正确使用方式（生产环境）

```rust
use zenoh::prelude::r#async::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ★ 唯一改动：配置端点为 iroh/，其他代码和 TCP 完全一样
    let session = zenoh::open(r#"
        mode: "peer",
        listen: { endpoints: ["iroh/0.0.0.0:0"] }
    "#).await?;

    // ── 标准 Zenoh API ──
    let sub = session.declare_subscriber("chat/lobby").await?;
    session.put("chat/lobby", "hello").await?;

    while let Ok(msg) = sub.recv_async().await {
        println!("[{}] {}", msg.key_expr(), msg.payload().to_string());
    }
    Ok(())
}
```

**关键事实**：`listen: ["iroh/..."]` 这一行配置切换传输层到 Iroh，其余代码与 TCP 版本完全一致。

---

## example 09 当前代码说明

当前 `src/` 下的代码（`iroh_chat.rs` + `main.rs`）是**底层传输验证**——直接使用 iroh crate 的 API，目的是验证 Iroh P2P 的可行性和 LinkStateMachine 的集成。生产环境应使用 `zenoh::open()` 加 `"iroh/..."` 配置，不要直接操作 iroh。

### 架构关系

```
生产环境 (推荐)              example 09 当前代码
─────────────────          ─────────────────────
session.put("k","v")       iroh.connect(node_id)
     │                            │
zenoh-core → iroh link     直接操作 iroh::Endpoint
     │                            │
LinkStateMachine (透明)     LinkStateMachine (显式)
```

### 两种模式的详细对比

| 维度 | 生产环境 | example 09 |
|------|---------|------------|
| API | `session.put("k","v")` | `endpoint.connect(id)` |
| 学到的东西 | Zenoh pub/sub | iroh 底层 API |
| iroh 是否可见 | 完全透明 | 可见，需手动管理 |
| 链路状态机 | 自动运行 | 显式调用 |
| 适用场景 | 实际产品开发 | 底层协议验证 |
| 需要 zenoh crate | ✅ | ❌ |
| 编译时间 | ~5min | ~3min |

---

## 完整配置示例

```rust
// 内网 3 节点互连，使用 Iroh P2P
let config = r#"
{
    mode: "peer",
    listen: { endpoints: ["iroh/0.0.0.0:0"] },
    connect: { endpoints: [
        "iroh/<node_b_id>",
        "iroh/<node_c_id>"
    ]}
}"#;
```

## 二维码扫码连接

```rust
let session = zenoh::open(r#"
    mode: "peer",
    listen: { endpoints: ["iroh/0.0.0.0:0"] }
"#).await?;

// session 的 NodeID 可通过管理 API 获取
let my_node_id = session.info().zid().to_string();
// 生成二维码: qrencode "zenoh:iroh:{my_node_id}"
```

# example 11 — 内网扫码消息接收器 (Zenoh + Iroh)

> 一台 PC 显示二维码。手机扫码后通过 iroh P2P 连接，发送消息。
> PC 打印收到的消息。
> 用户使用 Zenoh API。

---

## 正确使用方式（生产环境）

### PC 端

```rust
use zenoh::prelude::r#async::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let session = zenoh::open(r#"
        mode: "peer",
        listen: { endpoints: ["iroh/0.0.0.0:0"] }
    "#).await?;

    // 获取 NodeID → 生成二维码
    let node_id = session.info().zid();
    println!("QR payload: iroh:{node_id}");

    // 订阅消息
    let sub = session.declare_subscriber("msg/*").await?;
    while let Ok(msg) = sub.recv_async().await {
        println!("📩 {}: {}", msg.key_expr(), msg.payload().to_string());
    }
    Ok(())
}
```

### 手机端

```python
import zenoh

config = {
    "mode": "peer",
    "connect": {"endpoints": ["iroh/<PC的NodeID>"]},
}
session = zenoh.open(config)

# 发送消息给 PC
session.put("msg/from_phone", "hello from phone!")

# 多个手机可以同时发送，PC 端统一接收
session.put("msg/phone2", "hi from 2nd phone")
```

---

## 当前 example 11 说明

当前 `src/main.rs` 直接使用 `iroh::Endpoint` API，是**底层传输验证**。生产环境应使用 `zenoh::open()` + `"iroh/..."` 配置。

| 方式 | 推荐？ | 理由 |
|------|:---:|------|
| `zenoh::open()` + iroh 配置 | ✅ | 一行代码，Zenoh 管所有 |
| 手写 iroh `accept_bi()` 循环 | ❌ | 重复造轮子，需自己处理路由 |

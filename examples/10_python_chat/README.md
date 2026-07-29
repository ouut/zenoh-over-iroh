# example 10 — Python Zenoh 聊天室 (Iroh 传输层)

> 使用 `eclipse-zenoh` Python 包，搭配 Iroh 传输层。
> 用户只写 Zenoh API，不需要知道 Iroh 的细节。

---

## 正确使用方式

```python
import zenoh

# 唯一改动：配置端点为 iroh/
config = {
    "mode": "peer",
    "listen": {"endpoints": ["iroh/0.0.0.0:0"]},
}

session = zenoh.open(config)

# ── 标准 Zenoh API，和 TCP 完全一样 ──

# 发布
session.put("chat/lobby", "hello from Python")

# 订阅
sub = session.declare_subscriber("chat/lobby")
for msg in sub:
    print(f"[{msg.key_expr()}] {msg.payload.decode()}")

# 查询
responses = session.get("chat/**").wait()
for r in responses:
    print(f"  {r.key_expr}: {r.payload.decode()}")
```

---

## 当前 example 10 说明

当前 `chat.py` 通过 subprocess 调用 Rust iroh 二进制，是**底层传输验证**。生产环境直接用 `pip install eclipse-zenoh` 加 `"iroh/..."` 配置即可，不需要 subprocess。

| 方式 | 推荐？ | 理由 |
|------|:---:|------|
| `pip install eclipse-zenoh` + iroh 配置 | ✅ | 标准 API，生态兼容 |
| subprocess + iroh 二进制 | ❌ | 仅用于底层验证 |

---

## 安装

```bash
pip install eclipse-zenoh
```

无需安装 iroh、无需编译 Rust。

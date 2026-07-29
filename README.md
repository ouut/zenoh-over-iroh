# zenoh-link-iroh — Zenoh × Iroh P2P Transport Plugin

> 让 Zenoh 跑在 Iroh 的 P2P 网络上，解决复杂 NAT/移动网络下的连通性问题。

---

## 项目状态

```
Phase 3 第一批 ✅  第二批 ✅  第三批 ⏳
33/33 tests PASS  │  crates.io: zenoh-link-state  │  CI: 8 平台自动构建
```

---

## 产出物

### 产物一：传输层插件（给 zenohd 用）

```
文件名: libzenoh_link_iroh.so / .dylib / .dll
源码在: zenoh-link-iroh/
编译:   cargo build -p zenoh-link-iroh --release
用法:   zenohd -P iroh_link
原理:   zenohd 运行时动态加载 .so，注册 "iroh/" endpoint scheme
大小:   ~2MB
```

加载后，Zenoh 配置中 `"iroh/"` 生效：

```yaml
listen:
  endpoints: ["iroh/0.0.0.0:0"]
```

### 产物二：跨语言完整库（给开发者用）

```
文件名: libzenoh_over_iroh.so / .dylib / .a / .dll
源码在: bindings/ (Cargo.toml + src/lib.rs)
编译:   cargo build -p zenoh-over-iroh --release
用法:   C / Python / Swift / Kotlin 直接调用
大小:   ~20MB（包含 Zenoh 完整 pub/sub + Iroh P2P）
```

包含 `z_open()`, `z_put()`, `z_subscribe()` 等完整 Zenoh C API **和 Iroh 传输层**。

| 语言 | 文件 | 示例 |
|------|------|------|
| C | `bindings/c/zenoh_over_iroh.h` | `z_open(cfg)` |
| Python | `bindings/python/zenoh.py` | `open_session(cfg)` |
| Swift | `bindings/swift/ZenohMobile.swift` | `ZenohMobile.open()` |
| Kotlin | `bindings/kotlin/ZenohMobile.kt` | `ZenohMobile.open()` |

```python
# Python 示例
from bindings.python.zenoh import open_session, put
s = open_session('{"listen":{"endpoints":["iroh/0.0.0.0:0"]}}')
put(s, "hello", "world")
```

---

## 目录结构

```
项目根
├── src/                    # 核心状态机库 (crates.io: zenoh-link-state)
├── zenoh-link-iroh/        # 产物一：传输层插件 .so
├── bindings/               # 产物二：跨语言完整库
│   ├── src/lib.rs          #   Rust FFI 层
│   ├── c/                  #   C 头文件
│   ├── python/             #   Python ctypes 绑定
│   ├── swift/              #   Swift (iOS)
│   ├── kotlin/             #   Kotlin (Android)
│   └── build.sh            #   一键编译
├── tests/                  # Rust 测试 (33 tests)
├── examples/               # 示例
├── scripts/                # 测试脚本
└── doc/                    # 文档
```

## 快速开始

```bash
# 1. 编译并测试核心库
cargo test                          # 33/33 PASS

# 2. 编译产物一（传输层插件）
cargo build -p zenoh-link-iroh --release
# → target/release/libzenoh_link_iroh.so

# 3. 编译产物二（跨语言库）
cargo build -p zenoh-over-iroh --release
# → target/release/libzenoh_over_iroh.so

# 4. Python 测试
python3 -c "
from bindings.python.zenoh import open_session, put, close
s = open_session('{\"listen\":{\"endpoints\":[\"tcp/127.0.0.1:0\"]}}')
put(s, 'test', 'hello')
close(s)
print('OK')
"
```

## 编译到其他平台

```bash
./bindings/build.sh plugin    # 仅产物一（所有桌面平台）
./bindings/build.sh all       # 全部（含 iOS/Android）
```

## 文档

| 文档 | 说明 |
|------|------|
| `doc/使用Zenoh-over-Iroh的正确方式.md` | 架构原理 + 移动端/桌面端使用 |
| `doc/状态机设计说明.md` | 三态状态机设计 |
| `doc/插件集成指南.md` | 如何编译插件并集成到 zenohd |
| `doc/自建Relay部署方案.md` | 生产环境 Relay 部署 |
| `doc/Phase4-前置设计文档.md` | 下阶段规划 |

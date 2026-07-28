# zenoh-link-state FFI API Reference

> 13 个 C ABI 函数 + 所有语言的调用签名。
> 基于 `src/ffi.rs`，自动生成对应各语言的 binding。

---

## 1. 对象模型

| C 类型 | 说明 |
|------|------|
| `zenoh_lsm_t` | Opaque handle，指向 `LinkStateMachine` 实例 |

所有函数通过指针操作，调用方负责 `new → use → free` 生命周期。

---

## 2. 生命周期管理

### `zenoh_lsm_new`

```c
zenoh_lsm_t* zenoh_lsm_new(void);
```

创建新状态机，初始状态 `Connected`，无背压限制。

| 参数 | 类型 | 说明 |
|------|------|------|
| — | — | — |

| 返回 | 说明 |
|------|------|
| 非 NULL | 成功，opaque handle |
| NULL | 不会发生（内存分配失败则 abort） |

```python
lsm = LinkStateMachine()                    # Python
```
```lua
local ptr = lsm.new()                       -- Lua
```
```swift
let lsm = LinkStateMachine()                // Swift
```
```kotlin
val lsm = LinkStateMachine()                // Kotlin
```

---

### `zenoh_lsm_new_with_backpressure`

```c
zenoh_lsm_t* zenoh_lsm_new_with_backpressure(uint32_t max_queue);
```

创建带背压的状态机。排队超过 `max_queue` 时返回 `Backpressure`。

| 参数 | 类型 | 说明 |
|------|------|------|
| `max_queue` | `uint32_t` | 最大排队深度，0 = 无限制 |

| 返回 | 说明 |
|------|------|
| 非 NULL | 成功 |
| — | — |

```python
lsm = LinkStateMachine(max_queue_depth=5)    # Python
```
```lua
local ptr = lsm.new(5)                       -- Lua
```

---

### `zenoh_lsm_free`

```c
void zenoh_lsm_free(zenoh_lsm_t* lsm);
```

释放状态机。必须在不再使用时调用，**与 `new` 一一配对**。

| 参数 | 类型 | 说明 |
|------|------|------|
| `lsm` | `zenoh_lsm_t*` | 要释放的 handle，NULL 安全 |

```python
del lsm                                      # Python: __del__ 自动调用
```
```lua
lsm.free(ptr)                                -- Lua
```

---

## 3. 路径事件

### `zenoh_lsm_on_path_change`

```c
int zenoh_lsm_on_path_change(zenoh_lsm_t* lsm, int connected);
```

通知状态机路径状态变化。**不参与重连决策**，仅驱动状态转换。

| 参数 | 类型 | 说明 |
|------|------|------|
| `lsm` | `zenoh_lsm_t*` | — |
| `connected` | `int` | 1 = 路径恢复, 0 = 路径失联 |

| 返回值 | 含义 | 说明 |
|:---:|------|------|
| 0 | `None` | 无状态变化（重复事件或无效状态） |
| 1 | `PathMigrated` | Connected → Migrating |
| 2 | `PathRestored` | Migrating → Connected |
| 3 | `MigrationTimeout` | (此函数不返回此值) |

```python
event = lsm.on_path_change(False)            # → "path_migrated"
event = lsm.on_path_change(True)             # → "path_restored"
event = lsm.on_path_change(True)             # → "none" (已是Connected)
```
```lua
lsm.on_path_change(ptr, false)               -- → "path_migrated"
```
```swift
lsm.onPathChange(connected: false)           // → .pathMigrated
```
```kotlin
lsm.onPathChange(false)                      // → LinkEvent.PATH_MIGRATED
```

---

## 4. 数据写入

### `zenoh_lsm_write`

```c
int zenoh_lsm_write(zenoh_lsm_t* lsm, const uint8_t* data, uint32_t len);
```

写入数据。行为取决于当前状态：

| 状态 | 行为 | 返回 |
|------|------|:---:|
| Connected | 立即发送 | `Sent (0)` |
| Migrating | 排队等待 | `Queued (1)` |
| Migrating + 队列满 | 背压 | `Backpressure (2)` |
| Disconnected | 拒绝 | `Disconnected (-1)` |

| 参数 | 类型 | 说明 |
|------|------|------|
| `lsm` | `zenoh_lsm_t*` | — |
| `data` | `const uint8_t*` | 数据指针 |
| `len` | `uint32_t` | 数据长度（字节） |

| 返回值 | 含义 |
|:---:|------|
| 0 | `Sent` — 立即发送 |
| 1 | `Queued` — 已排队 |
| 2 | `Backpressure` — 队列已满 |
| -1 | `Disconnected` — 连接断开 |

```python
status = lsm.write(b"hello")                 # → "sent" / "queued" / "backpressure" / "disconnected"
```
```lua
lsm.write(ptr, "hello")                      -- → "sent"
```
```swift
let status = lsm.write(data: payload)         // → .sent
```
```kotlin
val status = lsm.write(payload)               // → WriteStatus.SENT
```

---

## 5. 读取检查

### `zenoh_lsm_can_read`

```c
int zenoh_lsm_can_read(zenoh_lsm_t* lsm);
```

检查是否可读。Connected 和 Migrating 态均可读，仅 Disconnected 返回错误。

| 返回值 | 含义 |
|:---:|------|
| 0 | 可读（Connected / Migrating） |
| -1 | 不可读（Disconnected） |

```python
if lsm.can_read():
    print("ready")
```

---

## 6. 超时轮询

### `zenoh_lsm_tick`

```c
int zenoh_lsm_tick(zenoh_lsm_t* lsm);
```

检查 Migrating 是否超时。调用方应在定时器中每 100ms 调用一次。

| 返回值 | 含义 |
|:---:|------|
| 0 | 未超时 |
| 3 | `MigrationTimeout` — 已进入 Disconnected，排队数据作废 |

| 常量 | 值 | 说明 |
|------|:---:|------|
| `MIGRATING_TIMEOUT_MS` | 4000 | 超时阈值（待用例 4/5 标定） |

```python
event = lsm.tick()                           # → "none" 或 "migration_timeout"
```
```swift
let event = lsm.tick()                       // → .none 或 .migrationTimeout
```

---

## 7. 排队数据排出

### `zenoh_lsm_drain`

```c
int32_t zenoh_lsm_drain(zenoh_lsm_t* lsm, uint8_t* buf, uint32_t buf_len);
```

将排队数据写入 `buf`。恢复 Connected 后调用以批量发送。

| 参数 | 类型 | 说明 |
|------|------|------|
| `lsm` | `zenoh_lsm_t*` | — |
| `buf` | `uint8_t*` | 输出缓冲区 |
| `buf_len` | `uint32_t` | 缓冲区大小 |

| 返回值 | 含义 |
|:---:|------|
| > 0 | 写入的字节数 |
| 0 | 队列为空但无错误 |
| -1 | 无排队数据 |

> **注意**：若 buf 不足，数据会被截断。建议 buf ≥ 65536。

```python
data = lsm.drain()                           # → b"msg1msg2..."
data = lsm.drain(buf_size=131072)            # 自定义 buffer 大小
```
```lua
local data = lsm.drain(ptr, 65536)           -- Lua
```

---

## 8. 状态查询

### `zenoh_lsm_queue_len`

```c
uint32_t zenoh_lsm_queue_len(zenoh_lsm_t* lsm);
```

当前排队消息数量。

```python
n = lsm.queue_length                         # Python property
```
```lua
local n = lsm.queue_length(ptr)              -- Lua
```

---

### `zenoh_lsm_is_connected`

```c
int zenoh_lsm_is_connected(zenoh_lsm_t* lsm);
```

返回 1 表示 Connected，0 表示其他状态。

---

### `zenoh_lsm_is_migrating`

```c
int zenoh_lsm_is_migrating(zenoh_lsm_t* lsm);
```

返回 1 表示 Migrating，0 表示其他状态。

---

## 9. 显式断开

### `zenoh_lsm_disconnect`

```c
void zenoh_lsm_disconnect(zenoh_lsm_t* lsm);
```

显式断开连接。清空排队数据，进入 Disconnected。

```python
lsm.disconnect()                             # Python
```

---

## 10. 状态转换图

```
             zenoh_lsm_new/with_backpressure
                         │
                         ▼
                   ┌───────────┐
              ┌─── │ Connected │
              │    └─────┬─────┘
              │          │ on_path_change(0) → event=1 (PathMigrated)
              │          ▼
              │    ┌───────────┐
              │    │ Migrating │ ← write() 可以排队
              │    └─────┬─────┘
              │          │ on_path_change(1) → event=2 (PathRestored)
              │          │ tick() 超时       → event=3 (MigrationTimeout)
              │          │
              │    ┌─────┴─────┐
              └───→│ Connected │    ┌──────────────┐
                   └───────────┘    │ Disconnected │ ← write() 返回 -1
                                    │              │    read() 返回 -1
                                    └──────────────┘
```

---

## 11. 事件代码速查表

| 代码 | 常量 | 触发条件 | Python | Lua |
|:---:|------|---------|--------|-----|
| 0 | `None` | 无状态变化 | `"none"` | `"none"` |
| 1 | `PathMigrated` | on_path_change(0) 从 Connected | `"path_migrated"` | `"path_migrated"` |
| 2 | `PathRestored` | on_path_change(1) 从 Migrating | `"path_restored"` | `"path_restored"` |
| 3 | `MigrationTimeout` | tick() 超时 | `"migration_timeout"` | `"migration_timeout"` |

## 12. 写入状态速查表

| 代码 | 常量 | 条件 | Python | Lua |
|:---:|------|------|--------|-----|
| 0 | `Sent` | Connected 态 | `"sent"` | `"sent"` |
| 1 | `Queued` | Migrating 态 | `"queued"` | `"queued"` |
| 2 | `Backpressure` | 队列满 (max_queue > 0) | `"backpressure"` | `"backpressure"` |
| -1 | `Disconnected` | 连接已断开 | `"disconnected"` | `"disconnected"` |

---

## 13. 典型使用流程

### 移动端网络切换

```python
lsm = LinkStateMachine(max_queue_depth=100)

# 网络正常
lsm.write(packet)  # → "sent"

# Wi-Fi → 4G 切换
lsm.on_path_change(False)  # → "path_migrated"

# 切换期间持续发送（消息排队）
for pkt in pending_packets:
    status = lsm.write(pkt)  # → "queued"

# 新网络就绪
lsm.on_path_change(True)  # → "path_restored"
data = lsm.drain()         # 排出所有排队数据
```

### 后台超时轮询

```python
import threading, time

def poll_ticker(lsm):
    while True:
        time.sleep(0.1)
        event = lsm.tick()
        if event == "migration_timeout":
            print("连接超时，触发重连")
            reconnect()
            break
```

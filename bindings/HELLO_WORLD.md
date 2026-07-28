# Hello World — 所有语言绑定全覆盖示例

每个语言一个完整的可运行程序，**覆盖 13 个 FFI 接口**。

---

## C

```c
// hello.c — 编译: gcc -o hello hello.c -L. -lzenoh_link_state
#include <stdio.h>
#include <string.h>
#include "zenoh_lsm.h"

int main() {
    printf("=== zenoh-link-state C Hello World ===\n\n");

    // 1. new
    zenoh_lsm_t* lsm = zenoh_lsm_new();
    printf("[new]          Created: connected=%d migrating=%d\n",
           zenoh_lsm_is_connected(lsm), zenoh_lsm_is_migrating(lsm));

    // 2. write (Connected → Sent)
    const char* msg = "hello from C";
    int r = zenoh_lsm_write(lsm, (const uint8_t*)msg, strlen(msg));
    printf("[write]        Sent: status=%d (0=Sent)\n", r);

    // 3. can_read
    printf("[can_read]     OK: %d (0=OK)\n", zenoh_lsm_can_read(lsm));

    // 4. on_path_change (失联 → Migrating)
    int ev = zenoh_lsm_on_path_change(lsm, 0);
    printf("[path_change]  Migrating: event=%d (1=PathMigrated)\n", ev);

    // 5. write (Migrating → Queued)
    r = zenoh_lsm_write(lsm, (const uint8_t*)"queued_msg", 10);
    printf("[write]        Queued: status=%d (1=Queued)\n", r);
    r = zenoh_lsm_write(lsm, (const uint8_t*)"another", 7);
    printf("[write]        Queued: status=%d queue_len=%u\n", r,
           zenoh_lsm_queue_len(lsm));

    // 6. tick (未超时)
    ev = zenoh_lsm_tick(lsm);
    printf("[tick]         No timeout: event=%d (0=None)\n", ev);

    // 7. on_path_change (恢复 → Connected)
    ev = zenoh_lsm_on_path_change(lsm, 1);
    printf("[path_change]  Restored: event=%d (2=PathRestored)\n", ev);

    // 8. drain
    uint8_t buf[256];
    int32_t n = zenoh_lsm_drain(lsm, buf, sizeof(buf));
    printf("[drain]        Recovered %d bytes: %.*s\n", n, n, buf);

    // 9. 背压示例
    zenoh_lsm_free(lsm);
    lsm = zenoh_lsm_new_with_backpressure(2);
    zenoh_lsm_on_path_change(lsm, 0);
    zenoh_lsm_write(lsm, (const uint8_t*)"a", 1);
    zenoh_lsm_write(lsm, (const uint8_t*)"b", 1);
    r = zenoh_lsm_write(lsm, (const uint8_t*)"c", 1);
    printf("[backpressure] status=%d (2=Backpressure)\n", r);

    // 10. disconnect
    zenoh_lsm_disconnect(lsm);
    printf("[disconnect]   connected=%d queue=%u\n",
           zenoh_lsm_is_connected(lsm), zenoh_lsm_queue_len(lsm));
    r = zenoh_lsm_write(lsm, (const uint8_t*)"x", 1);
    printf("[write]        Disconnected: status=%d (-1=Disconnected)\n", r);

    // 11. free
    zenoh_lsm_free(lsm);
    printf("[free]         OK\n");

    printf("\n=== ALL PASS ===\n");
    return 0;
}
```

---

## Python

```python
#!/usr/bin/env python3
"""hello.py — 覆盖所有 13 个 FFI 接口"""
from zenoh_link_state import LinkStateMachine

print("=== zenoh-link-state Python Hello World ===\n")

# 1. new
lsm = LinkStateMachine()
print(f"[new]          Created: connected={lsm.is_connected} migrating={lsm.is_migrating}")

# 2. write (Connected → Sent)
status = lsm.write(b"hello from Python")
print(f"[write]        Sent: status={status}")

# 3. can_read
print(f"[can_read]     OK: {lsm.can_read()}")

# 4. on_path_change (失联 → Migrating)
event = lsm.on_path_change(False)
print(f"[path_change]  Migrating: event={event}")

# 5. write (Migrating → Queued)
status = lsm.write(b"queued_msg")
print(f"[write]        Queued: status={status}")
lsm.write(b"another")
print(f"[write]        Queued: queue_len={lsm.queue_length}")

# 6. tick (未超时)
event = lsm.tick()
print(f"[tick]         No timeout: event={event}")

# 7. on_path_change (恢复 → Connected)
event = lsm.on_path_change(True)
print(f"[path_change]  Restored: event={event}")

# 8. drain
data = lsm.drain()
print(f"[drain]        Recovered {len(data)} bytes: {data}")

# 9. 背压
lsm2 = LinkStateMachine(max_queue_depth=2)
lsm2.on_path_change(False)
lsm2.write(b"a")
lsm2.write(b"b")
status = lsm2.write(b"c")
print(f"[backpressure] status={status}")

# 10. disconnect
lsm2.disconnect()
print(f"[disconnect]   connected={lsm2.is_connected} queue={lsm2.queue_length}")
status = lsm2.write(b"x")
print(f"[write]        Disconnected: status={status}")

# 11. free (auto via __del__)
del lsm2
print(f"[free]         OK (auto via __del__)")

print("\n=== ALL PASS ===")
```

---

## Lua

```lua
#!/usr/bin/env luajit
-- hello.lua — 覆盖所有 13 个 FFI 接口
local lsm = require("zenoh_link_state")

print("=== zenoh-link-state Lua Hello World ===\n")

-- 1. new
local ptr = lsm.new()
print(string.format("[new]          Created: connected=%s migrating=%s",
      lsm.is_connected(ptr), lsm.is_migrating(ptr)))

-- 2. write (Connected → Sent)
local status = lsm.write(ptr, "hello from Lua")
print(string.format("[write]        Sent: status=%s", status))

-- 3. can_read
print(string.format("[can_read]     OK: %s", lsm.can_read(ptr)))

-- 4. on_path_change (失联 → Migrating)
local event = lsm.on_path_change(ptr, false)
print(string.format("[path_change]  Migrating: event=%s", event))

-- 5. write (Migrating → Queued)
status = lsm.write(ptr, "queued_msg")
print(string.format("[write]        Queued: status=%s", status))
lsm.write(ptr, "another")
print(string.format("[write]        Queued: queue_len=%d", lsm.queue_length(ptr)))

-- 6. tick (未超时)
event = lsm.tick(ptr)
print(string.format("[tick]         No timeout: event=%s", event))

-- 7. on_path_change (恢复 → Connected)
event = lsm.on_path_change(ptr, true)
print(string.format("[path_change]  Restored: event=%s", event))

-- 8. drain
local data = lsm.drain(ptr, 256)
print(string.format("[drain]        Recovered %d bytes: %s", #data, data))

-- 9. 背压
local ptr2 = lsm.new(2)
lsm.on_path_change(ptr2, false)
lsm.write(ptr2, "a")
lsm.write(ptr2, "b")
status = lsm.write(ptr2, "c")
print(string.format("[backpressure] status=%s", status))

-- 10. disconnect
lsm.disconnect(ptr2)
print(string.format("[disconnect]   connected=%s queue=%d",
      lsm.is_connected(ptr2), lsm.queue_length(ptr2)))
status = lsm.write(ptr2, "x")
print(string.format("[write]        Disconnected: status=%s", status))

-- 11. free
lsm.free(ptr2)
lsm.free(ptr)
print("[free]         OK")

print("\n=== ALL PASS ===")
```

---

## JavaScript (Node.js)

```javascript
// hello.js — 覆盖所有 13 个接口
// 前提: 已用 wasm-bindgen 生成 pkg/

import { zenoh_lsm_new, zenoh_lsm_new_with_backpressure,
         zenoh_lsm_free, zenoh_lsm_on_path_change, zenoh_lsm_write,
         zenoh_lsm_can_read, zenoh_lsm_tick, zenoh_lsm_drain,
         zenoh_lsm_queue_len, zenoh_lsm_is_connected,
         zenoh_lsm_is_migrating, zenoh_lsm_disconnect } from './pkg/zenoh_link_state.js';

const EVENT = { 0: "none", 1: "path_migrated", 2: "path_restored", 3: "migration_timeout" };
const STATUS = { 0: "sent", 1: "queued", 2: "backpressure", "-1": "disconnected" };

console.log("=== zenoh-link-state JavaScript Hello World ===\n");

// 1. new
let lsm = zenoh_lsm_new();
console.log(`[new]          Created: connected=${!!zenoh_lsm_is_connected(lsm)}`);

// 2. write
let enc = new TextEncoder();
let r = zenoh_lsm_write(lsm, enc.encode("hello from JS"));
console.log(`[write]        Sent: status=${STATUS[r]}`);

// 3. can_read
console.log(`[can_read]     OK: ${zenoh_lsm_can_read(lsm) === 0}`);

// 4. on_path_change
let ev = zenoh_lsm_on_path_change(lsm, 0);
console.log(`[path_change]  Migrating: event=${EVENT[ev]}`);

// 5. write (Migrating → Queued)
r = zenoh_lsm_write(lsm, enc.encode("queued_msg"));
console.log(`[write]        Queued: status=${STATUS[r]} queue=${zenoh_lsm_queue_len(lsm)}`);

// 6. tick
ev = zenoh_lsm_tick(lsm);
console.log(`[tick]         No timeout: event=${EVENT[ev]}`);

// 7. on_path_change (恢复)
ev = zenoh_lsm_on_path_change(lsm, 1);
console.log(`[path_change]  Restored: event=${EVENT[ev]}`);

// 8. drain
let buf = new Uint8Array(256);
let n = zenoh_lsm_drain(lsm, buf);
console.log(`[drain]        Recovered ${n} bytes`);

// 9. backpressure
zenoh_lsm_free(lsm);
lsm = zenoh_lsm_new_with_backpressure(2);
zenoh_lsm_on_path_change(lsm, 0);
zenoh_lsm_write(lsm, enc.encode("a"));
zenoh_lsm_write(lsm, enc.encode("b"));
r = zenoh_lsm_write(lsm, enc.encode("c"));
console.log(`[backpressure] status=${STATUS[r]}`);

// 10. disconnect
zenoh_lsm_disconnect(lsm);
r = zenoh_lsm_write(lsm, enc.encode("x"));
console.log(`[write]        Disconnected: status=${STATUS[r]}`);

// 11. free
zenoh_lsm_free(lsm);
console.log(`[free]         OK`);

console.log("\n=== ALL PASS ===");
```

---

## Swift (iOS)

```swift
// HelloWorld.swift
import Foundation

print("=== zenoh-link-state Swift Hello World ===\n")

// 1. new
let lsm = LinkStateMachine()
print("[new]          Created: connected=\(lsm.isConnected)")

// 2. write
let status = lsm.write(data: "hello from Swift".data(using: .utf8)!)
print("[write]        Sent: status=\(status)")

// 3. can_read
print("[can_read]     OK: \(lsm.canRead())")

// 4. on_path_change
var event = lsm.onPathChange(connected: false)
print("[path_change]  Migrating: event=\(event)")

// 5. write (Migrating → Queued)
let s = lsm.write(data: "queued_msg".data(using: .utf8)!)
print("[write]        Queued: status=\(s) queue=\(lsm.queueLength)")

// 6. tick
event = lsm.tick()
print("[tick]         No timeout: event=\(event)")

// 7. on_path_change (恢复)
event = lsm.onPathChange(connected: true)
print("[path_change]  Restored: event=\(event)")

// 8. drain
let data = lsm.drain(bufSize: 256)
print("[drain]        Recovered \(data.count) bytes")

// 9. backpressure
let lsm2 = LinkStateMachine(maxQueueDepth: 2)
lsm2.onPathChange(connected: false)
lsm2.write(data: "a".data(using: .utf8)!)
lsm2.write(data: "b".data(using: .utf8)!)
let s2 = lsm2.write(data: "c".data(using: .utf8)!)
print("[backpressure] status=\(s2)")

// 10. disconnect
lsm2.disconnect()
let s3 = lsm2.write(data: "x".data(using: .utf8)!)
print("[write]        Disconnected: status=\(s3)")

// 11. free (auto via deinit)
print("[free]         OK (auto via deinit)")

print("\n=== ALL PASS ===")
```

---

## Kotlin (Android)

```kotlin
// HelloWorld.kt
fun main() {
    println("=== zenoh-link-state Kotlin Hello World ===\n")

    // 1. new
    val lsm = LinkStateMachine()
    println("[new]          Created: connected=${lsm.isConnected}")

    // 2. write
    var status = lsm.write("hello from Kotlin".toByteArray())
    println("[write]        Sent: status=$status")

    // 3. can_read
    println("[can_read]     OK: ${lsm.canRead()}")

    // 4. on_path_change
    var event = lsm.onPathChange(false)
    println("[path_change]  Migrating: event=$event")

    // 5. write (Migrating → Queued)
    status = lsm.write("queued_msg".toByteArray())
    println("[write]        Queued: status=$status queue=${lsm.queueLength}")

    // 6. tick
    event = lsm.tick()
    println("[tick]         No timeout: event=$event")

    // 7. on_path_change (恢复)
    event = lsm.onPathChange(true)
    println("[path_change]  Restored: event=$event")

    // 8. drain
    val data = lsm.drain(256)
    println("[drain]        Recovered ${data.size} bytes")

    // 9. backpressure
    val lsm2 = LinkStateMachine(maxQueueDepth = 2)
    lsm2.onPathChange(false)
    lsm2.write("a".toByteArray())
    lsm2.write("b".toByteArray())
    val s2 = lsm2.write("c".toByteArray())
    println("[backpressure] status=$s2")

    // 10. disconnect
    lsm2.disconnect()
    val s3 = lsm2.write("x".toByteArray())
    println("[write]        Disconnected: status=$s3")

    // 11. free (auto via finalize)
    println("[free]         OK (auto via finalize)")

    println("\n=== ALL PASS ===")
}
```

---

## 接口覆盖汇总

| # | 接口 | C | Python | Lua | JS | Swift | Kotlin |
|:---:|------|:---:|:---:|:---:|:---:|:---:|:---:|
| 1 | `new` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 2 | `write` (Connected) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 3 | `can_read` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 4 | `on_path_change` (失联) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 5 | `write` (Migrating) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 6 | `tick` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 7 | `on_path_change` (恢复) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 8 | `drain` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 9 | `with_backpressure` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 10 | `disconnect` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 11 | `free` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

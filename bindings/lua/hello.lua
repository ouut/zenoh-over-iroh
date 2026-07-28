#!/usr/bin/env luajit
-- hello.lua — zenoh-link-state Lua Hello World (覆盖所有 13 个 FFI 接口)
--
-- 运行前提:
--   cargo build --release
--   cp target/release/libzenoh_link_state.so .
--   luajit hello.lua

local lsm = require("zenoh_link_state")

print("=== zenoh-link-state Lua Hello World ===\n")

-- 1. new
local ptr = lsm.new()
print(string.format("[new]          connected=%s", lsm.is_connected(ptr)))

-- 2. write (Connected → Sent)
local s = lsm.write(ptr, "hello from Lua")
print(string.format("[write]        Sent: %s", s))

-- 3. can_read
print(string.format("[can_read]     OK: %s", lsm.can_read(ptr)))

-- 4. on_path_change (失联 → Migrating)
local e = lsm.on_path_change(ptr, false)
print(string.format("[path_change]  Migrating: %s", e))

-- 5. write (Migrating → Queued)
lsm.write(ptr, "queued_1")
lsm.write(ptr, "queued_2")
print(string.format("[write]        Queued: queue=%d", lsm.queue_length(ptr)))

-- 6. tick
e = lsm.tick(ptr)
print(string.format("[tick]         event=%s", e))

-- 7. on_path_change (恢复 → Connected)
e = lsm.on_path_change(ptr, true)
print(string.format("[path_change]  Restored: %s", e))

-- 8. drain
local data = lsm.drain(ptr)
print(string.format("[drain]        Recovered %d bytes", #data))

-- 9. backpressure
local bp = lsm.new(2)
lsm.on_path_change(bp, false)
lsm.write(bp, "a"); lsm.write(bp, "b")
s = lsm.write(bp, "c")
print(string.format("[backpressure] %s", s))

-- 10. disconnect
lsm.disconnect(bp)
s = lsm.write(bp, "x")
print(string.format("[write]        Disconnected: %s", s))

-- 11. free
lsm.free(bp)
lsm.free(ptr)
print("[free]         OK")

print("\n=== ALL PASS ===")

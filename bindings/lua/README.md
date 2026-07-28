# Lua Binding

通过 LuaJIT FFI 直接调用 C ABI，零编译开销。

## 使用

```lua
local lsm = require("zenoh_link_state")

local machine = lsm.new()

-- 路径变化
local event = machine:on_path_change(false)  -- → "path_migrated"

-- 写入数据
local status = machine:write("hello")          -- → "queued"

-- 恢复
machine:on_path_change(true)                   -- → "path_restored"

-- 查询
print(machine:is_connected())   -- true
print(machine:queue_length())   -- 0

-- 超时轮询
local event = machine:tick()     -- → "none" 或 "migration_timeout"

-- 释放
machine:free()
```

## 安装

```bash
# 1. 编译共享库
cargo build --release

# 2. 复制到 Lua 项目
cp target/release/libzenoh_link_state.so my_lua_project/

# 3. 复制 Lua wrapper
cp bindings/lua/zenoh_link_state.lua my_lua_project/
```

-- Lua binding for zenoh-link-state
-- Requires: libzenoh_link_state.so in the same directory or LD_LIBRARY_PATH
-- Usage: local lsm = require("zenoh_link_state")

local ffi = require("ffi")

ffi.cdef[[
    typedef void zenoh_lsm_t;
    zenoh_lsm_t* zenoh_lsm_new(void);
    zenoh_lsm_t* zenoh_lsm_new_with_backpressure(uint32_t max_queue);
    void zenoh_lsm_free(zenoh_lsm_t* lsm);
    int zenoh_lsm_on_path_change(zenoh_lsm_t* lsm, int connected);
    int zenoh_lsm_write(zenoh_lsm_t* lsm, const uint8_t* data, uint32_t len);
    int zenoh_lsm_can_read(zenoh_lsm_t* lsm);
    int zenoh_lsm_tick(zenoh_lsm_t* lsm);
    int zenoh_lsm_drain(zenoh_lsm_t* lsm, uint8_t* buf, uint32_t buf_len);
    uint32_t zenoh_lsm_queue_len(zenoh_lsm_t* lsm);
    int zenoh_lsm_is_connected(zenoh_lsm_t* lsm);
    int zenoh_lsm_is_migrating(zenoh_lsm_t* lsm);
    void zenoh_lsm_disconnect(zenoh_lsm_t* lsm);
]]

-- Try to load the library
local lib_path = os.getenv("ZENOH_LSM_LIB") or "libzenoh_link_state"
local lib = ffi.load(lib_path)

local EVENT = { [0]="none", [1]="path_migrated", [2]="path_restored", [3]="migration_timeout" }
local STATUS = { [0]="sent", [1]="queued", [2]="backpressure", [-1]="disconnected" }

local M = {}

function M.new(max_queue_depth)
    max_queue_depth = max_queue_depth or 0
    if max_queue_depth > 0 then
        return lib.zenoh_lsm_new_with_backpressure(max_queue_depth)
    else
        return lib.zenoh_lsm_new()
    end
end

function M.free(ptr)
    if ptr then lib.zenoh_lsm_free(ptr) end
end

function M.on_path_change(ptr, connected)
    return EVENT[lib.zenoh_lsm_on_path_change(ptr, connected and 1 or 0)]
end

function M.write(ptr, data)
    local buf = ffi.new("uint8_t[?]", #data)
    ffi.copy(buf, data, #data)
    return STATUS[lib.zenoh_lsm_write(ptr, buf, #data)]
end

function M.can_read(ptr)
    return lib.zenoh_lsm_can_read(ptr) == 0
end

function M.tick(ptr)
    return EVENT[lib.zenoh_lsm_tick(ptr)]
end

function M.drain(ptr, buf_size)
    buf_size = buf_size or 65536
    local buf = ffi.new("uint8_t[?]", buf_size)
    local n = lib.zenoh_lsm_drain(ptr, buf, buf_size)
    if n <= 0 then return "" end
    return ffi.string(buf, n)
end

function M.queue_length(ptr)
    return tonumber(lib.zenoh_lsm_queue_len(ptr))
end

function M.is_connected(ptr)
    return lib.zenoh_lsm_is_connected(ptr) ~= 0
end

function M.is_migrating(ptr)
    return lib.zenoh_lsm_is_migrating(ptr) ~= 0
end

function M.disconnect(ptr)
    lib.zenoh_lsm_disconnect(ptr)
end

return M

"""Python binding for zenoh-mobile (ctypes + iroh transport).

编译:
    cd mobile && cargo build --release
    # 产出 target/release/libzenoh_mobile.so

使用:
    from mobile.python.zenoh import open_session, put, subscribe
    s = open_session('{"listen":{"endpoints":["tcp/127.0.0.1:0"]}}')
    put(s, "hello", "world")
    close(s)
"""

import ctypes, os, json, threading, queue
from ctypes import c_char_p, c_int, c_void_p, c_uint8, c_size_t, CFUNCTYPE, POINTER

_lib = None

def _load():
    global _lib
    if _lib:
        return _lib
    # 查找 libzenoh_mobile.so
    paths = [
        os.path.join(os.path.dirname(__file__), "../../target/release/libzenoh_mobile.so"),
        os.path.join(os.path.dirname(__file__), "../../target/debug/libzenoh_mobile.so"),
        "libzenoh_mobile.so",
    ]
    for p in paths:
        p = os.path.abspath(p)
        if os.path.exists(p):
            _lib = ctypes.CDLL(p)
            break
    if not _lib:
        raise RuntimeError(f"libzenoh_mobile.so not found. Tried: {paths}")
    return _lib

def _setup():
    lib = _load()
    lib.z_open.restype = c_int
    lib.z_open.argtypes = [POINTER(c_void_p), c_void_p]
    lib.z_close.argtypes = [c_void_p]
    lib.z_put.restype = c_int
    lib.z_put.argtypes = [c_void_p, c_char_p, c_char_p]
    lib.z_config_from_str.restype = c_void_p
    lib.z_config_from_str.argtypes = [c_char_p]
    lib.z_config_free.argtypes = [c_void_p]
    lib.z_subscribe.restype = c_void_p
    lib.z_subscribe.argtypes = [c_void_p, c_char_p, CFUNCTYPE(None, c_char_p, c_char_p, c_void_p), c_void_p]
    return lib

# ── 高层 API ───────────────────────────────────

def open_session(config_json: str) -> c_void_p:
    """打开 Zenoh 会话（支持 iroh 传输）"""
    lib = _setup()
    cfg = lib.z_config_from_str(config_json.encode())
    if not cfg:
        raise RuntimeError("Invalid config")
    session = c_void_p()
    ret = lib.z_open(ctypes.byref(session), cfg)
    lib.z_config_free(cfg)
    if ret != 0:
        raise RuntimeError("Failed to open session")
    return session.value

def close(session: c_void_p):
    """关闭会话"""
    _setup().z_close(session)

def put(session: c_void_p, key: str, value: str) -> bool:
    """发布消息"""
    lib = _setup()
    return lib.z_put(session, key.encode(), value.encode()) == 0

def delete(session: c_void_p, key: str) -> bool:
    """删除键"""
    lib = _setup()
    return lib.z_delete(session, key.encode()) == 0

class Subscription:
    """订阅包装类"""
    def __init__(self, session, key, callback, ctx=None):
        self._queue = queue.Queue()
        self._running = True

        CB = CFUNCTYPE(None, c_char_p, c_char_p, c_void_p)
        def _cb(k, v, ctx):
            self._queue.put((k.decode(), v.decode()))

        lib = _setup()
        self._inner = lib.z_subscribe(session, key.encode(), CB(_cb), None)

    def recv(self, timeout=None):
        """接收一条消息"""
        try:
            return self._queue.get(timeout=timeout)
        except queue.Empty:
            return None

    def __iter__(self):
        return self

    def __next__(self):
        return self.recv()

def subscribe(session, key, callback=None):
    """订阅主题。callback 为可选参数，不传则返回 Subscription 对象"""
    if callback:
        CB = CFUNCTYPE(None, c_char_p, c_char_p, c_void_p)
        lib = _setup()
        lib.z_subscribe(session, key.encode(), CB(callback), None)
        return None
    return Subscription(session, key)

# ── 快速测试 ───────────────────────────────────

if __name__ == "__main__":
    import time
    print("=== zenoh-mobile Python Hello ===")
    cfg = '{"mode":"peer","listen":{"endpoints":["tcp/127.0.0.1:0"]},"scouting":{"multicast":{"enabled":false}}}'
    s = open_session(cfg)
    print(f"✅ open: {s}")

    def on_msg(key, value, ctx):
        print(f"📩 {key}: {value}")

    sub = subscribe(s, "demo/test", on_msg)
    time.sleep(0.5)
    put(s, "demo/test", "hello from Python")
    time.sleep(1)
    close(s)
    print("✅ close")

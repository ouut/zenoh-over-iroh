"""
Python ctypes binding for zenoh-link-state.

Usage:
    from zenoh_link_state import LinkStateMachine
    lsm = LinkStateMachine()
    lsm.on_path_change(False)
    lsm.write(b"hello")
"""

import ctypes
import os
import platform
from ctypes import c_void_p, c_int, c_uint32, c_uint8, POINTER, byref

_LIB = None

def _find_lib():
    system = platform.system()
    machine = platform.machine()
    lib_dir = os.path.dirname(os.path.abspath(__file__))

    candidates = []
    if system == "Linux":
        candidates = [
            os.path.join(lib_dir, "libzenoh_link_state.so"),
            "libzenoh_link_state.so",
        ]
    elif system == "Darwin":
        candidates = [
            os.path.join(lib_dir, "libzenoh_link_state.dylib"),
            "libzenoh_link_state.dylib",
        ]
    elif system == "Windows":
        candidates = [
            os.path.join(lib_dir, "zenoh_link_state.dll"),
            "zenoh_link_state.dll",
        ]

    for c in candidates:
        try:
            return ctypes.CDLL(c)
        except OSError:
            continue
    raise RuntimeError(f"Cannot find libzenoh_link_state. Tried: {candidates}")

def _get_lib():
    global _LIB
    if _LIB is None:
        _LIB = _find_lib()
        # Configure function signatures
        _LIB.zenoh_lsm_new.restype = c_void_p
        _LIB.zenoh_lsm_new_with_backpressure.argtypes = [c_uint32]
        _LIB.zenoh_lsm_new_with_backpressure.restype = c_void_p
        _LIB.zenoh_lsm_free.argtypes = [c_void_p]
        _LIB.zenoh_lsm_on_path_change.argtypes = [c_void_p, c_int]
        _LIB.zenoh_lsm_on_path_change.restype = c_int
        _LIB.zenoh_lsm_write.argtypes = [c_void_p, POINTER(c_uint8), c_uint32]
        _LIB.zenoh_lsm_write.restype = c_int
        _LIB.zenoh_lsm_can_read.argtypes = [c_void_p]
        _LIB.zenoh_lsm_can_read.restype = c_int
        _LIB.zenoh_lsm_tick.argtypes = [c_void_p]
        _LIB.zenoh_lsm_tick.restype = c_int
        _LIB.zenoh_lsm_drain.argtypes = [c_void_p, POINTER(c_uint8), c_uint32]
        _LIB.zenoh_lsm_drain.restype = c_int
        _LIB.zenoh_lsm_queue_len.argtypes = [c_void_p]
        _LIB.zenoh_lsm_queue_len.restype = c_uint32
        _LIB.zenoh_lsm_is_connected.argtypes = [c_void_p]
        _LIB.zenoh_lsm_is_connected.restype = c_int
        _LIB.zenoh_lsm_is_migrating.argtypes = [c_void_p]
        _LIB.zenoh_lsm_is_migrating.restype = c_int
        _LIB.zenoh_lsm_disconnect.argtypes = [c_void_p]
    return _LIB

_EVENT_MAP = {0: "none", 1: "path_migrated", 2: "path_restored", 3: "migration_timeout"}
_STATUS_MAP = {0: "sent", 1: "queued", 2: "backpressure", -1: "disconnected"}

class LinkStateMachine:
    def __init__(self, max_queue_depth=0):
        lib = _get_lib()
        if max_queue_depth > 0:
            self._ptr = lib.zenoh_lsm_new_with_backpressure(max_queue_depth)
        else:
            self._ptr = lib.zenoh_lsm_new()

    def __del__(self):
        if self._ptr:
            _get_lib().zenoh_lsm_free(self._ptr)

    def on_path_change(self, connected):
        code = _get_lib().zenoh_lsm_on_path_change(self._ptr, 1 if connected else 0)
        return _EVENT_MAP.get(code, "unknown")

    def write(self, data):
        buf = (c_uint8 * len(data))(*data)
        code = _get_lib().zenoh_lsm_write(self._ptr, buf, len(data))
        return _STATUS_MAP.get(code, "unknown")

    def can_read(self):
        return _get_lib().zenoh_lsm_can_read(self._ptr) == 0

    def tick(self):
        code = _get_lib().zenoh_lsm_tick(self._ptr)
        return _EVENT_MAP.get(code, "none")

    def drain(self, buf_size=65536):
        buf = (c_uint8 * buf_size)()
        n = _get_lib().zenoh_lsm_drain(self._ptr, buf, buf_size)
        return bytes(buf[:n]) if n > 0 else b""

    @property
    def queue_length(self):
        return _get_lib().zenoh_lsm_queue_len(self._ptr)

    @property
    def is_connected(self):
        return _get_lib().zenoh_lsm_is_connected(self._ptr) != 0

    @property
    def is_migrating(self):
        return _get_lib().zenoh_lsm_is_migrating(self._ptr) != 0

    def disconnect(self):
        _get_lib().zenoh_lsm_disconnect(self._ptr)

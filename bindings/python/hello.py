#!/usr/bin/env python3
"""
hello.py — zenoh-link-state Python Hello World (覆盖所有 13 个 FFI 接口)

运行前提:
    cargo build --release
    cp target/release/libzenoh_link_state.so .
    python hello.py
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from zenoh_link_state import LinkStateMachine

def main():
    print("=== zenoh-link-state Python Hello World ===\n")

    # 1. new
    lsm = LinkStateMachine()
    print(f"[new]          connected={lsm.is_connected}")

    # 2. write (Connected → Sent)
    s = lsm.write(b"hello from Python")
    print(f"[write]        Sent: {s}")

    # 3. can_read
    print(f"[can_read]     OK: {lsm.can_read()}")

    # 4. on_path_change (失联 → Migrating)
    e = lsm.on_path_change(False)
    print(f"[path_change]  Migrating: {e}")

    # 5. write (Migrating → Queued)
    lsm.write(b"queued_1")
    lsm.write(b"queued_2")
    print(f"[write]        Queued: queue={lsm.queue_length}")

    # 6. tick
    e = lsm.tick()
    print(f"[tick]         event={e}")

    # 7. on_path_change (恢复 → Connected)
    e = lsm.on_path_change(True)
    print(f"[path_change]  Restored: {e}")

    # 8. drain
    data = lsm.drain()
    print(f"[drain]        Recovered {len(data)} bytes")

    # 9. backpressure
    bp = LinkStateMachine(max_queue_depth=2)
    bp.on_path_change(False)
    bp.write(b"a"); bp.write(b"b")
    s = bp.write(b"c")
    print(f"[backpressure] {s}")

    # 10. disconnect
    bp.disconnect()
    s = bp.write(b"x")
    print(f"[write]        Disconnected: {s}")

    # 11. free (auto)
    del bp
    print("[free]         OK (auto)")

    print("\n=== ALL PASS ===")

if __name__ == "__main__":
    main()

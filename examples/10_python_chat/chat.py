#!/usr/bin/env python3
"""
Python 聊天室 — 基于 iroh P2P (真实 QUIC 连接) + LinkStateMachine。

Replicates example 09 in Python: spawns the Rust chat binary for iroh networking,
wraps it with a Python UI + LinkStateMachine for connection state management.
No TCP — all communication via real iroh QUIC P2P.

运行:
    cargo build --release  (in examples/09_chat_room/)
    python chat.py Alice
    python chat.py Bob
"""

import sys, os, re, threading, queue, time
from subprocess import Popen, PIPE

# ── 定位 Rust chat 二进制 ──────────────────────────

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CHAT_BIN = os.path.join(SCRIPT_DIR, "../09_chat_room/target/release/chat")
if not os.path.exists(CHAT_BIN):
    CHAT_BIN = os.path.join(SCRIPT_DIR, "../09_chat_room/target/debug/chat")
if not os.path.exists(CHAT_BIN):
    print("请先编译 Rust chat 二进制:")
    print("  cd examples/09_chat_room && cargo build --release")
    sys.exit(1)

# ── 状态机 (Python binding) ────────────────────────

sys.path.insert(0, os.path.join(SCRIPT_DIR, "../../bindings/python"))
from zenoh_link_state import LinkStateMachine

# ── 主程序 ─────────────────────────────────────────

def main():
    user_name = sys.argv[1] if len(sys.argv) > 1 else "Anonymous"

    lsm = LinkStateMachine(max_queue_depth=50)
    recv_queue = queue.Queue()
    node_id = None
    connected = False

    # ── 启动 Rust iroh 进程 ────────────────────
    proc = Popen([CHAT_BIN, user_name], stdin=PIPE, stdout=PIPE, stderr=PIPE, text=True,
                 bufsize=1)

    # ── 读取线程: 从 Rust 进程 stdout+stderr 提取 NodeID 和消息 ──
    def reader():
        nonlocal node_id
        for line in proc.stdout:
            line = line.strip()
            if not line:
                continue
            # 匹配 NodeID
            m = re.search(r'NodeID:\s*([a-f0-9]{64})', line)
            if m and not node_id:
                node_id = m.group(1)
                recv_queue.put(("nodeid", node_id))
                continue
            # 匹配聊天消息: [ts] 👤 name: text
            m = re.match(r'\[(\d+)\]\s*(.+):\s*(.+)', line)
            if m:
                recv_queue.put(("msg", {"ts": m.group(1), "sender": m.group(2), "text": m.group(3)}))
            else:
                recv_queue.put(("raw", line))

    def stderr_reader():
        for line in proc.stderr:
            line = line.strip()
            if line:
                recv_queue.put(("log", line))

    threading.Thread(target=reader, daemon=True).start()
    threading.Thread(target=stderr_reader, daemon=True).start()

    # ── 等待 NodeID ────────────────────────────
    print()
    print("╔══════════════════════════════════════════════════╗")
    print("║   Python × Iroh  P2P 控制台聊天室 (iroh QUIC)    ║")
    print("╠══════════════════════════════════════════════════╣")
    print(f"║ 用户:   {user_name:<38} ║")

    timeout = 10
    while timeout > 0 and not node_id:
        try:
            kind, val = recv_queue.get(timeout=1)
            if kind == "nodeid":
                node_id = val
        except queue.Empty:
            timeout -= 1

    if not node_id:
        print("║  ❌ 无法获取 NodeID                                   ║")
        print("╚══════════════════════════════════════════════════╝")
        proc.terminate()
        return

    print(f"║ NodeID: {node_id} ║")
    print("╚══════════════════════════════════════════════════╝")
    print(f"  输入 /connect <对方NodeID> 连接后开始聊天")
    print(f"  命令: /connect /help /quit /status /demo")
    print()

    # ── 主循环 ─────────────────────────────────
    try:
        while True:
            # 检查接收队列
            while True:
                try:
                    kind, val = recv_queue.get_nowait()
                    if kind == "msg":
                        tag = "👤 我" if val["sender"] == "👤 我" else val["sender"]
                        print(f"\r[{val['ts']}] {tag}: {val['text']}")
                    elif kind == "log" and ("ERROR" in val or "WARN" in val):
                        print(f"\r  ⚠️ {val[:80]}")
                except queue.Empty:
                    break

            # 输入
            print("> ", end="", flush=True)
            line = sys.stdin.readline()
            if not line:
                break
            cmd = line.strip()
            if not cmd:
                continue

            if cmd == "/quit":
                break
            if cmd == "/help":
                print("  /connect <NodeID>  /quit  /status  /demo")
                continue
            if cmd == "/status":
                s = "Connected" if lsm.is_connected else ("Migrating" if lsm.is_migrating else "Disconnected")
                print(f"  状态: {s} | 排队: {lsm.queue_length}")
                continue
            if cmd == "/demo":
                print("\n  🎬 状态机演示")
                lsm.on_path_change(False)
                time.sleep(0.5)
                print("  [1/3] Migrating — 消息排队")
                lsm.write(b"demo")
                lsm.on_path_change(True)
                lsm.drain()
                print("  [2/3] Connected — 恢复 ✓")
                print(f"  [3/3] 状态: {'Connected' if lsm.is_connected else 'Migrating'}")
                print()
                continue

            # 发送到 Rust 进程 (iroh P2P)
            try:
                proc.stdin.write(cmd + "\n")
                proc.stdin.flush()
            except BrokenPipeError:
                print("  ❌ Rust 进程已退出")
                break

    except KeyboardInterrupt:
        pass
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except:
            proc.kill()
        print("\n👋 再见！")

if __name__ == "__main__":
    main()

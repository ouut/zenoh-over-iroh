#!/usr/bin/env python3
"""
Python 聊天室 — 基于 LinkStateMachine + TCP 实现双向通信。

运行:
    # 终端1 (server 模式)
    python chat.py Alice --port 9000

    # 终端2 (client 模式)
    python chat.py Bob --connect localhost:9000 --port 9001
"""

import sys, os, json, socket, threading, time
from dataclasses import dataclass

# 将 bindings/python 加入路径以加载 LinkStateMachine
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../bindings/python"))
from zenoh_link_state import LinkStateMachine

# ── Wire message ─────────────────────────────────────

@dataclass
class WireMsg:
    sender:   str
    text:     str
    ts:       int
    msg_type: str  # "msg" | "join" | "leave"

def encode_msg(msg):
    return (json.dumps(msg.__dict__) + "\n").encode()

def decode_msg(line):
    d = json.loads(line)
    return WireMsg(**d)

# ── TCP Server (接收消息) ────────────────────────────

def run_server(port, on_recv):
    """在后台线程运行 TCP server，收到消息时回调 on_recv(msg)"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind(("0.0.0.0", port))
    sock.listen(1)
    sock.settimeout(1.0)

    print(f"[server] 监听端口 {port}...")

    while True:
        try:
            conn, addr = sock.accept()
            print(f"[server] 连接来自 {addr}")
            buf = b""
            while True:
                data = conn.recv(4096)
                if not data:
                    break
                buf += data
                while b"\n" in buf:
                    line, buf = buf.split(b"\n", 1)
                    try:
                        msg = decode_msg(line.decode())
                        on_recv(msg)
                    except:
                        pass
            conn.close()
            print(f"[server] 连接断开")
        except socket.timeout:
            continue
        except OSError:
            break

# ── TCP Client (发送消息) ────────────────────────────

class PeerConnection:
    def __init__(self):
        self.sock = None

    def connect(self, host, port):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.connect((host, port))
        print(f"[client] 已连接到 {host}:{port}")

    def send(self, msg):
        """发送 WireMsg，失败则返回 False"""
        if not self.sock:
            return False
        try:
            self.sock.sendall(encode_msg(msg))
            return True
        except OSError:
            return False

    def close(self):
        if self.sock:
            self.sock.close()
            self.sock = None

# ── 主程序 ──────────────────────────────────────────

def main():
    import argparse
    p = argparse.ArgumentParser(description="Python P2P Chat with LinkStateMachine")
    p.add_argument("name", help="用户名")
    p.add_argument("--port", type=int, default=9000, help="本地监听端口")
    p.add_argument("--connect", help="远端地址 host:port")
    args = p.parse_args()

    # ── 状态机 ──────────────────────────────────
    lsm = LinkStateMachine(max_queue_depth=100)

    print()
    print("╔══════════════════════════════════════════╗")
    print("║   Python P2P 聊天室 + LinkStateMachine   ║")
    print("╠══════════════════════════════════════════╣")
    print(f"║ 用户: {args.name:<32} ║")
    print(f"║ 端口: {args.port:<32} ║")
    print("╚══════════════════════════════════════════╝")
    print("  命令: /connect host:port  /help  /quit  /status")
    print()

    peer = PeerConnection()
    lock = threading.Lock()

    # ── 消息接收回调 ───────────────────────────
    def on_recv(msg):
        tag = "👤 我" if msg.sender == args.name else f"👤 {msg.sender}"
        t = msg.ts % 100000
        print(f"\r[{t}] {tag}: {msg.text}")
        print("> ", end="", flush=True)

    # ── 启动服务器 ─────────────────────────────
    srv = threading.Thread(target=run_server, args=(args.port, on_recv), daemon=True)
    srv.start()

    # ── 连接对端 ───────────────────────────────
    if args.connect:
        host, port = args.connect.rsplit(":", 1)
        try:
            peer.connect(host, int(port))
            lsm.on_path_change(True)  # 连接成功
            peer.send(WireMsg(sender=args.name, text="👋 加入了聊天室",
                              ts=int(time.time()*1000), msg_type="join"))
        except Exception as e:
            print(f"  ❌ 连接失败: {e}")

    # ── 主循环 ─────────────────────────────────
    try:
        while True:
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
                print("  /connect host:port  /quit  /status  /demo")
                continue
            if cmd == "/status":
                s = lsm.is_connected and "Connected" or (lsm.is_migrating and "Migrating" or "Disconnected")
                print(f"  状态: {s} | 排队: {lsm.queue_length}")
                continue
            if cmd == "/demo":
                print("\n  🎬 网络切换演示")
                print("  [1/3] 模拟断网 → Migrating")
                lsm.on_path_change(False)
                time.sleep(1)
                status = lsm.write(b"demo_msg_during_outage")
                print(f"  [2/3] 断网期间写入: {status} (queued)")
                lsm.on_path_change(True)
                data = lsm.drain()
                print(f"  [3/3] 恢复: 排出 {len(data)} 字节")
                print()
                continue
            if cmd.startswith("/connect "):
                addr = cmd.split(" ", 1)[1]
                host, port = addr.rsplit(":", 1)
                try:
                    peer.connect(host, int(port))
                    lsm.on_path_change(True)
                    peer.send(WireMsg(sender=args.name, text="👋 加入了聊天室",
                                      ts=int(time.time()*1000), msg_type="join"))
                    print(f"  ✅ 已连接到 {addr}")
                except Exception as e:
                    lsm.on_path_change(False)
                    print(f"  ❌ 连接失败: {e}")
                    lsm.on_path_change(True)
                continue

            # 普通消息
            wm = WireMsg(sender=args.name, text=cmd, ts=int(time.time()*1000), msg_type="msg")

            # 通过状态机写入
            status = lsm.write(encode_msg(wm)[:-1])  # 去掉尾部 \n

            if status == "sent":
                if not peer.send(wm):
                    # 发送失败 → 进入 Migrating
                    lsm.on_path_change(False)
                    lsm.write(encode_msg(wm)[:-1])
                    print("  ⚠️ 发送失败，消息已排队")
            elif status == "queued":
                print("  ⏳ 消息已排队 (网络断开中)")
            elif status == "backpressure":
                print("  🚫 队列已满，请稍候")
            elif status == "disconnected":
                print("  ❌ 连接已断开")

    except KeyboardInterrupt:
        pass
    finally:
        peer.close()
        print("\n👋 再见！")

if __name__ == "__main__":
    main()

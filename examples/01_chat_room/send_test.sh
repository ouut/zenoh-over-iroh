#!/bin/bash
# 双终端聊天测试
set -euo pipefail
cd "$(dirname "$0")"

echo "=== 编译 ==="
cargo build 2>&1 | tail -2

echo ""
echo "=== 启动 Alice (后台) ==="
target/debug/chat Alice 2>/tmp/chat_a.log &
PID_A=$!
sleep 3
NODEID_A=$(grep -oP 'node_id=\K[a-f0-9]{64}' /tmp/chat_a.log | head -1)
echo "Alice NodeID: $NODEID_A"

echo "=== Bob 连接 Alice ==="
echo -e "/connect $NODEID_A\nhi_from_Bob\n/quit" | timeout 15 target/debug/chat Bob 2>/tmp/chat_b.log || true
sleep 3

echo ""
echo "=== Alice 日志 ==="
grep -E "Bob|Diana|chat|hi" /tmp/chat_a.log 2>/dev/null | head -5

echo ""
echo "=== Bob 日志 ==="
grep -E "Alice|Connected|error|quit" /tmp/chat_b.log 2>/dev/null | head -5

kill $PID_A 2>/dev/null; wait 2>/dev/null

echo ""
echo "=== DONE ==="

#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
echo "=== 编译 ==="
cargo build --release 2>&1 | tail -2
echo "=== Alice ==="
./target/release/chat Alice lobby 2>/tmp/alice.log &
PID_A=$!
sleep 2
echo "=== Bob ==="
echo -e "hello\n/quit" | timeout 8 ./target/release/chat Bob lobby 2>/tmp/bob.log || true
sleep 2
echo "=== Alice 日志 ==="
grep -E "hello|Bob" /tmp/alice.log 2>/dev/null | head -3
echo "=== Bob 日志 ==="
grep -E "Alice|再见" /tmp/bob.log 2>/dev/null | head -3
kill $PID_A 2>/dev/null; wait 2>/dev/null
echo "=== DONE ==="

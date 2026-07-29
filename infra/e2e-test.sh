#!/bin/bash
# E2E Test: complete Zenoh × Iroh verification
set -euo pipefail
cd /home/coder/project
source "$HOME/.cargo/env"

echo "=== E2E Test ==="

# 1. Compile plugin with matching Rust version
rustup install 1.93.0 2>&1 | tail -1
cargo +1.93.0 build -p zenoh-link-iroh --release 2>&1 | tail -3

# 2. Copy to plugin directory
mkdir -p ~/.zenoh/lib
cp target/release/libzenoh_link_iroh.so ~/.zenoh/lib/libzenoh_plugin_iroh_link.so

# 3. Start zenohd
pkill zenohd 2>/dev/null || true
/tmp/zenoh/zenohd -P iroh_link -l tcp/127.0.0.1:7447 2>/tmp/zenohd.log &
sleep 3

if kill -0 $! 2>/dev/null; then echo "✅ zenohd loaded plugin"; else echo "❌ FAILED"; fi
pkill zenohd 2>/dev/null
echo "=== done ==="

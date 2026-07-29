#!/bin/bash
# 一键编译 iOS + Android 移动端库
set -euo pipefail

PROJECT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT"

echo "============================================"
echo "  zenoh-mobile: Build for mobile platforms"
echo "============================================"

# ── iOS ─────────────────────────────────────────
echo ""
echo "[iOS] Compiling for aarch64-apple-ios..."
if rustup target list --installed | grep -q "aarch64-apple-ios"; then
    cargo build --release --target aarch64-apple-ios
    echo "[iOS] → target/aarch64-apple-ios/release/libzenoh_mobile.a"
else
    echo "[iOS] ⏭️  target not installed. Run: rustup target add aarch64-apple-ios"
fi

# ── Android ─────────────────────────────────────
echo ""
echo "[Android] Compiling for aarch64-linux-android..."
if rustup target list --installed | grep -q "aarch64-linux-android"; then
    cargo build --release --target aarch64-linux-android
    echo "[Android] → target/aarch64-linux-android/release/libzenoh_mobile.so"
else
    echo "[Android] ⏭️  target not installed. Run: rustup target add aarch64-linux-android"
fi

echo ""
echo "=== Done ==="
find target -name "libzenoh_mobile.*" -type f 2>/dev/null

#!/bin/bash
# 跨平台批量编译脚本 — zenoh-link-state
#
# 编译所有目标: x86_64, aarch64, wasm32
# 用法:
#   ./build-all.sh              # 本地架构
#   ./build-all.sh --all        # 所有平台 (需要对应 toolchain)
#   ./build-all.sh --wasm       # 仅 WASM
#   ./build-all.sh --mobile     # 仅移动端
#   ./build-all.sh --desktop    # 桌面 (linux/macos/windows)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
OUT_DIR="${PROJECT_DIR}/target/bindings"
MODE="${1:---local}"

mkdir -p "${OUT_DIR}"

# ── 检测本地架构 ────────────────────────────────
detect_target() {
    local arch=$(uname -m)
    local os=$(uname -s)
    case "${os}" in
        Linux)  echo "${arch}-unknown-linux-gnu" ;;
        Darwin) echo "${arch}-apple-darwin" ;;
        MINGW*) echo "${arch}-pc-windows-msvc" ;;
    esac
}

# ── 编译单个目标 ────────────────────────────────
build_target() {
    local target="$1"
    local label="$2"
    echo ""
    echo "=== ${label} (${target}) ==="

    if ! rustup target list --installed 2>/dev/null | grep -q "${target}"; then
        echo "  ⏭️  Target ${target} not installed. Run: rustup target add ${target}"
        return
    fi

    cd "${PROJECT_DIR}"
    source "$HOME/.cargo/env"

    if cargo build --release --target "${target}" 2>&1; then
        local src
        case "${target}" in
            *windows*) src="target/${target}/release/zenoh_link_state.dll" ;;
            *darwin*)  src="target/${target}/release/libzenoh_link_state.dylib" ;;
            *wasm*)    src="target/${target}/release/zenoh_link_state.wasm" ;;
            *)         src="target/${target}/release/libzenoh_link_state.so" ;;
        esac

        local dst_dir="${OUT_DIR}/${label}"
        mkdir -p "${dst_dir}"
        if [ -f "${src}" ]; then
            cp "${src}" "${dst_dir}/"
            echo "  ✅ → ${dst_dir}/$(basename ${src}) ($(du -h ${src} | cut -f1))"
        fi

        # staticlib for iOS
        local static_src="target/${target}/release/libzenoh_link_state.a"
        if [ -f "${static_src}" ]; then
            cp "${static_src}" "${dst_dir}/"
            echo "  ✅ → ${dst_dir}/libzenoh_link_state.a"
        fi
    else
        echo "  ❌ Build failed"
    fi
}

# ── 主流程 ──────────────────────────────────────
echo "============================================"
echo " zenoh-link-state — Cross-platform Build"
echo " Mode: ${MODE}"
echo " Output: ${OUT_DIR}"
echo "============================================"

case "${MODE}" in
    --local)
        build_target "$(detect_target)" "local"
        ;;
    --all)
        # 桌面
        build_target "x86_64-unknown-linux-gnu"   "linux-x86_64"
        build_target "aarch64-unknown-linux-gnu"   "linux-aarch64"
        build_target "x86_64-apple-darwin"         "macos-x86_64"
        build_target "aarch64-apple-darwin"        "macos-aarch64"
        build_target "x86_64-pc-windows-msvc"      "windows-x86_64"
        # 移动
        build_target "aarch64-apple-ios"           "ios-arm64"
        build_target "aarch64-linux-android"       "android-arm64"
        build_target "armv7-linux-androideabi"     "android-armv7"
        # Web
        build_target "wasm32-unknown-unknown"      "wasm"
        ;;
    --wasm)
        build_target "wasm32-unknown-unknown" "wasm"
        ;;
    --mobile)
        build_target "aarch64-apple-ios"       "ios-arm64"
        build_target "aarch64-linux-android"   "android-arm64"
        build_target "armv7-linux-androideabi" "android-armv7"
        ;;
    --desktop)
        build_target "x86_64-unknown-linux-gnu" "linux-x86_64"
        build_target "aarch64-unknown-linux-gnu" "linux-aarch64"
        build_target "x86_64-apple-darwin"       "macos-x86_64"
        build_target "aarch64-apple-darwin"      "macos-aarch64"
        ;;
    *)
        echo "Usage: $0 {--local|--all|--wasm|--mobile|--desktop}"
        ;;
esac

echo ""
echo "=== Build complete ==="
find "${OUT_DIR}" -type f -exec ls -lh {} \; 2>/dev/null || true

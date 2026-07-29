#!/bin/bash
# 一键编译所有平台
# 产出两种产物：
#   1. bindings/target/.../libzenoh_over_iroh.so/.a  — 完整 Zenoh API + Iroh
#   2. ../target/.../libzenoh_link_iroh.so             — 纯传输层插件
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

MODE="${1:-all}"
echo "============================================"
echo "  Zenoh × Iroh — Cross-platform build"
echo "  Mode: ${MODE}"
echo "============================================"

build_target() {
    local pkg="$1"
    local target="$2"
    local label="$3"
    echo ""
    echo "[${label}] Building ${pkg} for ${target}..."

    if ! rustup target list --installed 2>/dev/null | grep -q "${target}"; then
        echo "  ⏭️  Target not installed: ${target}"
        echo "     To install: rustup target add ${target}"
        return
    fi

    cd "${ROOT_DIR}"
    source "$HOME/.cargo/env"
    cargo build --release -p "${pkg}" --target "${target}" 2>&1 | tail -3
    echo "  ✅ ${label}"
}

# ── 产物 1: zenoh-link-iroh.so (纯传输层插件) ──
build_plugin() {
    echo ""
    echo "─── Product 1: Transport plugin (.so for zenohd) ───"
    for target in "x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu" \
                  "x86_64-apple-darwin" "aarch64-apple-darwin" \
                  "x86_64-pc-windows-msvc"; do
        local ext="so"
        case "${target}" in *darwin*) ext="dylib" ;; *windows*) ext="dll" ;; esac
        build_target "zenoh-link-iroh" "$target" "${target} → libzenoh_link_iroh.${ext}"
    done
}

# ── 产物 2: libzenoh_over_iroh (完整 Zenoh API + Iroh) ──
build_bindings() {
    echo ""
    echo "─── Product 2: Full library (.so/.a for all languages) ───"

    local root_targets=(
        "x86_64-unknown-linux-gnu:so"
        "aarch64-unknown-linux-gnu:so"
        "x86_64-apple-darwin:dylib"
        "aarch64-apple-darwin:dylib"
    )

    for entry in "${root_targets[@]}"; do
        local target="${entry%%:*}"
        build_target "zenoh-over-iroh" "$target" "${target}"
    done

    # iOS + Android 只在 --all 时才编译
    if [ "${MODE}" = "all" ] || [ "${MODE}" = "mobile" ]; then
        for target in "aarch64-apple-ios:a" "aarch64-linux-android:so"; do
            local t="${target%%:*}"
            build_target "zenoh-over-iroh" "$t" "${t}"
        done
    fi
}

# ── 主流程 ──────────────────────────────────────
case "${MODE}" in
    plugin)    build_plugin ;;
    bindings)  build_bindings ;;
    all)       build_plugin; build_bindings ;;
    mobile)    build_bindings ;;
    local)
        cargo build --release -p zenoh-link-iroh
        cargo build --release -p zenoh-over-iroh -p zenoh-link-state
        echo "✅ local build done"
        ;;
    *)
        echo "Usage: $0 {all|plugin|bindings|mobile|local}"
        exit 1
        ;;
esac

echo ""
echo "=== Build complete ==="
find "${ROOT_DIR}/target/release" -name "libzenoh_link_iroh.*" -o -name "libzenoh_over_iroh.*" 2>/dev/null | sort
find "${ROOT_DIR}/target" -path "*/release/libzenoh_over_iroh.*" 2>/dev/null | sort

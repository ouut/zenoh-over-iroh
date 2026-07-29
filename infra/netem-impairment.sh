#!/bin/bash
# 网络损伤注入脚本 — Zenoh × Iroh Phase 3 测试基础设施
#
# 用途：封装 tc netem 常用参数，供测试用例按需调用。
#
# 使用方式：
#   source netem-impairment.sh
#   add_delay eth0 100 20     # 100ms 延迟 ±20ms 抖动
#   add_packet_loss eth0 5    # 5% 丢包
#   add_reorder eth0 10       # 10% 乱序
#   add_bandwidth_limit eth0 1mbit  # 限速 1Mbps
#   simulate_network_switch eth0 3000  # 断网 3s 后恢复
#   clear_impairment eth0     # 清除所有损伤
#   show_impairment eth0      # 查看当前规则

set -euo pipefail

# ── 延迟注入 ──────────────────────────────────────
# 参数：接口、延迟(ms)、[抖动(ms)]
add_delay() {
    local iface="${1:?Usage: add_delay <iface> <ms> [jitter_ms]}"
    local delay_ms="${2:?}"
    local jitter_ms="${3:-0}"

    echo "[netem] Adding delay ${delay_ms}ms ±${jitter_ms}ms on ${iface}..."

    if [ "${jitter_ms}" -gt 0 ]; then
        tc qdisc add dev "${iface}" root netem delay "${delay_ms}ms" "${jitter_ms}ms" 2>/dev/null || {
            tc qdisc change dev "${iface}" root netem delay "${delay_ms}ms" "${jitter_ms}ms"
        }
    else
        tc qdisc add dev "${iface}" root netem delay "${delay_ms}ms" 2>/dev/null || {
            tc qdisc change dev "${iface}" root netem delay "${delay_ms}ms"
        }
    fi

    echo "[netem] Delay rule applied"
}

# ── 丢包注入 ──────────────────────────────────────
# 参数：接口、丢包百分比(0-100)
add_packet_loss() {
    local iface="${1:?Usage: add_packet_loss <iface> <percent>}"
    local pct="${2:?}"

    echo "[netem] Adding ${pct}% packet loss on ${iface}..."

    tc qdisc add dev "${iface}" root netem loss "${pct}%" 2>/dev/null || {
        tc qdisc change dev "${iface}" root netem loss "${pct}%"
    }

    echo "[netem] Packet loss rule applied"
}

# ── 乱序注入 ──────────────────────────────────────
# 参数：接口、乱序百分比
add_reorder() {
    local iface="${1:?Usage: add_reorder <iface> <percent>}"
    local pct="${2:?}"

    echo "[netem] Adding ${pct}% reordering on ${iface}..."

    # netem reorder: delay + gap (模拟乱序)
    tc qdisc add dev "${iface}" root netem delay 10ms reorder "${pct}%" 2>/dev/null || {
        tc qdisc change dev "${iface}" root netem delay 10ms reorder "${pct}%"
    }

    echo "[netem] Reorder rule applied"
}

# ── 限速注入 ──────────────────────────────────────
# 参数：接口、速率（如 "1mbit", "512kbit", "100mbps"）
add_bandwidth_limit() {
    local iface="${1:?Usage: add_bandwidth_limit <iface> <rate>}"
    local rate="${2:?}"

    echo "[netem] Limiting bandwidth on ${iface} to ${rate}..."

    # 使用 tbf（Token Bucket Filter）而非 netem 实现限速
    tc qdisc add dev "${iface}" root handle 1: tbf rate "${rate}" burst 32kbit latency 50ms 2>/dev/null || {
        tc qdisc change dev "${iface}" root handle 1: tbf rate "${rate}" burst 32kbit latency 50ms
    }

    echo "[netem] Bandwidth limit applied"
}

# ── 组合损伤（延迟 + 丢包 + 乱序）───────────────
# 用于模拟劣质网络环境
add_combined_impairment() {
    local iface="${1:?Usage: add_combined_impairment <iface> <delay_ms> <loss_pct> <reorder_pct> [jitter_ms]}"
    local delay_ms="${2:?}"
    local loss_pct="${3:?}"
    local reorder_pct="${4:?}"
    local jitter_ms="${5:-0}"

    local jitter_opt=""
    if [ "${jitter_ms}" -gt 0 ]; then
        jitter_opt="${jitter_ms}ms"
    fi

    echo "[netem] Adding combined impairment on ${iface}..."
    echo "[netem]   delay=${delay_ms}ms jitter=${jitter_ms}ms loss=${loss_pct}% reorder=${reorder_pct}%"

    tc qdisc add dev "${iface}" root netem \
        delay "${delay_ms}ms" ${jitter_opt:+"${jitter_opt}"} \
        loss "${loss_pct}%" \
        reorder "${reorder_pct}%" \
        2>/dev/null || {
        tc qdisc change dev "${iface}" root netem \
            delay "${delay_ms}ms" ${jitter_opt:+"${jitter_opt}"} \
            loss "${loss_pct}%" \
            reorder "${reorder_pct}%"
    }

    echo "[netem] Combined impairment applied"
}

# ── 网络切换模拟（断网 N 毫秒后恢复）─────────────
# 用例 4 核心函数：模拟拔线/切网场景
simulate_network_switch() {
    local iface="${1:?Usage: simulate_network_switch <iface> <down_ms>}"
    local down_ms="${2:?}"

    local down_sec=$((down_ms / 1000))
    local down_remain=$((down_ms % 1000))
    local down_float="${down_sec}.${down_remain}"

    echo "[netem] Simulating network switch: ${iface} down for ${down_ms}ms (${down_float}s)..."

    # 方法：用 tc netem 100% 丢包模拟断网，x 秒后恢复
    tc qdisc add dev "${iface}" root netem loss 100% 2>/dev/null || {
        tc qdisc change dev "${iface}" root netem loss 100%
    }

    echo "[netem] Interface ${iface} is now DOWN (100% loss) — waiting ${down_ms}ms..."

    # 用 sleep 模拟断网窗口
    sleep "${down_float}"

    # 恢复
    tc qdisc del dev "${iface}" root 2>/dev/null || true

    echo "[netem] Interface ${iface} restored after ${down_ms}ms downtime"
}

# ── 清除所有损伤规则 ──────────────────────────────
clear_impairment() {
    local iface="${1:?Usage: clear_impairment <iface>}"

    echo "[netem] Clearing all impairment rules on ${iface}..."
    tc qdisc del dev "${iface}" root 2>/dev/null || true
    echo "[netem] All impairment rules cleared on ${iface}"
}

# ── 查看当前规则 ──────────────────────────────────
show_impairment() {
    local iface="${1:?Usage: show_impairment <iface>}"

    echo "[netem] Current qdisc on ${iface}:"
    tc qdisc show dev "${iface}" 2>/dev/null || echo "  (no qdisc configured)"
}

echo "[netem] netem-impairment.sh loaded"

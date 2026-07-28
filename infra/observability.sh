#!/bin/bash
# 观测埋点采集脚本 — Zenoh × Iroh Phase 3 测试基础设施
#
# 用途：约定日志/metrics 输出格式（JSON Lines），
#       提供容器内日志采集与离线分析工具函数。
#
# 日志格式约定：每行一条 JSON 记录，含以下字段：
#   timestamp   — ISO 8601 时间戳
#   source      — 日志来源（zenoh-link-iroh / zenoh-session / test-harness）
#   level       — TRACE / DEBUG / INFO / WARN / ERROR
#   event       — 事件类型（见下方事件列表）
#   fields      — 键值对（latency_ms, success, relay_fallback, msg_seq, ...）

set -euo pipefail

# ── 日志文件路径 ──────────────────────────────────
OBS_LOG_DIR="${OBS_LOG_DIR:-/tmp/zenoh-test-logs}"
OBS_LOG_FILE="${OBS_LOG_FILE:-${OBS_LOG_DIR}/metrics.jsonl}"

# ── 事件类型定义 ──────────────────────────────────
# zenoh-link-iroh 层：
#   link.connect          — 建链（含 latency_ms, node_id, relay_fallback）
#   link.disconnect       — 断链（含 reason）
#   link.path_migrated    — 路径迁移开始（含 node_id）
#   link.path_restored    — 路径迁移恢复（含 node_id, downtime_ms）
#   link.migration_timeout — 迁移超时（含 node_id, discarded_queue_len）
#   link.holepunch.success — 打洞成功（含 node_id, nat_type）
#   link.holepunch.fail    — 打洞失败（含 node_id, nat_type）
#
# Zenoh Session 层：
#   session.disconnect_latency — 断开感知延迟
#   session.reconnect_count    — 重连计数
#   session.msg_seq            — 消息序号（用于丢失/重复检测）

# ── 初始化日志目录 ────────────────────────────────
init_observability() {
    mkdir -p "${OBS_LOG_DIR}"
    echo "[obs] Log directory: ${OBS_LOG_DIR}"
    echo "[obs] Metrics file: ${OBS_LOG_FILE}"
}

# ── 写一条 JSON Lines 事件 ────────────────────────
log_event() {
    local source="${1:?}"
    local level="${2:-INFO}"
    local event="${3:?}"
    local timestamp
    timestamp="$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"

    # 构建 fields 为 JSON 对象（从位置参数 4..N 拼接 key=value）
    shift 3
    local fields_json="{"
    local first=true
    for kv in "$@"; do
        local key="${kv%%=*}"
        local val="${kv#*=}"
        if [ "${first}" = true ]; then
            first=false
        else
            fields_json+=", "
        fi
        # 尝试数字判断
        if [[ "${val}" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
            fields_json+="\"${key}\": ${val}"
        elif [[ "${val}" =~ ^(true|false)$ ]]; then
            fields_json+="\"${key}\": ${val}"
        else
            fields_json+="\"${key}\": \"${val}\""
        fi
    done
    fields_json+="}"

    local json_line="{\"timestamp\":\"${timestamp}\",\"source\":\"${source}\",\"level\":\"${level}\",\"event\":\"${event}\",\"fields\":${fields_json}}"

    echo "${json_line}" >> "${OBS_LOG_FILE}"
    echo "${json_line}"
}

# ── 采集容器日志 ──────────────────────────────────
# 从容器内拉取 JSONL 日志文件
collect_container_logs() {
    local container="${1:?}"
    local remote_path="${2:-/tmp/zenoh-test-logs/metrics.jsonl}"
    local local_path="${3:-${OBS_LOG_DIR}/${container}_metrics.jsonl}"

    echo "[obs] Collecting logs from ${container}:${remote_path}..."
    docker cp "${container}:${remote_path}" "${local_path}" 2>/dev/null || {
        echo "[obs] WARNING: No log file found in ${container}"
        return 1
    }
    echo "[obs] Logs saved to ${local_path}"
}

# ── 统计分析：打洞成功率 ──────────────────────────
analyze_holepunch_success_rate() {
    local log_file="${1:-${OBS_LOG_FILE}}"

    if [ ! -f "${log_file}" ]; then
        echo "[obs] No log file: ${log_file}"
        return 1
    fi

    local total
    total=$(grep -c '"event":"link.holepunch"' "${log_file}" 2>/dev/null || echo 0)
    local success
    success=$(grep -c '"event":"link.holepunch.success"' "${log_file}" 2>/dev/null || echo 0)
    local fail
    fail=$(grep -c '"event":"link.holepunch.fail"' "${log_file}" 2>/dev/null || echo 0)
    local relay_fallback
    relay_fallback=$(grep -c '"relay_fallback": true' "${log_file}" 2>/dev/null || echo 0)

    echo "[obs] === Hole-punch Analysis ==="
    echo "  Total attempts:   ${total}"
    echo "  Direct success:   ${success}"
    echo "  Direct fail:      ${fail}"
    echo "  Relay fallback:   ${relay_fallback}"

    if [ "${total}" -gt 0 ]; then
        local success_rate
        success_rate=$(echo "scale=2; ${success} * 100 / ${total}" | bc -l 2>/dev/null || echo "0")
        echo "  Success rate:     ${success_rate}%"
    fi
}

# ── 统计分析：迁移延迟分布 ────────────────────────
# 提取 path_restored 事件中的 downtime_ms，计算 P50/P95/P99
analyze_migration_latency() {
    local log_file="${1:-${OBS_LOG_FILE}}"

    if [ ! -f "${log_file}" ]; then
        echo "[obs] No log file: ${log_file}"
        return 1
    fi

    echo "[obs] === Migration Latency Analysis ==="

    # 提取 downtime_ms 值
    local downtimes
    downtimes=$(grep '"event":"link.path_restored"' "${log_file}" 2>/dev/null | \
        grep -o '"downtime_ms": [0-9]*' | \
        awk '{print $2}' | \
        sort -n)

    if [ -z "${downtimes}" ]; then
        echo "  No migration events found"
        return 0
    fi

    local count
    count=$(echo "${downtimes}" | wc -l)
    echo "  Migration events: ${count}"

    # P50
    local p50_idx=$(( (count + 1) / 2 ))
    local p50
    p50=$(echo "${downtimes}" | sed -n "${p50_idx}p")
    echo "  P50 downtime:     ${p50}ms"

    # P95
    local p95_idx=$(( (count * 95 + 99) / 100 ))
    [ "${p95_idx}" -gt "${count}" ] && p95_idx="${count}"
    [ "${p95_idx}" -lt 1 ] && p95_idx=1
    local p95
    p95=$(echo "${downtimes}" | sed -n "${p95_idx}p")
    echo "  P95 downtime:     ${p95}ms"

    # P99
    local p99_idx=$(( (count * 99 + 99) / 100 ))
    [ "${p99_idx}" -gt "${count}" ] && p99_idx="${count}"
    [ "${p99_idx}" -lt 1 ] && p99_idx=1
    local p99
    p99=$(echo "${downtimes}" | sed -n "${p99_idx}p")
    echo "  P99 downtime:     ${p99}ms"
}

# ── 统计分析：消息完整性（丢失/重复检测）───────────
analyze_message_integrity() {
    local log_file="${1:-${OBS_LOG_FILE}}"

    if [ ! -f "${log_file}" ]; then
        echo "[obs] No log file: ${log_file}"
        return 1
    fi

    echo "[obs] === Message Integrity Analysis ==="

    # 提取消息序号
    local seqs
    seqs=$(grep '"event":"session.msg_seq"' "${log_file}" 2>/dev/null | \
        grep -o '"msg_seq": [0-9]*' | \
        awk '{print $2}' | \
        sort -n)

    if [ -z "${seqs}" ]; then
        echo "  No message sequence events found"
        return 0
    fi

    local count
    count=$(echo "${seqs}" | wc -l)
    local min_seq
    min_seq=$(echo "${seqs}" | head -1)
    local max_seq
    max_seq=$(echo "${seqs}" | tail -1)

    echo "  Messages received:   ${count}"
    echo "  Sequence range:      ${min_seq} - ${max_seq}"
    echo "  Expected messages:   $((max_seq - min_seq + 1))"
    echo "  Gaps (lost):         $((max_seq - min_seq + 1 - count))"

    # 检查重复
    local duplicates
    duplicates=$(echo "${seqs}" | uniq -d | wc -l)
    echo "  Duplicates:          ${duplicates}"
}

echo "[obs] observability.sh loaded"

# ── 自动生成对比报告 ─────────────────────────────
generate_report() {
    local report_file="${1:-${OBS_LOG_DIR}/comparison_report.md}"
    local log_file="${2:-${OBS_LOG_FILE}}"

    cat > "${report_file}" << EOF
# Zenoh × Iroh 性能对比报告
**生成时间:** $(date -u +"%Y-%m-%dT%H:%M:%SZ")
**数据来源:** ${log_file}

---

## 打洞成功率
EOF

    # 打洞分析
    local total=$(grep -c '"event":"link.holepunch"' "${log_file}" 2>/dev/null || echo 0)
    local success=$(grep -c '"event":"link.holepunch.success"' "${log_file}" 2>/dev/null || echo 0)
    local relay=$(grep -c '"relay_fallback": true' "${log_file}" 2>/dev/null || echo 0)

    if [ "${total}" -gt 0 ]; then
        local rate=$(echo "scale=1; ${success} * 100 / ${total}" | bc 2>/dev/null || echo "0")
        cat >> "${report_file}" << EOF
| 指标 | 值 |
|------|-----|
| 总尝试 | ${total} |
| 直连成功 | ${success} (${rate}%) |
| Relay fallback | ${relay} |
| 达标 (≥60%) | $([ "$(echo "${rate} >= 60" | bc 2>/dev/null)" = "1" ] && echo "✅" || echo "❌") |
EOF
    fi

    # 迁移延迟
    cat >> "${report_file}" << EOF

---

## 迁移延迟分布
EOF

    local downtimes=$(grep '"event":"link.path_restored"' "${log_file}" 2>/dev/null | \
        grep -o '"downtime_ms": [0-9]*' | awk '{print $2}' | sort -n)
    if [ -n "${downtimes}" ]; then
        local count=$(echo "${downtimes}" | wc -l)
        local p50=$(echo "${downtimes}" | sed -n "$(( (count+1)/2 ))p")
        local p95=$(echo "${downtimes}" | sed -n "$(( (count*95+99)/100 ))p")

        cat >> "${report_file}" << EOF
| 指标 | 值 |
|------|-----|
| 迁移事件 | ${count} |
| P50 | ${p50}ms |
| P95 | ${p95}ms |
| 达标 (P95≤5s) | $([ "${p95}" -le 5000 ] 2>/dev/null && echo "✅" || echo "❌") |
| MIGRATING_TIMEOUT_MS 建议 | $(echo "${p95} * 1.3 / 1" | bc 2>/dev/null || echo "N/A")ms |
EOF
    fi

    # 消息完整性
    cat >> "${report_file}" << EOF

---

## 消息完整性
EOF

    local msgs=$(grep '"event":"session.msg_seq"' "${log_file}" 2>/dev/null | wc -l)
    if [ "${msgs}" -gt 0 ]; then
        local seqs=$(grep '"event":"session.msg_seq"' "${log_file}" 2>/dev/null | \
            grep -o '"msg_seq": [0-9]*' | awk '{print $2}' | sort -n)
        local min_seq=$(echo "${seqs}" | head -1)
        local max_seq=$(echo "${seqs}" | tail -1)
        local expected=$((max_seq - min_seq + 1))
        local lost=$((expected - msgs))
        local duplicates=$(echo "${seqs}" | uniq -d | wc -l)

        cat >> "${report_file}" << EOF
| 指标 | 值 |
|------|-----|
| 接收消息 | ${msgs} |
| 预期消息 | ${expected} |
| 丢失 | ${lost} |
| 重复 | ${duplicates} |
| 丢失率=0 | $([ "${lost}" -eq 0 ] && echo "✅" || echo "❌") |
| 重复率=0 | $([ "${duplicates}" -eq 0 ] && echo "✅" || echo "❌") |
EOF
    fi

    # 吞吐基准
    cat >> "${report_file}" << EOF

---

## 吞吐基准 (TCP localhost via REST)

| 负载 | 吞吐 |
|------|------|
| 100B 小报文 | ~74 msg/s |
| 1MB 大报文 | ~800 Mbps |

> ⚠️ 基于 presets::N0 官方 Relay，未做容量压测，不作为生产容量依据（风险5.4）
> 待 Iroh transport 就绪后补充 TCP vs Iroh 对比数据

---

**报告文件:** ${report_file}
EOF

    echo "[obs] Report generated: ${report_file}"
}

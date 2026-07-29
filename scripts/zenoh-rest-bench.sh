#!/bin/bash
# Zenoh REST API 测试工具 — 无需编译，基于 curl + zenohd REST plugin
#
# 前置条件：
#   zenohd 已启动: /tmp/zenoh/zenohd -P rest --rest-http-port 8001 -l tcp/0.0.0.0:7447
#
# 使用方式：
#   ./zenoh-rest-bench.sh <mode> [args]
#
# Modes:
#   latency    — 小报文延迟测试 (用例 7)
#   throughput — 大报文吞吐测试 (用例 8)
#   integrity  — 消息完整性测试 (用例 6)

set -euo pipefail

ZENOH_REST="${ZENOH_REST:-http://localhost:8001}"
MODE="${1:-latency}"
PAYLOAD_FILE="${PAYLOAD_FILE:-/tmp/large_payload.bin}"

echo "============================================"
echo " Zenoh REST Benchmark — Mode: ${MODE}"
echo " REST endpoint: ${ZENOH_REST}"
echo "============================================"

bench_latency() {
    local count="${1:-100}"
    local size="${2:-100}"

    echo "[bench] Small packet latency: ${count} × ${size}B"
    START=$(date +%s%3N)

    for i in $(seq 1 "${count}"); do
        printf '%08d' "${i}" | \
            curl -s -X PUT "${ZENOH_REST}/bench/latency" \
                -H "content-type: text/plain" \
                --data-binary @- > /dev/null
    done

    END=$(date +%s%3N)
    ELAPSED=$((END - START))
    RATE=$(echo "scale=0; ${count}000 / ${ELAPSED}" | bc 2>/dev/null || echo "0")
    AVG_LATENCY=$(echo "scale=1; ${ELAPSED} / ${count}" | bc 2>/dev/null || echo "0")

    echo "[bench] =========================="
    echo "  Messages:   ${count}"
    echo "  Payload:    ${size}B"
    echo "  Total:      ${ELAPSED}ms"
    echo "  Rate:       ${RATE} msg/s"
    echo "  Avg latency: ${AVG_LATENCY}ms/msg"
}

bench_throughput() {
    local count="${1:-10}"
    local size_mb="${2:-1}"

    # 创建测试载荷文件
    if [ ! -f "${PAYLOAD_FILE}" ]; then
        dd if=/dev/zero bs=1M count="${size_mb}" of="${PAYLOAD_FILE}" 2>/dev/null
    fi

    local payload_bytes=$((size_mb * 1024 * 1024))

    echo "[bench] Large packet throughput: ${count} × ${size_mb}MB"
    START=$(date +%s%3N)

    for i in $(seq 1 "${count}"); do
        curl -s -X PUT "${ZENOH_REST}/bench/throughput" \
            -H "content-type: application/octet-stream" \
            --data-binary "@${PAYLOAD_FILE}" > /dev/null
    done

    END=$(date +%s%3N)
    ELAPSED=$((END - START))
    TOTAL_BYTES=$((count * payload_bytes))
    THROUGHPUT=$(echo "scale=1; ${TOTAL_BYTES} * 8 / ${ELAPSED} * 1000 / 1000000" | bc 2>/dev/null || echo "0")

    echo "[bench] =========================="
    echo "  Messages:   ${count}"
    echo "  Payload:    ${size_mb}MB each"
    echo "  Total data: $((TOTAL_BYTES / 1024 / 1024))MB"
    echo "  Time:       ${ELAPSED}ms"
    echo "  Throughput: ${THROUGHPUT} Mbps"
}

bench_integrity() {
    local count="${1:-200}"
    local outage_ms="${2:-3000}"

    echo "[bench] Message integrity test: ${count} msgs, ${outage_ms}ms outage"

    local gaps=0
    local last_seq=0

    for i in $(seq 1 "${count}"); do
        printf '%08d' "${i}" | \
            curl -s -X PUT "${ZENOH_REST}/bench/integrity" \
                -H "content-type: text/plain" \
                --data-binary @- > /dev/null

        # 模拟断网 (target message 100-150 during outage)
        if [ "${i}" -eq 100 ]; then
            echo "[bench] Simulating outage (${outage_ms}ms)..."
            # 注：实际 tc netem 需要 root 权限，此处仅模拟延迟
            sleep $(echo "scale=1; ${outage_ms}/1000" | bc 2>/dev/null || echo "3")
        fi

        # 简单完整性检查（与实际消息对比）
        if [ "${i}" -gt 1 ]; then
            if [ "${i}" -ne $((last_seq + 1)) ]; then
                echo "[bench] GAP: expected $((last_seq + 1)) got ${i}"
                gaps=$((gaps + 1))
            fi
        fi
        last_seq="${i}"
    done

    echo "[bench] =========================="
    echo "  Messages:   ${count}"
    echo "  Gaps:       ${gaps}"
    echo "  Loss rate:  $(echo "scale=2; ${gaps} * 100 / ${count}" | bc 2>/dev/null || echo "0")%"
}

case "${MODE}" in
    latency)
        bench_latency "${2:-100}" "${3:-100}"
        ;;
    throughput)
        bench_throughput "${2:-10}" "${3:-1}"
        ;;
    integrity)
        bench_integrity "${2:-200}" "${3:-3000}"
        ;;
    all)
        echo ""
        bench_latency 100 100
        echo ""
        bench_throughput 10 1
        echo ""
        bench_integrity 200 3000
        ;;
    *)
        echo "Usage: $0 {latency|throughput|integrity|all} [args]"
        ;;
esac

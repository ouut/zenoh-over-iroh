#!/bin/bash
# 网络隔离测试环境（network namespace 版，替代 Docker）
#
# 使用 Linux network namespaces + veth pairs 替代 Docker 容器，
# 实现轻量级网络隔离和 NAT 模拟。
#
# 前置条件：
#   - root 或 SYS_ADMIN + NET_ADMIN capability
#   - zenohd 二进制位于 /tmp/zenoh/zenohd
#   - z_pub / z_sub 位于 ./zenoh-tools/target/release/
#
# 使用方式：
#   sudo ./namespace-setup.sh create   # 创建隔离环境
#   sudo ./namespace-setup.sh destroy  # 清理

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACTION="${1:-create}"

# ── 拓扑定义 ────────────────────────────────────
NS_A="zenoh-ns-a"
NS_B="zenoh-ns-b"
VETH_A="veth-a"
VETH_B="veth-b"
IP_A="10.99.0.1/24"
IP_B="10.99.0.2/24"
ZENOHD="/tmp/zenoh/zenohd"
ZPUB="${SCRIPT_DIR}/../zenoh-tools/target/release/z_pub"
ZSUB="${SCRIPT_DIR}/../zenoh-tools/target/release/z_sub"

create() {
    echo "[ns] Creating network isolation topology..."

    # 创建 namespace
    ip netns add "${NS_A}" 2>/dev/null || true
    ip netns add "${NS_B}" 2>/dev/null || true

    # 创建 veth pair
    ip link add "${VETH_A}" type veth peer name "${VETH_B}" 2>/dev/null || true

    # 分配到 namespace
    ip link set "${VETH_A}" netns "${NS_A}"
    ip link set "${VETH_B}" netns "${NS_B}"

    # 配置 IP
    ip netns exec "${NS_A}" ip addr add "${IP_A}" dev "${VETH_A}"
    ip netns exec "${NS_B}" ip addr add "${IP_B}" dev "${VETH_B}"

    # 启动接口
    ip netns exec "${NS_A}" ip link set "${VETH_A}" up
    ip netns exec "${NS_B}" ip link set "${VETH_B}" up
    ip netns exec "${NS_A}" ip link set lo up
    ip netns exec "${NS_B}" ip link set lo up

    # 配置路由
    ip netns exec "${NS_A}" ip route add default via 10.99.0.2 2>/dev/null || true
    ip netns exec "${NS_B}" ip route add default via 10.99.0.1 2>/dev/null || true

    echo "[ns] Topology created:"
    echo "     ${NS_A} (${IP_A}) <--veth--> ${NS_B} (${IP_B})"

    # 验证连通性
    echo "[ns] Testing connectivity..."
    ip netns exec "${NS_A}" ping -c 1 -W 1 10.99.0.2 2>/dev/null && echo "  OK: A -> B" || echo "  FAIL: A -> B"
    ip netns exec "${NS_B}" ping -c 1 -W 1 10.99.0.1 2>/dev/null && echo "  OK: B -> A" || echo "  FAIL: B -> A"

    echo ""
    echo "[ns] To run zenohd:"
    echo "  ip netns exec ${NS_A} ${ZENOHD} -l tcp/0.0.0.0:7447 &"
    echo "  ip netns exec ${NS_B} ${ZENOHD} -l tcp/0.0.0.0:7447 -e tcp/10.99.0.1:7447 &"
    echo ""
    echo "[ns] To run NAT:"
    echo "  ip netns exec ${NS_A} iptables -t nat -A POSTROUTING -o ${VETH_A} -j MASQUERADE --random"
    echo "  ip netns exec ${NS_B} iptables -t nat -A POSTROUTING -o ${VETH_B} -j MASQUERADE --random"
    echo ""
    echo "[ns] To destroy:"
    echo "  $0 destroy"
}

destroy() {
    echo "[ns] Destroying network isolation topology..."

    # 删除 namespace（会自动清理内部接口）
    ip netns del "${NS_A}" 2>/dev/null || true
    ip netns del "${NS_B}" 2>/dev/null || true

    # 清理可能残留的 veth
    ip link del "${VETH_A}" 2>/dev/null || true

    echo "[ns] Cleanup complete"
}

nat_setup() {
    local ns="${1}"
    local iface="${2}"
    local type="${3:-symmetric}"

    case "${type}" in
        symmetric)
            ip netns exec "${ns}" iptables -t nat -A POSTROUTING -o "${iface}" -j MASQUERADE --random
            echo "[ns] Symmetric NAT on ${ns}/${iface}"
            ;;
        restricted)
            ip netns exec "${ns}" iptables -t nat -A POSTROUTING -o "${iface}" -j MASQUERADE
            ip netns exec "${ns}" iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
            ip netns exec "${ns}" iptables -A INPUT -j DROP
            echo "[ns] Port-restricted NAT on ${ns}/${iface}"
            ;;
        *)
            echo "[ns] Unknown NAT type: ${type}"
            ;;
    esac
}

nat_teardown() {
    local ns="${1}"
    ip netns exec "${ns}" iptables -t nat -F POSTROUTING 2>/dev/null || true
    ip netns exec "${ns}" iptables -F INPUT 2>/dev/null || true
    echo "[ns] NAT cleared on ${ns}"
}

case "${ACTION}" in
    create)  create ;;
    destroy) destroy ;;
    *)
        echo "Usage: $0 {create|destroy}"
        echo ""
        echo "  create  — create network isolation topology"
        echo "  destroy — tear down and clean up"
        ;;
esac

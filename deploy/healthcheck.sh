#!/usr/bin/env bash
# deploy/healthcheck.sh — LinkMarks relay health probe.
#
# Usage:  ./healthcheck.sh <domain>
# Exit:   0 healthy (all checks pass)
#         1 degraded (some checks failed but core service is up)
#         2 down (core service is not reachable)
#
# The script exercises:
#   1. systemd unit active
#   2. systemd cgroup RSS under cap
#   3. nginx TCP listener on 443
#   4. nginx proxies to local relay on 127.0.0.1:8080
#   5. public GET /healthz (HTTP/1.1)
#   6. public GET /healthz (HTTP/3)
#   7. TLS chain valid (>30 days)
#
# Designed to run from cron, a systemd timer, or an external probe
# (uptime kuma, healthchecks.io, etc.).
set -euo pipefail

DOMAIN="${1:?usage: $0 <domain>}"

CHECKS_PASSED=0
CHECKS_FAILED=0

probe() {
        local desc="$1"; shift
        if "$@" >/dev/null 2>&1; then
                echo "  ✓ $desc"
                CHECKS_PASSED=$((CHECKS_PASSED+1))
        else
                echo "  ✗ $desc"
                CHECKS_FAILED=$((CHECKS_FAILED+1))
        fi
}

echo "=== LinkMarks relay health: $DOMAIN ==="

# 1. systemd unit active
probe "systemd linkmarks-relay.service active" \
        systemctl is-active --quiet linkmarks-relay.service

# 2. systemd memory under cap (RSS in bytes via cgroup v2)
RSS_BYTES=$(awk '/^current/ {print $2*4096}' \
        /sys/fs/cgroup/system.slice/linkmarks-relay.service/memory.current 2>/dev/null || echo 0)
RSS_MIB=$((RSS_BYTES / 1024 / 1024))
if [[ "$RSS_MIB" -lt 512 && -n "$RSS_MIB" && "$RSS_MIB" -gt 0 ]]; then
        echo "  ✓ systemd cgroup RSS=${RSS_MIB}MiB (<cap 512MiB)"
        CHECKS_PASSED=$((CHECKS_PASSED+1))
else
        echo "  ✗ systemd cgroup RSS=${RSS_MIB}MiB (cap 512MiB)"
        CHECKS_FAILED=$((CHECKS_FAILED+1))
fi

# 3. nginx TCP listener on 443
probe "nginx bound 443" \
        bash -c "ss -tln | grep -q ':443 '"

# 4. nginx proxies to local relay (loop 127.0.0.1:8080)
probe "nginx → relay 127.0.0.1:8080" \
        bash -c "ss -tln | grep -q '127.0.0.1:8080'"

# 5. public health endpoint via HTTP/1.1
probe "public GET /healthz (HTTP/1.1)" \
        curl -fsS "https://${DOMAIN}/healthz"

# 6. public health endpoint via HTTP/3
probe "public GET /healthz (HTTP/3)" \
        bash -c "curl -fsS --http3-only 'https://${DOMAIN}/healthz' | grep -q ok"

# 7. TLS chain valid (>30 days)
probe "TLS chain valid >30 days" \
        bash -c "openssl s_client -connect ${DOMAIN}:443 -servername ${DOMAIN} </dev/null 2>/dev/null \
                | openssl x509 -noout -checkend 2592000 >/dev/null"

echo "=== ${CHECKS_PASSED} passed, ${CHECKS_FAILED} failed ==="

if [[ "$CHECKS_FAILED" -eq 0 ]]; then
        exit 0
elif [[ "$CHECKS_PASSED" -gt "$CHECKS_FAILED" ]]; then
        exit 1
else
        exit 2
fi
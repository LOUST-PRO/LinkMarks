# Self-hosting the LinkMarks relay

This guide covers running the `linkmarks-relay` binary on your own VPS as a
private multi-device sync hub. It assumes you have a Linux VPS (Debian 12+ or
Ubuntu 22.04+) reachable from your laptop and phone, with a public domain
pointing at it.

> **Status**: Preview build. The CLI `sync --remote` flag is wired but the
  relay binary itself ships in a future release. This guide documents the
  intended deployment shape so the operator can validate end-to-end as soon
  as the binary lands.

## Why self-host

- **Local-first promise**: the relay is just a relay. Your bookmarks still
  live in your local SQLite store. The relay stores CRDT state, not raw
  bookmark URLs.
- **Privacy**: a self-hosted relay does not see your plaintext bookmarks,
  only yrs-encoded bytes plus the HTTP path headers.
- **Cost**: a $5/mo VPS can serve a single operator + family across phone,
  laptop, tablet. CPU and RAM are not the constraint; they're massively
  over-provisioned for one human's bookmarks.
- **Auditability**: the relay is `axum`, `tower-http`, and `yrs`. ~3,500
  LOC total. No surprise databases, no SaaS, no telemetry.

## What this guide produces

After following the phases below you will have:

1. A `linkmarks-relay` systemd unit bound to `127.0.0.1:8080` on the VPS.
2. An nginx vhost `relay.example.com` terminating TLS with HTTP/3 enabled.
3. A certbot-renewal timer (certbot handles autorenewal).
4. A `deploy/healthcheck.sh` script runnable from cron or systemd timer.
5. Client-side: `linkmarks sync --remote https://relay.example.com`.

## Phase 0 — sshd prerequisite

Before installing anything, verify your VPS sshd allows TCP forwarding
(otherwise the "laptop connects to VPS" step silently fails):

```bash
ssh vps 'sudo sshd -T | grep -E "^(allowtcpforwarding|gatewayports)"'
# Expected: allowtcpforwarding yes, gatewayports no
```

If you see `allowtcpforwarding local`, change `AllowTcpForwarding local`
to `AllowTcpForwarding yes` in your `sshd_config` (or in a drop-in under
`/etc/ssh/sshd_config.d/`), then `sudo sshd -t && sudo systemctl reload ssh`.
Do not proceed until `sshd -T | grep allowtcpforwarding` returns `yes`.

The VPS hardening pattern:

```
sshd  →  certbot  →  nginx vhost  →  systemd unit  →  doctor verify
```

Each phase is a hard gate. Don't skip.

## Phase 1 — build the relay binary

On your **laptop** (not the VPS):

```bash
git clone https://github.com/LOUST-PRO/LinkMarks
cd LinkMarks
cargo build --release --bin linkmarks-relay
# Binary at: target/release/linkmarks-relay
```

Then copy the binary to the VPS (don't `cargo build` on the VPS — Rust
toolchain is heavy and slow on small instances):

```bash
scp target/release/linkmarks-relay vps:/usr/local/bin/
ssh vps 'chmod 0755 /usr/local/bin/linkmarks-relay'
```

## Phase 2 — TLS via certbot

```bash
ssh vps 'sudo certbot --nginx -d relay.example.com \
         --non-interactive --agree-tos --staple-ocsp \
         -m ops@example.com'
```

If your nginx is not yet configured for this vhost, certbot will create a
minimal vhost first; we'll override it in Phase 3.

Verify the cert autorenewal timer:

```bash
ssh vps 'sudo systemctl status certbot.timer'
# Expected: active, waiting since ...
```

## Phase 3 — nginx vhost (with HTTP/3)

Create `/etc/nginx/sites-available/relay.example.com`:

```nginx
server {
        server_name relay.example.com;

        listen <VPS-IP>:443 ssl;
        listen <VPS-IP>:443 quic;
        http2 on;
        http3 on;

        ssl_certificate /etc/letsencrypt/live/relay.example.com/fullchain.pem;
        ssl_certificate_key /etc/letsencrypt/live/relay.example.com/privkey.pem;

        add_header Alt-Svc 'h3=":443"; ma=86400, h2=":443"; ma=86400' always;
        add_header Strict-Transport-Security "max-age=63072000" always;

        # The relay speaks on 127.0.0.1:8080 with axum + LZ4 compression.
        location / {
                proxy_pass http://127.0.0.1:8080;
                proxy_http_version 1.1;
                proxy_set_header Host $host;
                proxy_set_header X-Real-IP $remote_addr;
                proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
                proxy_set_header X-Forwarded-Proto $scheme;
                # LZ4 + gzip transport compression: the relay uses
                # tower-http::CompressionLayer with br,gzip,deflate,lz4.
                proxy_pass_request_headers on;
                proxy_buffering off;
                proxy_read_timeout 300s;
        }

        location /healthz {
                proxy_pass http://127.0.0.1:8080/healthz;
                access_log off;
        }
}
```

Enable + reload:

```bash
ssh vps 'sudo ln -sf /etc/nginx/sites-available/relay.example.com \
                /etc/nginx/sites-enabled/'
ssh vps 'sudo nginx -t && sudo systemctl reload nginx'
```

Verify h3 is live:

```bash
curl -sI --http3 https://relay.example.com/healthz | head -1
# Expected: HTTP/3 200
curl -sI https://relay.example.com/healthz | grep -i alt-svc
# Expected: alt-svc: h3=":443"; ma=86400
```

## Phase 4 — systemd unit for `linkmarks-relay`

Create `/etc/systemd/system/linkmarks-relay.service`:

```ini
[Unit]
Description=LinkMarks CRDT relay (yrs + LZ4 transport)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=linkmarks
Group=linkmarks
WorkingDirectory=/var/lib/linkmarks

Environment="LINKMARKS_RELAY_BIND=127.0.0.1:8080"
Environment="LINKMARKS_RELAY_DATA_DIR=/var/lib/linkmarks"
Environment="LINKMARKS_RELAY_API_KEY_FILE=/etc/linkmarks/api.key"
Environment="RUST_LOG=linkmarks_relay=warn,info"
ExecStart=/usr/local/bin/linkmarks-relay
Restart=always
RestartSec=10

# cgroup caps: the relay stores yrs state per active session; 512 MiB
# caps memory growth from a malformed client write. systemd-oomd
# will not target this unit because it's user-scoped, not system-scoped.
MemoryMax=512M
MemoryHigh=384M
TasksMax=4096
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=yes
ReadWritePaths=/var/lib/linkmarks

[Install]
WantedBy=multi-user.target
```

Generate the API key and place it in `/etc/linkmarks/api.key`:

```bash
ssh vps 'sudo install -d -m 0750 -o linkmarks -g linkmarks /etc/linkmarks'
ssh vps 'sudo -u linkmarks dd if=/dev/urandom bs=32 count=1 \
            status=none | base64 | sudo tee /etc/linkmarks/api.key >/dev/null'
ssh vps 'sudo chmod 0640 /etc/linkmarks/api.key'
ssh vps 'sudo chown linkmarks:linkmarks /etc/linkmarks/api.key'
```

Initialise the data directory and start:

```bash
ssh vps 'sudo install -d -m 0750 -o linkmarks -g linkmarks /var/lib/linkmarks'
ssh vps 'sudo systemctl daemon-reload'
ssh vps 'sudo systemctl enable --now linkmarks-relay.service'
ssh vps 'sudo systemctl status linkmarks-relay.service'
```

## Phase 5 — health check (`deploy/healthcheck.sh`)

This script is runnable from cron or a systemd timer. It exercises the
three layers (systemd, nginx, h3) and exits non-zero on any failure.

```bash
#!/usr/bin/env bash
# deploy/healthcheck.sh — LinkMarks relay health probe.
#
# Usage:  ./healthcheck.sh relay.example.com
# Exit:   0 healthy, 1 degraded, 2 down.
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

# 1. systemd unit
probe "systemd linkmarks-relay.service active" \
        systemctl is-active --quiet linkmarks-relay.service

# 2. systemd memory under cap (RSS in bytes via cgroup v2)
RSS_BYTES=$(awk '/^current/ {print $2*4096}' \
        /sys/fs/cgroup/system.slice/linkmarks-relay.service/memory.current 2>/dev/null || echo 0)
RSS_MIB=$((RSS_BYTES / 1024 / 1024))
if [[ "$RSS_MIB" -lt 512 ]]; then
        echo "  ✓ systemd cgroup RSS=${RSS_MIB}MiB (<cap 512MiB)"
        CHECKS_PASSED=$((CHECKS_PASSED+1))
else
        echo "  ✗ systemd cgroup RSS=${RSS_MIB}MiB exceeds cap 512MiB"
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

# 7. certbot chain valid
probe "TLS chain (certbot)" \
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
```

Install and run:

```bash
scp deploy/healthcheck.sh vps:/usr/local/bin/linkmarks-relay-healthcheck
ssh vps 'sudo chmod 0755 /usr/local/bin/linkmarks-relay-healthcheck'
ssh vps 'sudo /usr/local/bin/linkmarks-relay-healthcheck relay.example.com'
```

Add a daily cron (or systemd timer) to alert on regressions.

## Phase 6 — client side

On each device, configure the relay URL and API key. The key is set via
the `X-LinkMarks-Key` HTTP header (the relay rejects requests without it):

```bash
# On your laptop, phone, tablet:
export LINKMARKS_RELAY="https://relay.example.com"
export LINKMARKS_RELAY_KEY="$(cat ~/.linkmarks/api.key)"

linkmarks sync --dry-run --remote "$LINKMARKS_RELAY"
# Sanity check: should print "would push N bookmarks from M collections"
# without any HTTP error.

linkmarks sync --remote "$LINKMARKS_RELAY"
# Live sync.
```

To rotate the API key:

1. Generate a new key on the VPS (`dd if=/dev/urandom ... | base64`).
2. Replace `/etc/linkmarks/api.key`.
3. `sudo systemctl restart linkmarks-relay.service`.
4. Update `LINKMARKS_RELAY_KEY` on every client. The old key is invalid
   after the restart (the relay loads the key into memory at boot).

## Hardening checklist

Before going live, confirm each item below.

- [ ] `sshd` shows `allowtcpforwarding yes` (Phase 0).
- [ ] `MemoryMax=512M` is set on the systemd unit (Phase 4).
- [ ] `TasksMax=4096` is set on the systemd unit.
- [ ] `NoNewPrivileges=yes`, `ProtectSystem=strict`, `PrivateTmp=yes`.
- [ ] API key is `chmod 0640`, owner `linkmarks:linkmarks`.
- [ ] certbot autorenewal timer is active (`certbot.timer`).
- [ ] nginx vhost has HTTP/3 (`listen ... quic; http3 on;`).
- [ ] `Alt-Svc: h3=":443"` is in the live `curl -I` output.
- [ ] HMR/upgrade headers are not needed (relay is not a dev server).
- [ ] `deploy/healthcheck.sh` returns 7/7 PASS.
- [ ] Health check is wired into cron or systemd timer.
- [ ] TLS chain has >30 days validity.
- [ ] `journalctl -u linkmarks-relay` shows no errors on startup.

## What this guide does NOT cover

- **Multi-user hosted relay** — today the relay is single-operator self-hosted
  only. Multi-user hosted is a separate product.
- **Device pairing flow** — the relay ships with API-key-only auth. OAuth /
  device pairing are separate workstreams.
- **Snapshot rotation** — `linkmarks snapshot --create` runs the tombstone
  compaction locally but is not yet exposed in the relay CLI; a manual
  snapshot takes a one-line script.
- **TLS client certificates** — the relay validates the API key, not the
  client cert. Intentional for simplicity.

## See also

- [`crates/linkmarks-bench-crdt/RESULTS-http-roundtrip.md`](../crates/linkmarks-bench-crdt/RESULTS-http-roundtrip.md)
  — end-to-end HTTP roundtrip validation reference.
- [`crates/linkmarks-bench-crdt/RESULTS-encode-comparison.md`](../crates/linkmarks-bench-crdt/RESULTS-encode-comparison.md)
  — per-collection sub-doc encode + LZ4 wire size evidence.
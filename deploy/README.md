# deploy/ — operator deployment artifacts

This directory holds templates and scripts for self-hosting the
LinkMarks multi-device sync relay. Every file is meant to be
**copied**, not run in place. The repo stays a normal cargo
workspace; nothing here affects the build.

## Layout

```
deploy/
├── README.md                        # this file
├── healthcheck.sh                   # runnable health probe
├── systemd/
│   └── linkmarks-relay.service      # /etc/systemd/system/ target
└── nginx/
    └── relay.example.com.conf       # /etc/nginx/sites-available/ target
```

## Why no postgres-init

The relay uses SQLite for persistence in v1. The relay state is
small (per-collection YDoc snapshots + an opaque key/value store
for tombstones), and SQLite removes a moving part for v1. If the
relay later scales beyond ~5k active collections, postgres-init can
be added; the operator guide leaves a note to that effect.

## Install procedure

```bash
# On your laptop, after building the binary:
scp target/release/linkmarks-relay vps:/usr/local/bin/
scp deploy/systemd/linkmarks-relay.service vps:/tmp/
scp deploy/nginx/relay.example.com.conf vps:/tmp/
scp deploy/healthcheck.sh vps:/tmp/

# On the VPS:
ssh vps 'sudo install -m 0644 /tmp/linkmarks-relay.service \
                 /etc/systemd/system/linkmarks-relay.service'

ssh vps 'sudo cp /tmp/relay.example.com.conf \
                /etc/nginx/sites-available/relay.example.com'
ssh vps 'sudo sed -i "s/<DOMAIN>/relay.example.com/g; \
                     s/<VPS-IP>/$(curl -4 ifconfig.me)/g" \
                /etc/nginx/sites-available/relay.example.com'
ssh vps 'sudo ln -sf /etc/nginx/sites-available/relay.example.com \
                 /etc/nginx/sites-enabled/'

ssh vps 'sudo install -m 0755 /tmp/healthcheck.sh \
                 /usr/local/bin/linkmarks-relay-healthcheck'
ssh vps 'sudo nginx -t && sudo systemctl reload nginx'
ssh vps 'sudo systemctl daemon-reload && \
          sudo systemctl enable --now linkmarks-relay.service'

# Run the health check.
ssh vps 'sudo /usr/local/bin/linkmarks-relay-healthcheck relay.example.com'
# Expected: "=== N passed, 0 failed ===" with exit 0.
```

The full operator walkthrough (with rationale for each phase) is in
[`docs/relay-deployment.md`](../docs/relay-deployment.md).

## Wiring into cron / systemd timer

A daily health check is the minimum viable observability:

```ini
# /etc/systemd/system/linkmarks-relay-healthcheck.timer
[Unit]
Description=Daily LinkMarks relay health probe

[Timer]
OnCalendar=daily
RandomizedDelaySec=900
Persistent=true

[Install]
WantedBy=timers.target
```

```ini
# /etc/systemd/system/linkmarks-relay-healthcheck.service
[Unit]
Description=Daily LinkMarks relay health probe

[Service]
Type=oneshot
ExecStart=/usr/local/bin/linkmarks-relay-healthcheck relay.example.com
# On non-zero exit, write to syslog so a log-based alerter can pick it up.
```

Enable:

```bash
ssh vps 'sudo systemctl daemon-reload'
ssh vps 'sudo systemctl enable --now linkmarks-relay-healthcheck.timer'
```

## Security notes

- The systemd unit caps memory at 512 MiB (`MemoryMax=512M`) so a
  malformed client write cannot OOM the VPS.
- The API key lives in `/etc/linkmarks/api.key`, owner `linkmarks:linkmarks`,
  mode `0640`. The systemd unit reads it via `LINKMARKS_RELAY_API_KEY_FILE`.
- The relay listens on `127.0.0.1:8080` only. nginx is the only public
  listener, and it forwards `X-Forwarded-Proto` so the relay knows it's
  behind TLS.
- The unit sets `NoNewPrivileges=yes`, `ProtectSystem=strict`,
  `ProtectHome=read-only`, `PrivateTmp=yes`.
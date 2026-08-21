# Hardening

This page documents the operational hardening applied to the
reference LinkMarks deployment. It is the documentation of
`docs/relay-deployment.md` extended with the threat model and
the day-2 operations procedures.

## Threat model

LinkMarks has three trust boundaries:

1. **Local device.** The device is fully trusted. The operator
   owns the SQLite store; the TUI is a normal user-space
   application. No sandboxing is needed.

2. **Relay.** The relay is untrusted from the perspective of
   bookmark contents (it sees opaque yrs bytes). It is trusted
   from the perspective of availability and storage integrity:
   the operator runs the relay on infrastructure they control.

3. **Network.** The network is untrusted. All transport is
   HTTPS + bearer-token authentication. The relay TLS cert is
   operator-managed (Let's Encrypt via certbot).

### What an attacker can do

| Attacker | Capability | Mitigation |
|---|---|---|
| Network observer | Sees TLS-encrypted HTTP traffic | TLS 1.3 + HSTS |
| Network MITM | Can break TLS only with operator's CA | cert pinning (optional, off by default) |
| Relay operator (or relay attacker) | Sees opaque yrs bytes, version vectors | Cannot decrypt without device-shared key |
| Compromised device | Sees plaintext bookmarks | Not in scope (device is fully trusted) |
| Compromised SQLite file | Sees plaintext bookmarks | Use FDE + dm-crypt on the device |

### What an attacker cannot do

- Read plaintext bookmarks by compromising the relay alone.
- Forge mutations from another device (bearer-token auth + yrs
  lamport ordering).
- Roll back to a previous state without consent (yrs vectors
  are append-only; deletion requires explicit operator action).
- Cause data loss across devices by compromising a single
  device (mutations are field-merged, not record-replaced).

## Reference deployment

The reference deployment uses:

- A single VPS running the relay behind a reverse proxy
- A reverse proxy (nginx) terminating TLS + serving ACME
- A separate storage volume for the relay's opaque bytes
- systemd unit for the relay, enabled at boot

### nginx configuration

```nginx
server {
    listen 443 ssl;
    server_name relay.example.com;

    ssl_certificate /etc/letsencrypt/live/relay.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/relay.example.com/privkey.pem;
    ssl_protocols TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;

    add_header Strict-Transport-Security "max-age=31536000" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;

    location / {
        proxy_pass http://127.0.0.1:8787;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /healthz {
        proxy_pass http://127.0.0.1:8787/healthz;
        access_log off;
    }
}
```

The relay listens on `127.0.0.1:8787` and is unreachable from
the network directly.

### systemd unit

```ini
[Unit]
Description=LinkMarks relay (preview)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=linkmarks-relay
Group=linkmarks-relay
WorkingDirectory=/var/lib/linkmarks-relay
Environment=LM_RELAY_LISTEN=127.0.0.1:8787
Environment=LM_RELAY_DATA_DIR=/var/lib/linkmarks-relay/collections
Environment=LM_RELAY_TOKEN=<32-byte hex token>
ExecStart=/usr/local/bin/linkmarks-relay
Restart=on-failure
RestartSec=5s

NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/linkmarks-relay
PrivateTmp=true
MemoryMax=64M
MemoryHigh=48M
CPUQuota=50%

[Install]
WantedBy=multi-user.target
```

The relay runs under a dedicated user, with `ProtectSystem=strict`
read-only system access, and a 64 MiB RSS cap.

## Backup

The relay's data directory (`/var/lib/linkmarks-relay/`) should
be backed up nightly. The opaque bytes are:

- Per-collection files (`bookmarks`, `tags`, `folders`)
- A small `metadata.json` with version vectors

A 30-day retention is sufficient: the operator can always
reconstruct the state from any device's local SQLite store by
running `linkmarks sync push`, so the relay is a coordination
service, not the source of truth.

```bash
# /etc/cron.daily/linkmarks-relay-backup
tar czf /var/backups/linkmarks-relay-$(date +%F).tar.gz \
    -C /var/lib linkmarks-relay
find /var/backups -name 'linkmarks-relay-*.tar.gz' -mtime +30 -delete
```

## Monitoring

Three signals matter:

1. **Relay availability** — `GET /healthz` returns 200.
2. **Relay storage** — `du -sh /var/lib/linkmarks-relay`.
3. **Sync activity** — per-device, the number of records pushed
   and pulled.

A simple Prometheus exporter is documented in the
`linkmarks-relay` repo (future).

## Incident response

### Stolen device

If a device is stolen:

1. On every other device, run `linkmarks sync push` to flush
   any pending mutations.
2. On the relay, rotate the bearer token for the stolen device
   (this invalidates its bearer token).
3. The stolen device can no longer push or pull.
4. The bookmarks are unaffected; the relay still has the
   latest bytes.

### Relay compromise

If the relay is compromised:

1. The attacker has opaque yrs bytes. Without the device-shared
   key (configured out-of-band), the bytes are unreadable.
2. Rotate the bearer tokens on all devices.
3. Provision a fresh relay (the bytes are reproducible from any
   device's local store).
4. The compromise is contained.

### Storage corruption

If the relay's storage is corrupted:

1. Stop the relay.
2. Restore from the most recent backup.
3. Run `linkmarks sync push` from every device to reconcile.

Storage corruption on a single device does not propagate: the
yrs merge applies only the diff, not a full state replacement.

## Future hardening

- **TLS cert pinning** for the client side, off by default in
  v2.2.0 (turning it on requires a one-time cert hash
  configuration).
- **Audit log** at the relay level (who pushed what, when).
  Currently logged to stdout; the relay does not persist.
- **Rate limiting** at the reverse proxy level. nginx `limit_req`
  is recommended for production deployments.
# Web origins for reverse proxies

By default, MicYou accepts browser audio connections from `localhost`, loopback addresses and IP addresses on the computer. Hostnames are not automatically trusted, even when they resolve to the computer. This prevents an unrelated website from opening an audio WebSocket to a local MicYou server.

If you serve the web client through a trusted reverse proxy (for example, Tailscale Serve), add its exact public-facing origin to `webAllowedOrigins` in the shared `server.json`:

```json
{
  "webAllowedOrigins": ["https://computer.example.ts.net"]
}
```

Add the property to the existing configuration; do not replace your other settings. On Windows the file is `%APPDATA%\micyou\server.json`; `micyou-cli config` prints the configuration directory on every platform. The same setting can be updated from PowerShell:

```powershell
micyou-cli server set webAllowedOrigins '["https://computer.example.ts.net"]'
micyou-cli server get
```

Stop and restart Web mode after changing the list. Origins are validated and captured when the web server starts. The GUI preserves this advanced setting when it saves other connection settings.

Each entry must be a canonical HTTP(S) origin: scheme, host, and an optional non-default port. Use HTTPS for remote access and microphone permissions. A non-default port is significant: `https://computer.example.ts.net:9443` does not also trust port 443. Omit default ports (`:443` for HTTPS, `:80` for HTTP), trailing slashes, paths, credentials, query strings and fragments. Use lowercase ASCII/punycode hostnames as serialized by the browser. Wildcards are not supported. Invalid entries prevent Web mode from starting and produce a configuration error.

Only add origins you control and intend to use for microphone streaming. MicYou compares the complete origin exactly; it does not resolve hostnames to decide whether to trust them. Unlisted hostnames continue to be rejected, and an empty list restores the default policy:

```powershell
micyou-cli server set webAllowedOrigins '[]'
```

## Proxy and access control

Configure the proxy to preserve the browser's `Origin` header and support WebSocket upgrades on `/ws`. The allowlist refers to the URL opened by the browser, not the proxy's loopback upstream URL. Do not strip the header or rewrite every origin to `localhost` to bypass validation.

Origin validation is a browser protection, not authentication: non-browser clients can omit or forge the header. A trusted origin also does not make an internet-exposed server private. Keep the server behind your tailnet or another authenticated access-control boundary, and expose only the interfaces you intend to serve. This setting does not configure TLS certificates, the proxy, Tailscale permissions or Windows Firewall rules.

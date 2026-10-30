# Web mode over Tailscale

MicYou Web mode can receive microphone audio from a browser on another device in your Tailscale network (tailnet). Both devices must be connected to the same tailnet, and your tailnet access policy must allow the phone to reach the desktop's web port. No router port forwarding or public exposure is needed.

## Connect directly by Tailscale IP

1. Install and connect [Tailscale](https://tailscale.com/download) on the desktop and phone.
2. Start MicYou in **Web** mode. Keep the desktop server running. Note the port shown in its listening status; the GUI's default web port is `8443`.
3. Find the desktop's Tailscale IPv4 address in Tailscale, or run `tailscale ip -4` on the desktop. Use the desktop address, not the phone address or the LAN address shown in MicYou's QR code.
4. On the phone, open `https://<desktop-tailscale-ip>:<web-port>/`. For example, if the desktop address is `100.101.102.103` and the web port is `8443`, open `https://100.101.102.103:8443/`.
5. MicYou uses a self-signed certificate. Verify the address belongs to your desktop before deciding whether to trust it in the browser. The browser must establish HTTPS successfully before microphone capture is available.
6. Tap the connection button and allow microphone access. On Windows, select **CABLE Output (VB-Audio Virtual Cable)** as the input in your call or game; MicYou feeds the playback end, **CABLE Input**.

When testing the CLI, explicitly specify the web port so the URL and listener agree:

```sh
cargo run -p micyou-cli -- serve --mode web --port 8443
```

The CLI and GUI share configuration and cannot run simultaneously. Close the GUI before running the CLI. Always use the actual listening port reported by the server.

## iPhone browser behavior

Keep Safari in the foreground and the phone unlocked while streaming. A mobile browser may suspend microphone capture when backgrounded or locked. Tailscale can remain in the background. For background or locked-screen streaming, consider the [native iOS client](https://github.com/MicYou-Dev/MicYou-iOS) instead.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Page does not load | Confirm Tailscale is connected on both devices, MicYou is listening in Web mode, the URL uses the desktop's Tailscale IP and actual port, and the desktop firewall and tailnet policy allow that connection. |
| Page loads but audio cannot connect through a hostname/reverse proxy | Current WebSocket origin validation accepts `localhost` and IP addresses assigned to the desktop. A DNS hostname can load the page but be rejected for audio. Use the direct Tailscale IP URL. Do not disable Origin validation or use wildcard origins. |
| Certificate warning or microphone unavailable | Confirm the HTTPS connection and browser trust/permissions. Plain HTTP to a remote IP is not a secure context for microphone capture. |
| Connected but silent | Look for microphone levels on the phone, verify the selected desktop virtual input, and keep the browser foregrounded. |

Tailscale Serve with an HTTPS hostname is not required for the direct IP setup above. In the current implementation it does not by itself solve MicYou's hostname Origin restriction. Tailscale Funnel, an exit node, and public router forwarding are not required.

See [Tailscale installation](https://tailscale.com/docs/install) and [tailnet access controls](https://tailscale.com/docs/features/access-control) for VPN setup and policy management.

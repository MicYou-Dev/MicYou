# Web mode across private networks

MicYou Web mode can connect a phone browser to a desktop on another network using a private VPN or overlay. Both devices need a route to one another; network policies and the desktop firewall must allow the phone to reach the Web server's TCP port. A private overlay can do this without public router port forwarding.

## Connect using a private-network address

1. Connect the desktop and phone to the private network and authorize both devices as required by its administrator.
2. Start MicYou in **Web** mode and leave the server running. Note its listening port; the GUI's default Web port is `8443`. If the desktop has multiple interfaces, make sure MicYou listens on the VPN interface or all interfaces.
3. Find the desktop's VPN-assigned IP address. Use the desktop's address, not the phone's or the LAN address printed in the QR code. Choose an address assigned directly to the MicYou desktop when possible; routed aliases or proxy addresses may not pass the current WebSocket Origin check.
4. On the phone, open `https://<desktop-private-ip>:<web-port>/`. Bracket IPv6 addresses, for example `https://[fd00::1234]:8443/`.
5. MicYou uses a self-signed certificate. Verify the address belongs to your desktop before deciding whether to trust it in the browser. The browser must establish HTTPS successfully before microphone capture is available.
6. Tap the connection button and allow microphone access. On Windows, select **CABLE Output (VB-Audio Virtual Cable)** as the input in your call or game; MicYou feeds **CABLE Input**.

The desktop's Web listener does not itself authenticate individual users. Restrict network access to trusted devices with the VPN's access policy and the desktop firewall. Avoid publishing the microphone service to the public internet without appropriate authentication and access control.

## Examples

- **Tailscale:** Connect both devices to the same tailnet. Find the desktop's IPv4 address in Tailscale or run `tailscale ip -4` on the desktop. If its address is `100.101.102.103` and the listening port is `8443`, open `https://100.101.102.103:8443/` on the phone. [Installation](https://tailscale.com/docs/install) · [Access controls](https://tailscale.com/docs/features/access-control). This direct IP setup was tested with iPhone Safari and Windows.
- **ZeroTier:** Join both devices to the same authorized network, find the desktop's assigned network IP in ZeroTier Central or the client, and use that IP and the MicYou listening port in the URL. [ZeroTier setup](https://docs.zerotier.com/start/). This example has not been tested with MicYou here.
- **NetBird:** Connect the desktop and phone as peers, make sure access policies permit the connection, and use the desktop's peer IP and MicYou listening port. [Add peers](https://docs.netbird.io/manage/peers/add-machines-to-your-network) · [Access policies](https://docs.netbird.io/manage/access-control/manage-network-access). This example has not been tested with MicYou here.

For CLI testing, explicitly provide the Web port to avoid ambiguity:

```sh
cargo run -p micyou-cli -- serve --mode web --port 8443
```

The CLI and GUI share configuration and cannot run simultaneously. Close the GUI before running the CLI, and use the actual listening port reported by the server.

## Hostnames, proxies, and tunnels

The current WebSocket Origin check accepts `localhost` and IP addresses assigned to the desktop. A reverse proxy or tunneling service may load the page at a DNS hostname while the subsequent audio connection is rejected. Using a private IP assigned to the desktop avoids that hostname restriction where the network provides direct reachability. Do not disable Origin checks or use wildcard origins to work around it.

A tunnel that exposes a public URL has different security properties from a private overlay: its authentication, TLS, routing, and WebSocket support must be evaluated separately. A page loading successfully does not prove that its audio WebSocket is accepted. Private-network setup does not require a public URL, exit node, or public port forwarding.

## iPhone browser behavior

Keep Safari in the foreground and the phone unlocked while streaming. A mobile browser may suspend microphone capture when backgrounded or locked. The VPN can remain in the background. For background or locked-screen streaming, consider the [native iOS client](https://github.com/MicYou-Dev/MicYou-iOS) instead.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Page does not load | Confirm the VPN is connected on both devices, MicYou is listening in Web mode, the URL uses the desktop's private IP and actual port, and network policies and the desktop firewall allow that connection. |
| Page loads but audio cannot connect through a hostname or proxy | Check WebSocket forwarding and the Origin restriction above; try an IP assigned directly to the desktop. |
| Certificate warning or microphone unavailable | Confirm HTTPS, certificate trust, and browser microphone permissions. Plain HTTP to a remote IP is not a secure context for microphone capture. |
| Connected but silent | Check the phone's microphone level, the selected desktop virtual input, and whether the browser is foregrounded. |

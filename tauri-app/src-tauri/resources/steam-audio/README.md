# Steam Streaming audio driver (Windows)

MicYou uses Valve's Steam Streaming audio driver as its Windows virtual
microphone path instead of requiring a separate VB-CABLE download:

- `SteamStreamingMicrophone.{inf,sys,cat}` is the device MicYou needs. MicYou
  renders microphone audio into its **playback/render endpoint**, and calls,
  games and streaming apps capture from the same device's **recording/capture
  endpoint**.
- `SteamStreamingSpeakers.{inf,sys,cat}` is Valve's playback sibling, used by
  Steam Link for game audio. It is **not** the virtual microphone and must not
  be selected for microphone routing: its capture side never appears on the
  Microphone endpoint.

Valve's driver defaults are not usable as a microphone cable without
configuration: the Microphone render endpoint starts at 8 channels (the loopback
then decimates, turning 1 kHz into 125 Hz) and the capture endpoint starts at
+30 dB (which clips every recording app). `configure.ps1` sets the render
endpoint to mono 48 kHz 16-bit and the capture gain to 0 dB via
`IPolicyConfig::SetDeviceFormat` and `IAudioEndpointVolume`, then cycles
`AudioEndpointBuilder`.

The installer stages both packages with `pnputil /add-driver <inf> /install`
during setup (admin mode), and the onboarding flow can install or repair them
later through the same code path with a single UAC prompt.

## Provenance

These files are Valve Software's Steam Link audio drivers, mirrored from
<https://github.com/milo-src/SteamLinkAudioDriver> at commit
`3ec213b905526cd3c54b1a3389b7a08befb2bf97` (the `Steam VAC/drivers/Windows10/x64`
directory). Valve does not publish an official standalone download for them;
they normally arrive with the Steam client. The `.cat` files carry Valve's
WHQL signature and must stay next to the matching `.inf`/`.sys` files.

## SHA-256

```text
a767b5f76a5f9eddde5793b81370280adeddd4658e164798dcd451ede7e1a212  steamstreamingmicrophone.cat
996fe11be464a2be518e4cbab5c46fc7a250c6a67cf705a75ce3bb7b2f2611f4  SteamStreamingMicrophone.inf
0ec2de32da3afbc63ea84e76d1f0322382c273b3162885c7254f3960802b3b5e  SteamStreamingMicrophone.sys
97a03a1567e6913eb7972f96f98fb0284e41d10be1de13a846a1a529c2d4bfc1  steamstreamingspeakers.cat
40899fad06a5f0b23f202e96dec9fed3b7fa38979eeeb5f62613b172e927203b  SteamStreamingSpeakers.inf
ee1ee0d1346328da18d70988f01bd961437e95be5ab82059a28cbcc7d4b23239  SteamStreamingSpeakers.sys
```

Only the Windows x64 build uses this directory. It is copied into the Inno
Setup installer and into `%ProgramFiles%\MicYou\resources\steam-audio\x64`.

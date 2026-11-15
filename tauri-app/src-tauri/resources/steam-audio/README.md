# Steam Streaming audio driver (Windows)

MicYou uses Valve's Steam Streaming audio driver as its Windows virtual
microphone path instead of requiring a separate VB-CABLE download:

- `SteamStreamingSpeakers.{inf,sys,cat}` provides the playback endpoint
  ("Steam Streaming Speakers") that MicYou renders microphone audio into.
- `SteamStreamingMicrophone.{inf,sys,cat}` provides the recording endpoint
  ("Steam Streaming Microphone") that calls, games and streaming apps select as
  their microphone.

The installer stages both packages with `pnputil /add-driver <inf> /install`
during setup (admin mode), and the onboarding flow can install them later
through the same code path with a single UAC prompt.

## Provenance

These files are Valve Software's Steam Link audio drivers, mirrored from
<https://github.com/milo-src/SteamLinkAudioDriver> at commit
`3ec213b905526cd3c54b1a3389b7a08befb2bf97` (the `Steam VAC/drivers/Windows10/x64`
directory). Valve does not publish an official standalone download for them;
they normally arrive with the Steam client. The `.cat` files carry Valve's
signature and must stay next to the matching `.inf`/`.sys` files.

Only the Windows x64 build uses this directory. It is copied into the Inno
Setup installer and into `%ProgramFiles%\MicYou\resources\steam-audio\x64`.

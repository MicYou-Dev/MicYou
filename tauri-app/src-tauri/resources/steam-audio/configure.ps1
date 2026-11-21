# Configures Valve's Steam Streaming Microphone as a usable virtual microphone.
#
# Valve's driver ships with unusable defaults: the render endpoint starts
# multichannel, the capture endpoint is boosted by +30 dB, and the two loopback
# sides can end up at different formats (a 2.2.x default install produced a
# 48 kHz 16-bit render side against a 44.1 kHz 32-bit capture side). Rendering
# multichannel audio makes the driver decimate the signal (a 1 kHz tone arrives
# as 125 Hz); the capture boost clips everything a recording application
# receives; and when the render and capture endpoints disagree about sample
# rate or bit depth, the loopback resampler produces extremely loud artifacts
# (the "audio explosion" users hit after a default install). The endpoints also
# allow exclusive mode by default, so one application can reopen either side
# with its own format and break the pair.
#
# Configure a matched virtual cable on both endpoints: stereo 48 kHz 24-bit,
# unity capture gain, and exclusive mode disabled. MicYou renders into the
# playback endpoint and calls/games/streaming apps capture from the recording
# endpoint, so both sides must advertise the same format.
#
# Run elevated (the installer does). Idempotent: it only restarts the audio
# endpoint builder when something actually needs to change.

$ErrorActionPreference = 'Stop'

$TargetChannels = 2
$TargetRate = 48000
$TargetBits = 24
# SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT
$TargetChannelMask = 0x3
$ExclusiveAllowKey = '{b3f8fa53-0004-438e-9003-51a46e139bfc},3'
$ExclusivePriorityKey = '{b3f8fa53-0004-438e-9003-51a46e139bfc},4'
$FormatValueName = '{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0'

Add-Type @"
using System;
using System.Runtime.InteropServices;

[ComImport, Guid("870af99c-171d-4f9e-af0d-e63df40c2bc9")]
class PolicyConfigClient { }

[ComImport, Guid("f8679f50-850a-41cf-9c72-430f290290c8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IPolicyConfig {
    [PreserveSig] int GetMixFormat(IntPtr device, out IntPtr fmt);
    [PreserveSig] int GetDeviceFormat(IntPtr device, int defaultFormat, out IntPtr fmt);
    [PreserveSig] int ResetDeviceFormat(IntPtr device);
    [PreserveSig] int SetDeviceFormat(IntPtr device, IntPtr endFormat, IntPtr mixFormat);
    [PreserveSig] int GetProcessingPeriod(IntPtr device, int defaultPeriod, out long a, out long b);
    [PreserveSig] int SetProcessingPeriod(IntPtr device, ref long period);
    [PreserveSig] int GetShareMode(IntPtr device, IntPtr mode);
    [PreserveSig] int SetShareMode(IntPtr device, IntPtr mode);
    [PreserveSig] int GetPropertyValue(IntPtr device, IntPtr key, IntPtr value);
    [PreserveSig] int SetPropertyValue(IntPtr device, IntPtr key, IntPtr value);
    [PreserveSig] int SetDefaultEndpoint(IntPtr device, int role);
    [PreserveSig] int SetEndpointVisibility(IntPtr device, int visible);
}

[ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
class MMDeviceEnumeratorComObject { }

[ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IMMDeviceEnumerator {
    int EnumAudioEndpoints(int dataFlow, int stateMask, out IntPtr devices);
    int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice device);
    int GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IMMDevice device);
    int RegisterEndpointNotificationCallback(IntPtr client);
    int UnregisterEndpointNotificationCallback(IntPtr client);
}

[ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IMMDevice {
    int Activate(ref Guid iid, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object iface);
    int OpenPropertyStore(int access, out IPropertyStore props);
    int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
    int GetState(out int state);
}

[StructLayout(LayoutKind.Sequential)]
public struct PROPERTYKEY {
    public Guid fmtid;
    public uint pid;
}

// VT_UI4 PROPVARIANT. Size 24 matches the x64 layout (8-byte header plus the
// 16-byte union); the installer and MicYou are 64-bit only.
[StructLayout(LayoutKind.Explicit, Size = 24)]
public struct PROPVARIANT {
    [FieldOffset(0)] public ushort vt;
    [FieldOffset(2)] public ushort wReserved1;
    [FieldOffset(4)] public ushort wReserved2;
    [FieldOffset(6)] public ushort wReserved3;
    [FieldOffset(8)] public uint ulVal;
}

[ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IPropertyStore {
    [PreserveSig] int GetCount(out uint cProps);
    [PreserveSig] int GetAt(uint iProp, out PROPERTYKEY pkey);
    [PreserveSig] int GetValue(ref PROPERTYKEY key, out PROPVARIANT pv);
    [PreserveSig] int SetValue(ref PROPERTYKEY key, ref PROPVARIANT pv);
    [PreserveSig] int Commit();
}

[ComImport, Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IAudioEndpointVolume {
    int RegisterControlChangeNotify(IntPtr n);
    int UnregisterControlChangeNotify(IntPtr n);
    int GetChannelCount(out int c);
    int SetMasterVolumeLevel(float dB, IntPtr ctx);
    int SetMasterVolumeLevelScalar(float s, IntPtr ctx);
    int GetMasterVolumeLevel(out float dB);
}

[StructLayout(LayoutKind.Sequential, Pack = 2)]
public struct WAVEFORMATEX {
    public ushort wFormatTag;
    public ushort nChannels;
    public uint nSamplesPerSec;
    public uint nAvgBytesPerSec;
    public ushort nBlockAlign;
    public ushort wBitsPerSample;
    public ushort cbSize;
}

[StructLayout(LayoutKind.Sequential, Pack = 2)]
public struct WAVEFORMATEXTENSIBLE {
    public WAVEFORMATEX Format;
    public ushort wValidBitsPerSample;
    public uint dwChannelMask;
    public Guid SubFormat;
}

public static class SteamAudioConfig {
    public static int SetFormat(string deviceId, ushort channels, uint rate, ushort bits, uint mask) {
        var policy = (IPolicyConfig)new PolicyConfigClient();
        var fmt = new WAVEFORMATEXTENSIBLE();
        fmt.Format.wFormatTag = 0xFFFE;
        fmt.Format.nChannels = channels;
        fmt.Format.nSamplesPerSec = rate;
        fmt.Format.wBitsPerSample = bits;
        fmt.Format.nBlockAlign = (ushort)(channels * bits / 8);
        fmt.Format.nAvgBytesPerSec = rate * fmt.Format.nBlockAlign;
        fmt.Format.cbSize = 22;
        fmt.wValidBitsPerSample = bits;
        fmt.dwChannelMask = mask;
        fmt.SubFormat = new Guid("00000001-0000-0010-8000-00aa00389b71");
        IntPtr p = Marshal.AllocHGlobal(Marshal.SizeOf(fmt));
        try {
            Marshal.StructureToPtr(fmt, p, false);
            IntPtr id = Marshal.StringToCoTaskMemUni(deviceId);
            try {
                return policy.SetDeviceFormat(id, p, p);
            } finally {
                Marshal.FreeCoTaskMem(id);
            }
        } finally {
            Marshal.FreeHGlobal(p);
        }
    }

    public static float SetVolume(string deviceId, float db) {
        var enumerator = (IMMDeviceEnumerator)new MMDeviceEnumeratorComObject();
        IMMDevice dev;
        int hr = enumerator.GetDevice(deviceId, out dev);
        if (hr != 0) throw new Exception("GetDevice hr=" + hr);
        Guid iid = typeof(IAudioEndpointVolume).GUID;
        object o;
        hr = dev.Activate(ref iid, 1, IntPtr.Zero, out o);
        if (hr != 0) throw new Exception("Activate hr=" + hr);
        var vol = (IAudioEndpointVolume)o;
        float before;
        vol.GetMasterVolumeLevel(out before);
        hr = vol.SetMasterVolumeLevel(db, IntPtr.Zero);
        if (hr != 0) throw new Exception("SetMasterVolumeLevel hr=" + hr);
        return before;
    }

    // Writes the two exclusive-mode endpoint properties through the endpoint
    // property store. Going through the audio service (instead of writing
    // HKLM directly) updates the live policy as well as the persisted value.
    public static int SetExclusiveMode(string deviceId, uint allow, uint priority) {
        var enumerator = (IMMDeviceEnumerator)new MMDeviceEnumeratorComObject();
        IMMDevice dev;
        int hr = enumerator.GetDevice(deviceId, out dev);
        if (hr != 0) return hr;
        IPropertyStore store;
        hr = dev.OpenPropertyStore(2, out store); // STGM_READWRITE
        if (hr != 0) return hr;
        var fmtid = new Guid("b3f8fa53-0004-438e-9003-51a46e139bfc");
        for (int i = 0; i < 2; i++) {
            var key = new PROPERTYKEY();
            key.fmtid = fmtid;
            key.pid = (uint)(3 + i);
            var pv = new PROPVARIANT();
            pv.vt = 19; // VT_UI4
            pv.ulVal = i == 0 ? allow : priority;
            hr = store.SetValue(ref key, ref pv);
            if (hr != 0) return hr;
        }
        return store.Commit();
    }
}
"@

function Get-SteamEndpoint {
    param([string]$Flow, [string]$Prefix)
    $base = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$Flow"
    $match = Get-ChildItem $base -ErrorAction SilentlyContinue | Where-Object {
        $props = Get-ItemProperty "$($_.PSPath)\Properties" -ErrorAction SilentlyContinue
        $props.'{b3f8fa53-0004-438e-9003-51a46e139bfc},6' -eq 'Steam Streaming Microphone'
    } | Select-Object -First 1
    if (-not $match) { return $null }
    [pscustomobject]@{
        Flow = $Flow
        Id = "$Prefix$($match.PSChildName)"
        PropertiesPath = "$($match.PSPath)\Properties"
    }
}

# The endpoint format is a WAVEFORMATEXTENSIBLE blob stored after the
# 8-byte PROPVARIANT header: nChannels at 10, nSamplesPerSec at 12 and
# wBitsPerSample at 22.
function Get-EndpointFormat {
    param([string]$PropertiesPath)
    $props = Get-ItemProperty $PropertiesPath -ErrorAction SilentlyContinue
    if ($null -eq $props) { return $null }
    $blob = $props.$FormatValueName
    if ($blob -isnot [byte[]] -or $blob.Length -lt 28) { return $null }
    [pscustomobject]@{
        Channels = [BitConverter]::ToUInt16($blob, 10)
        Rate = [BitConverter]::ToUInt32($blob, 12)
        Bits = [BitConverter]::ToUInt16($blob, 22)
    }
}

function Test-TargetFormat {
    param($Format)
    return (
        $null -ne $Format -and
        $Format.Channels -eq $TargetChannels -and
        $Format.Rate -eq $TargetRate -and
        $Format.Bits -eq $TargetBits
    )
}

function Format-Description {
    param($Format)
    if ($null -eq $Format) { return 'missing' }
    return ("{0} ch, {1} Hz, {2}-bit" -f $Format.Channels, $Format.Rate, $Format.Bits)
}

function Test-ExclusiveAllowed {
    param([string]$PropertiesPath)
    $props = Get-ItemProperty $PropertiesPath -ErrorAction SilentlyContinue
    if ($null -eq $props) { return $true }
    # Missing values mean the Windows default, which is "allowed".
    return (($props.$ExclusiveAllowKey -ne 0) -or ($props.$ExclusivePriorityKey -ne 0))
}

$render = $null
$capture = $null
$deadline = (Get-Date).AddSeconds(20)
do {
    $render = Get-SteamEndpoint -Flow 'Render' -Prefix '{0.0.0.00000000}.'
    $capture = Get-SteamEndpoint -Flow 'Capture' -Prefix '{0.0.1.00000000}.'
    if ($render -and $capture) { break }
    Start-Sleep -Seconds 1
} while ((Get-Date) -lt $deadline)

if (-not $render -or -not $capture) {
    Write-Output 'Steam Streaming Microphone endpoints not found; nothing to configure.'
    exit 1
}

$needFormat = -not (Test-TargetFormat (Get-EndpointFormat $render.PropertiesPath)) -or
    -not (Test-TargetFormat (Get-EndpointFormat $capture.PropertiesPath))

if ($needFormat) {
    Write-Output 'Applying stereo 24-bit 48 kHz to both Steam Streaming Microphone endpoints.'
    foreach ($endpoint in @($render, $capture)) {
        # Setting an endpoint to the format it already has returns
        # E_ACCESSDENIED, so only touch the side that actually differs.
        if (Test-TargetFormat (Get-EndpointFormat $endpoint.PropertiesPath)) {
            continue
        }
        $hr = [SteamAudioConfig]::SetFormat($endpoint.Id, $TargetChannels, $TargetRate, $TargetBits, $TargetChannelMask)
        if ($hr -ne 0) {
            Write-Output ("SetDeviceFormat for the {0} endpoint failed: HRESULT=0x{1:X8}" -f $endpoint.Flow, $hr)
            exit 1
        }
    }

    # The audio endpoint builder owns the live endpoint format; cycling it
    # makes the new format visible to WASAPI/CPAL without a reboot.
    Restart-Service AudioEndpointBuilder -Force
    # Cycling AudioEndpointBuilder stops dependent services; make sure Windows
    # Audio comes back so the machine is not left without audio.
    Start-Service Audiosrv -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 3

    # The endpoints are recreated asynchronously; re-resolve them and wait for
    # the new format to become visible before touching anything else.
    $deadline = (Get-Date).AddSeconds(20)
    do {
        $render = Get-SteamEndpoint -Flow 'Render' -Prefix '{0.0.0.00000000}.'
        $capture = Get-SteamEndpoint -Flow 'Capture' -Prefix '{0.0.1.00000000}.'
        if ($render -and $capture -and
            (Test-TargetFormat (Get-EndpointFormat $render.PropertiesPath)) -and
            (Test-TargetFormat (Get-EndpointFormat $capture.PropertiesPath))) {
            break
        }
        Start-Sleep -Seconds 1
    } while ((Get-Date) -lt $deadline)
}

if (-not $render -or -not $capture) {
    Write-Output 'Steam Streaming Microphone endpoints disappeared after reconfiguration.'
    exit 1
}

$needExclusive = (Test-ExclusiveAllowed $render.PropertiesPath) -or
    (Test-ExclusiveAllowed $capture.PropertiesPath)

# The endpoint property store is written through the audio service, so this
# takes effect for new streams without another service restart. Values 3 and 4
# are "Allow applications to take exclusive control of this device" and "Give
# exclusive mode applications priority"; 0 disables each.
if ($needExclusive) {
    foreach ($endpoint in @($render, $capture)) {
        $hr = [SteamAudioConfig]::SetExclusiveMode($endpoint.Id, 0, 0)
        if ($hr -ne 0) {
            Write-Output ("Disabling exclusive mode for the {0} endpoint failed: HRESULT=0x{1:X8}" -f $endpoint.Flow, $hr)
            exit 1
        }
    }
    Write-Output 'Disabled exclusive mode on both Steam Streaming Microphone endpoints.'
}

$errors = @()
$renderFormat = Get-EndpointFormat $render.PropertiesPath
$captureFormat = Get-EndpointFormat $capture.PropertiesPath
if (-not (Test-TargetFormat $renderFormat)) {
    $errors += ("Render endpoint format is {0}, expected {1} ch, {2} Hz, {3}-bit." -f (Format-Description $renderFormat), $TargetChannels, $TargetRate, $TargetBits)
}
if (-not (Test-TargetFormat $captureFormat)) {
    $errors += ("Capture endpoint format is {0}, expected {1} ch, {2} Hz, {3}-bit." -f (Format-Description $captureFormat), $TargetChannels, $TargetRate, $TargetBits)
}
if (Test-ExclusiveAllowed $render.PropertiesPath) {
    $errors += 'Render endpoint still allows exclusive mode.'
}
if (Test-ExclusiveAllowed $capture.PropertiesPath) {
    $errors += 'Capture endpoint still allows exclusive mode.'
}
if ($errors.Count -gt 0) {
    $errors | ForEach-Object { Write-Output $_ }
    exit 1
}

$before = [SteamAudioConfig]::SetVolume($capture.Id, 0.0)
Write-Output ("Steam Streaming Microphone configured: stereo 24-bit 48 kHz on both endpoints, exclusive mode disabled, capture gain {0:N1} dB -> 0.0 dB." -f $before)
exit 0

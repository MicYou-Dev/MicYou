# Configures Valve's Steam Streaming Microphone as a usable virtual microphone.
#
# The driver's render endpoint defaults to 8 channels and its capture endpoint
# defaults to +30 dB. Rendering 8-channel audio makes the driver's loopback
# decimate the signal (a 1 kHz tone arrives as 125 Hz), and the +30 dB capture
# gain clips everything a normal application records. Set the render endpoint to
# mono 48 kHz 16-bit and the capture endpoint to 0 dB so the pair behaves like a
# unity-gain virtual cable.
#
# Run elevated (the installer does). Idempotent: it only restarts the audio
# endpoint builder when the format actually needs to change.

$ErrorActionPreference = 'Stop'

Add-Type @"
using System;
using System.Runtime.InteropServices;

[ComImport, Guid("870af99c-171d-4f9e-af0d-e63df40c2bc9")]
class PolicyConfigClient { }

[ComImport, Guid("f8679f50-850a-41cf-9c72-430f290290c8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IPolicyConfig {
    int GetMixFormat(IntPtr device, out IntPtr fmt);
    int GetDeviceFormat(IntPtr device, int defaultFormat, out IntPtr fmt);
    int ResetDeviceFormat(IntPtr device);
    int SetDeviceFormat(IntPtr device, IntPtr endFormat, IntPtr mixFormat);
    int GetProcessingPeriod(IntPtr device, int defaultPeriod, out long a, out long b);
    int SetProcessingPeriod(IntPtr device, ref long period);
    int GetShareMode(IntPtr device, IntPtr mode);
    int SetShareMode(IntPtr device, IntPtr mode);
    int GetPropertyValue(IntPtr device, IntPtr key, IntPtr value);
    int SetPropertyValue(IntPtr device, IntPtr key, IntPtr value);
    int SetDefaultEndpoint(IntPtr device, int role);
    int SetEndpointVisibility(IntPtr device, int visible);
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
    int OpenPropertyStore(int access, out IntPtr props);
    int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
    int GetState(out int state);
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
}
"@

function Get-SteamEndpointId {
    param([string]$Flow, [string]$Prefix)
    $base = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$Flow"
    $match = Get-ChildItem $base -ErrorAction SilentlyContinue | Where-Object {
        $props = Get-ItemProperty "$($_.PSPath)\Properties" -ErrorAction SilentlyContinue
        $props.'{b3f8fa53-0004-438e-9003-51a46e139bfc},6' -eq 'Steam Streaming Microphone'
    } | Select-Object -First 1
    if (-not $match) { return $null }
    return "$Prefix$($match.PSChildName)"
}

$renderId = $null
$captureId = $null
$deadline = (Get-Date).AddSeconds(20)
do {
    $renderId = Get-SteamEndpointId -Flow 'Render' -Prefix '{0.0.0.00000000}.'
    $captureId = Get-SteamEndpointId -Flow 'Capture' -Prefix '{0.0.1.00000000}.'
    if ($renderId -and $captureId) { break }
    Start-Sleep -Seconds 1
} while ((Get-Date) -lt $deadline)

if (-not $renderId -or -not $captureId) {
    Write-Output 'Steam Streaming Microphone endpoints not found; nothing to configure.'
    exit 1
}

# Current render endpoint format, stored after an 8-byte PROPVARIANT header.
$renderGuid = $renderId -replace '^.*\.\{', '' -replace '\}$', ''
$props = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{$renderGuid}\Properties" -ErrorAction SilentlyContinue
$blob = $props.'{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0'
$needFormat = $true
if ($blob -is [byte[]] -and $blob.Length -ge 28) {
    $ch = [BitConverter]::ToUInt16($blob, 10)
    $rate = [BitConverter]::ToUInt32($blob, 12)
    $bits = [BitConverter]::ToUInt16($blob, 22)
    if ($ch -eq 1 -and $rate -eq 48000 -and $bits -eq 16) { $needFormat = $false }
}

if ($needFormat) {
    $hr = [SteamAudioConfig]::SetFormat($renderId, 1, 48000, 16, 0x4)
    if ($hr -ne 0) {
        Write-Output ("SetDeviceFormat failed: HRESULT=0x{0:X8}" -f $hr)
        exit 1
    }
    # The audio endpoint builder owns the live endpoint format; cycling it makes
    # the new mono format visible to WASAPI/CPAL without a reboot.
    Restart-Service AudioEndpointBuilder -Force
    Start-Sleep -Seconds 3
    # The endpoint may be recreated with the same GUID, but re-resolve anyway.
    $captureId = Get-SteamEndpointId -Flow 'Capture' -Prefix '{0.0.1.00000000}.'
    if (-not $captureId) {
        Write-Output 'Steam Streaming Microphone capture endpoint disappeared after reconfiguration.'
        exit 1
    }
}

$before = [SteamAudioConfig]::SetVolume($captureId, 0.0)
Write-Output ("Steam Streaming Microphone configured (capture gain {0:N1} dB -> 0.0 dB)." -f $before)
exit 0

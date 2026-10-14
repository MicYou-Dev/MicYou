import type { Composer } from 'vue-i18n';

/** 帮助文档链接（按错误类型针对性跳转） */
const QUICK_START_URL = 'https://micyou.top/docs/quick-start';
const FAQ_URL = 'https://micyou.top/docs/faq';

export type ConnectionErrorType =
  | 'NetworkTimeout'
  | 'NetworkUnreachable'
  | 'PortInUse'
  | 'ConnectionRefused'
  | 'PermissionDenied'
  | 'FirewallBlocked'
  | 'AdminPrivilegeRequired'
  | 'DeviceNotFound'
  | 'UsbConnectionFailed'
  | 'AdbCommandFailed'
  | 'HandshakeFailed'
  | 'ProtocolError'
  | 'VersionMismatch'
  | 'UdpPortBlocked'
  | 'AudioDeviceError'
  | 'AudioFormatError'
  | 'UnknownError';

export interface ConnectionErrorDetails {
  type: ConnectionErrorType;
  originalMessage: string;
  title: string;
  message: string;
  suggestions: string[];
  showRetry: boolean;
  showHelp: boolean;
  helpUrl: string | null;
}

export function analyzeError(errorMessage: string): ConnectionErrorType {
  const msg = errorMessage.toLowerCase();

  if (msg.includes('timeout')) return 'NetworkTimeout';
  if (
    msg.includes('bind') ||
    msg.includes('port is already in use') ||
    msg.includes('address already in use')
  )
    return 'PortInUse';
  if (msg.includes('connection refused') || msg.includes('refused')) return 'ConnectionRefused';
  if (
    msg.includes('unreachable') ||
    msg.includes('no route to host') ||
    msg.includes('network is unreachable')
  )
    return 'NetworkUnreachable';
  if (msg.includes('firewall') || msg.includes('blocked')) return 'FirewallBlocked';
  if (msg.includes('permission') || msg.includes('access denied') || msg.includes('privilege'))
    return 'PermissionDenied';
  if (msg.includes('adb')) return 'AdbCommandFailed';
  if (msg.includes('usb')) return 'UsbConnectionFailed';
  if (msg.includes('handshake') || msg.includes('握手')) return 'HandshakeFailed';
  if (msg.includes('audio')) return 'AudioDeviceError';
  if (msg.includes('udp')) return 'UdpPortBlocked';

  return 'UnknownError';
}

function extractAdbCommand(message: string): string | null {
  for (const delimiter of ['：', ':']) {
    const idx = message.indexOf(delimiter);
    if (idx !== -1) {
      const after = message.substring(idx + 1).trim();
      if (after) return after;
    }
  }
  return null;
}

export function generateErrorDetails(
  type: ConnectionErrorType,
  originalMessage: string,
  mode: string,
  port: number | undefined,
  ip: string | undefined,
  t: Composer['t'],
): ConnectionErrorDetails {
  const base = {
    type,
    originalMessage,
    showRetry: true,
    showHelp: false,
    helpUrl: null as string | null,
  };

  switch (type) {
    case 'NetworkTimeout':
      return {
        ...base,
        title: t('error.networkTimeout.title'),
        message: t('error.networkTimeout.message'),
        suggestions: [
          t('error.suggestion.checkNetwork'),
          t('error.suggestion.checkTargetRunning'),
          t('error.suggestion.tryDifferentPort'),
        ],
      };

    case 'PortInUse':
      return {
        ...base,
        title: t('error.portInUse.title'),
        message: t('error.portInUse.message', { port: String(port ?? 8554) }),
        suggestions: [t('error.suggestion.changePort'), t('error.suggestion.checkOtherApps')],
      };

    case 'ConnectionRefused':
      return {
        ...base,
        title: t('error.connectionRefused.title'),
        message:
          mode === 'wifi'
            ? t('error.connectionRefused.wifiMessage', { ip: ip ?? '' })
            : t('error.connectionRefused.message'),
        suggestions: [
          t('error.suggestion.checkServerRunning'),
          t('error.suggestion.checkServerConfig'),
        ],
      };

    case 'NetworkUnreachable':
      return {
        ...base,
        title: t('error.networkUnreachable.title'),
        message: t('error.networkUnreachable.message', { ip: ip ?? '' }),
        suggestions: [
          t('error.suggestion.checkNetworkConnection'),
          t('error.suggestion.verifyIpAddress'),
          t('error.suggestion.checkWifiConnected'),
        ],
      };

    case 'FirewallBlocked':
      return {
        ...base,
        title: t('error.firewallBlocked.title'),
        message: t('error.firewallBlocked.message', { port: String(port ?? 8554) }),
        suggestions: [t('error.suggestion.addFirewallRule'), t('error.suggestion.runAsAdmin')],
        showHelp: true,
        helpUrl: FAQ_URL,
      };

    case 'PermissionDenied':
      return {
        ...base,
        title: t('error.permissionDenied.title'),
        message: t('error.permissionDenied.message'),
        suggestions: [t('error.suggestion.runAsAdmin'), t('error.suggestion.checkAntivirus')],
      };

    case 'DeviceNotFound':
      return {
        ...base,
        title: t('error.deviceNotFound.title'),
        message: t('error.deviceNotFound.message'),
        suggestions: [t('error.suggestion.checkNetworkConnection')],
      };

    case 'UsbConnectionFailed':
      return {
        ...base,
        title: t('error.usbConnectionFailed.title'),
        message: t('error.usbConnectionFailed.message'),
        suggestions: [
          t('error.suggestion.checkUsbCable'),
          t('error.suggestion.enableUsbDebugging'),
          t('error.suggestion.connectIosOverUsb'),
          ...(extractAdbCommand(originalMessage)
            ? [t('error.suggestion.runAdbCommand', { cmd: extractAdbCommand(originalMessage)! })]
            : []),
        ],
        showHelp: true,
        helpUrl: FAQ_URL,
      };

    case 'AdbCommandFailed':
      return {
        ...base,
        title: t('error.adbCommandFailed.title'),
        message: t('error.adbCommandFailed.message'),
        suggestions: [
          t('error.suggestion.checkAdbInstalled'),
          // Without adb only an iPhone can use USB mode, so point there too
          ...(originalMessage.toLowerCase().includes('not found')
            ? [t('error.suggestion.connectIosOverUsb')]
            : []),
          ...(extractAdbCommand(originalMessage)
            ? [t('error.suggestion.runAdbManually', { cmd: extractAdbCommand(originalMessage)! })]
            : []),
        ],
        showHelp: true,
        helpUrl: QUICK_START_URL,
      };

    case 'HandshakeFailed':
      return {
        ...base,
        title: t('error.handshakeFailed.title'),
        message: t('error.handshakeFailed.message'),
        suggestions: [t('error.suggestion.versionMatch'), t('error.suggestion.restartApp')],
      };

    case 'ProtocolError':
      return {
        ...base,
        title: t('error.protocolError.title'),
        message: t('error.protocolError.message'),
        suggestions: [t('error.suggestion.restartApp'), t('error.suggestion.checkVersion')],
      };

    case 'AudioDeviceError':
      return {
        ...base,
        title: t('error.audioDevice.title'),
        message: t('error.audioDevice.message'),
        suggestions: [t('error.suggestion.checkAudioDevice'), t('error.suggestion.restartApp')],
      };

    case 'AudioFormatError':
      return {
        ...base,
        title: t('error.audioFormat.title'),
        message: t('error.audioFormat.message'),
        suggestions: [
          t('error.suggestion.changeAudioConfig'),
          t('error.suggestion.useDefaultConfig'),
        ],
      };

    case 'VersionMismatch':
      return {
        ...base,
        title: t('error.versionMismatch.title'),
        message: t('error.versionMismatch.message'),
        suggestions: [t('error.suggestion.updateApp'), t('error.suggestion.checkVersion')],
        showHelp: true,
        helpUrl: QUICK_START_URL,
      };

    case 'AdminPrivilegeRequired':
      return {
        ...base,
        title: t('error.adminPrivilege.title'),
        message: t('error.adminPrivilege.message'),
        suggestions: [t('error.suggestion.runAsAdmin')],
      };

    case 'UdpPortBlocked':
      return {
        ...base,
        title: t('error.udpPortBlocked.title'),
        message: t('error.udpPortBlocked.message', { port: port ? port + 1 : 6001 }),
        suggestions: [t('error.suggestion.addFirewallRule'), t('error.suggestion.runAsAdmin')],
        showHelp: true,
        helpUrl: FAQ_URL,
      };

    case 'UnknownError':
    default:
      return {
        ...base,
        title: t('error.unknown.title'),
        message: t('error.unknown.message', { error: originalMessage }),
        suggestions: [t('error.suggestion.restartApp'), t('error.suggestion.checkLogs')],
        showHelp: true,
        helpUrl: FAQ_URL,
      };
  }
}

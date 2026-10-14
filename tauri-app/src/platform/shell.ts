import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { open, type DialogFilter } from '@tauri-apps/plugin-dialog';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { openUrl as openerOpenUrl } from '@tauri-apps/plugin-opener';

/** Opens a URL in the system browser. */
export function openUrl(url: string): Promise<void> {
  return openerOpenUrl(url);
}

/** Shows a desktop notification, asking for permission the first time. */
export async function notify(body: string, title = 'MicYou'): Promise<void> {
  if (!(await isPermissionGranted())) await requestPermission();
  sendNotification({ title, body });
}

/** Asks the user for one file; null when the dialog is cancelled. */
export async function pickFile(filters: DialogFilter[]): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, filters });
  return picked ? String(picked) : null;
}

export const autostart = {
  isEnabled,
  enable,
  disable,
};

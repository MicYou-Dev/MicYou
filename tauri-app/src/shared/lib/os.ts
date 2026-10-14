const nav = typeof navigator !== 'undefined' ? navigator : undefined;

export const isMacOS = !!nav &&
  /Mac/.test(nav.platform || nav.userAgent) &&
  !/iPhone|iPad|iPod/.test(nav.userAgent) &&
  !(nav.maxTouchPoints && nav.maxTouchPoints > 2);

export const isLinux = !!nav && /Linux/.test(nav.userAgent) && !/Android/.test(nav.userAgent);

export const isWindows = !!nav && /Win/.test(nav.platform || nav.userAgent);

/** Root classes that platform-specific glass styles hang off. */
export function applyPlatformClasses() {
  const root = document.documentElement;
  root.classList.toggle('platform-macos', isMacOS);
  root.classList.toggle('platform-linux', isLinux);
}

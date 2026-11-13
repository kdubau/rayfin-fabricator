function platformName(): string {
  return (
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
    navigator.platform ??
    ''
  )
}

/** The app is running on macOS, where shortcuts use ⌘ rather than Ctrl. */
export function isMac(): boolean {
  if (typeof navigator === 'undefined') return false
  return /mac|iphone|ipad/i.test(platformName()) || /Macintosh/.test(navigator.userAgent)
}

/** The app is running on Windows. */
export function isWindows(): boolean {
  if (typeof navigator === 'undefined') return false
  return /^win/i.test(platformName()) || /Windows NT/.test(navigator.userAgent)
}

/**
 * The platform whose main window has no native title bar, so the app's own top
 * bar reaches the window's top edge and keeps clear of the window controls
 * (see `titlebar/`). `null` where the native title bar stays.
 */
export function customTitlebar(): 'mac' | 'windows' | null {
  return isMac() ? 'mac' : isWindows() ? 'windows' : null
}

/** The modifier for app shortcuts as it is printed on keycaps: `⌘` or `Ctrl`. */
export function modKey(): string {
  return isMac() ? '⌘' : 'Ctrl'
}

/** A shortcut as the app writes it in tooltips: `Ctrl+K`, or `⌘K` on macOS. */
export function shortcut(key: string): string {
  return isMac() ? `⌘${key}` : `Ctrl+${key}`
}

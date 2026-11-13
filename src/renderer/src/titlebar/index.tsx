import { useLayoutEffect, type JSX } from 'react'
import { customTitlebar } from '../platform'
import { useMacTitlebar } from './mac'
import './titlebar.css'

export { WindowControls } from './WindowControls'

/**
 * The main window's own title bar. On macOS and Windows the native title bar is
 * gone, so the first `--titlebar-h` row of whichever bar is on top is the
 * window's handle, and `<html data-titlebar="mac|windows">` has that bar keep
 * clear of the window controls (titlebar.css): the native traffic lights on
 * macOS (mac.ts), and the app's own caption buttons on Windows
 * (WindowControls.tsx).
 */
export function useTitlebar(uiScale: number | undefined): void {
  useLayoutEffect(() => {
    const kind = customTitlebar()
    if (!kind) return
    const root = document.documentElement
    root.dataset.titlebar = kind
    return () => {
      delete root.dataset.titlebar
    }
  }, [])
  useMacTitlebar(uiScale)
}

/** `data-tauri-drag-region` for a bar that stands in for the native title bar. */
export function dragRegion(): '' | undefined {
  return customTitlebar() ? '' : undefined
}

/** Moves the window from its top edge on screens without an app bar. */
export function StartupDragRegion({
  blocking = false
}: {
  blocking?: boolean
}): JSX.Element | null {
  return customTitlebar() ? (
    <div
      className={`titlebar-startup-drag${blocking ? ' titlebar-startup-drag--blocking' : ''}`}
      data-tauri-drag-region
      aria-hidden="true"
    />
  ) : null
}

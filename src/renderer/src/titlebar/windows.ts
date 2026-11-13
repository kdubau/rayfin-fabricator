import { useEffect, useState, type RefObject } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { MaximizeButtonState } from '@shared/ipc'
import { measurePreviewBounds, watchPreviewPixelRatio } from '../previewBounds'

export interface WindowFrame {
  maximized: boolean
  focused: boolean
}

/**
 * Whether the main window is maximized (Maximize or Restore) and focused
 * (Windows dims the caption buttons of an inactive window). Window-level
 * focus, so focus moving into the native preview still counts as focused.
 */
export function useWindowFrame(): WindowFrame {
  const [frame, setFrame] = useState<WindowFrame>({ maximized: false, focused: true })
  useEffect(() => {
    const win = getCurrentWindow()
    let alive = true
    const update = (patch: Partial<WindowFrame>): void => {
      if (alive) setFrame((frame) => ({ ...frame, ...patch }))
    }
    const syncMaximized = (): void => {
      win.isMaximized().then(
        (maximized) => update({ maximized }),
        (error: unknown) => console.error('Could not read the window state:', error)
      )
    }
    syncMaximized()
    win.isFocused().then(
      (focused) => update({ focused }),
      () => {}
    )
    const listeners = [
      win.onResized(syncMaximized),
      win.onFocusChanged(({ payload }) => update({ focused: payload }))
    ]
    return () => {
      alive = false
      for (const listener of listeners) {
        void listener.then((unlisten) => unlisten()).catch(() => {})
      }
    }
  }, [])
  return frame
}

const RELEASED: MaximizeButtonState = { hover: false, pressed: false }

/**
 * Windows 11 offers its Snap Layouts flyout only over a native Maximize button,
 * so a native overlay (src-tauri/src/titlebar/windows.rs) sits over `button`.
 * It takes the pointer from the page there, so the button's hover and press
 * come back from it. `uiScale` moves the button without resizing the window.
 */
export function useSnapOverlay(
  button: RefObject<HTMLElement | null>,
  uiScale: number | undefined
): MaximizeButtonState {
  const [state, setState] = useState(RELEASED)
  useEffect(() => {
    const element = button.current
    if (!element) return
    let frame = 0
    const place = (): void => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        const bounds = measurePreviewBounds(element)
        if (!bounds) return
        window.api.placeSnapOverlay(bounds).catch((error: unknown) => {
          console.error('Could not place the Snap Layouts overlay:', error)
        })
      })
    }
    place()
    const resized = new ResizeObserver(place)
    resized.observe(element)
    window.addEventListener('resize', place)
    const unwatch = watchPreviewPixelRatio(place)
    const unsubscribe = window.api.onMaximizeButton(setState)
    return () => {
      cancelAnimationFrame(frame)
      resized.disconnect()
      window.removeEventListener('resize', place)
      unwatch()
      unsubscribe()
      setState(RELEASED)
      window.api.placeSnapOverlay(null).catch(() => {})
    }
  }, [button, uiScale])
  return state
}

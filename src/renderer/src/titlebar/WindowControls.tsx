import { useRef, type JSX } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { customTitlebar } from '../platform'
import { useSnapOverlay, useWindowFrame } from './windows'

/** Segoe Fluent Icons (Segoe MDL2 Assets on Windows 10) caption glyphs. */
const GLYPH = { minimize: '\uE921', maximize: '\uE922', restore: '\uE923', close: '\uE8BB' }

function act(action: () => Promise<void>, what: string): void {
  action().catch((error: unknown) => console.error(`Could not ${what} the window:`, error))
}

/**
 * Windows caption buttons, where the main window has no native title bar.
 * They sit at the top right on every screen, above everything as native
 * buttons would, and the bar on top keeps `--titlebar-inset-end` clear for them.
 * Like native ones they are not in the tab order. Maximize also offers Windows
 * 11's Snap Layouts, through a native overlay (useSnapOverlay).
 */
export function WindowControls({ uiScale }: { uiScale?: number }): JSX.Element | null {
  return customTitlebar() === 'windows' ? <CaptionButtons uiScale={uiScale} /> : null
}

function CaptionButtons({ uiScale }: { uiScale?: number }): JSX.Element {
  const { maximized, focused } = useWindowFrame()
  const maximize = useRef<HTMLButtonElement>(null)
  const pointer = useSnapOverlay(maximize, uiScale)
  const win = getCurrentWindow()
  const resize = maximized ? 'Restore' : 'Maximize'
  return (
    <div
      className={`caption-buttons${focused ? '' : ' caption-buttons--inactive'}`}
      role="group"
      aria-label="Window"
    >
      <button
        type="button"
        className="caption-button"
        tabIndex={-1}
        aria-label="Minimize"
        title="Minimize"
        onClick={() => act(() => win.minimize(), 'minimize')}
      >
        <span aria-hidden="true">{GLYPH.minimize}</span>
      </button>
      <button
        ref={maximize}
        type="button"
        className={`caption-button${pointer.hover ? ' is-hover' : ''}${pointer.pressed ? ' is-pressed' : ''}`}
        tabIndex={-1}
        aria-label={resize}
        title={resize}
        // The overlay maximizes on a pointer click; this serves assistive tech.
        onClick={() => act(() => win.toggleMaximize(), resize.toLowerCase())}
      >
        <span aria-hidden="true">{maximized ? GLYPH.restore : GLYPH.maximize}</span>
      </button>
      <button
        type="button"
        className="caption-button caption-button--close"
        tabIndex={-1}
        aria-label="Close"
        title="Close"
        onClick={() => act(() => win.close(), 'close')}
      >
        <span aria-hidden="true">{GLYPH.close}</span>
      </button>
    </div>
  )
}

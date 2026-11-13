import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { MaximizeButtonState } from '@shared/ipc'
import * as platform from '../platform'
import { WindowControls } from './WindowControls'

const win = vi.hoisted(() => {
  const listeners: { resized?: () => void; focus?: (e: { payload: boolean }) => void } = {}
  return {
    listeners,
    maximized: false,
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined)
  }
})

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    isMaximized: () => Promise.resolve(win.maximized),
    isFocused: () => Promise.resolve(true),
    onResized: (cb: () => void) => {
      win.listeners.resized = cb
      return Promise.resolve(() => {})
    },
    onFocusChanged: (cb: (e: { payload: boolean }) => void) => {
      win.listeners.focus = cb
      return Promise.resolve(() => {})
    },
    minimize: win.minimize,
    toggleMaximize: win.toggleMaximize,
    close: win.close
  })
}))

let pointer: (state: MaximizeButtonState) => void = () => {}
const api = {
  placeSnapOverlay: vi.fn().mockResolvedValue(undefined),
  onMaximizeButton: vi.fn((cb: (state: MaximizeButtonState) => void) => {
    pointer = cb
    return () => {}
  })
}

beforeEach(() => {
  vi.spyOn(platform, 'customTitlebar').mockReturnValue('windows')
  win.maximized = false
  ;(window as unknown as { api: typeof api }).api = api
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe(): void {}
      disconnect(): void {}
    }
  )
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    cb(0)
    return 1
  })
  vi.stubGlobal('cancelAnimationFrame', () => {})
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  api.placeSnapOverlay.mockClear()
})

describe('WindowControls', () => {
  it.each(['mac', null] as const)('draws nothing without Windows caption buttons: %s', (kind) => {
    vi.mocked(platform.customTitlebar).mockReturnValue(kind)
    const { container } = render(<WindowControls />)
    expect(container.innerHTML).toBe('')
  })

  it('minimizes, maximizes and closes the window', async () => {
    render(<WindowControls />)
    fireEvent.click(await screen.findByRole('button', { name: 'Minimize' }))
    fireEvent.click(screen.getByRole('button', { name: 'Maximize' }))
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(win.minimize).toHaveBeenCalledOnce()
    expect(win.toggleMaximize).toHaveBeenCalledOnce()
    expect(win.close).toHaveBeenCalledOnce()
  })

  it('offers Restore once the window is maximized', async () => {
    render(<WindowControls />)
    expect(await screen.findByRole('button', { name: 'Maximize' })).toBeTruthy()
    win.maximized = true
    await act(async () => win.listeners.resized?.())
    expect(screen.getByRole('button', { name: 'Restore' })).toBeTruthy()
  })

  it('dims while the window is inactive', async () => {
    const { container } = render(<WindowControls />)
    await act(async () => {})
    const group = container.querySelector('.caption-buttons')!
    act(() => win.listeners.focus?.({ payload: false }))
    expect(group.classList.contains('caption-buttons--inactive')).toBe(true)
    act(() => win.listeners.focus?.({ payload: true }))
    expect(group.classList.contains('caption-buttons--inactive')).toBe(false)
  })

  it('puts the Snap Layouts overlay over Maximize, and removes it when unmounted', async () => {
    vi.stubGlobal('devicePixelRatio', 1.5)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(
      new DOMRect(822, 0, 46, 42)
    )
    const { unmount } = render(<WindowControls uiScale={1} />)
    await act(async () => {})
    expect(api.placeSnapOverlay).toHaveBeenLastCalledWith({
      x: 822,
      y: 0,
      width: 46,
      height: 42,
      pixelRatio: 1.5
    })
    unmount()
    expect(api.placeSnapOverlay).toHaveBeenLastCalledWith(null)
  })

  it('shows hover and press on Maximize from the native overlay', async () => {
    render(<WindowControls />)
    const maximize = await screen.findByRole('button', { name: 'Maximize' })
    act(() => pointer({ hover: true, pressed: false }))
    expect(maximize.className).toBe('caption-button is-hover')
    act(() => pointer({ hover: true, pressed: true }))
    expect(maximize.className).toBe('caption-button is-hover is-pressed')
    act(() => pointer({ hover: false, pressed: false }))
    expect(maximize.className).toBe('caption-button')
  })
})

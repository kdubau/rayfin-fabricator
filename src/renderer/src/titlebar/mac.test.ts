import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import type { RayfinStudioApi, WindowTitlebarState } from '@shared/ipc'
import * as platform from '../platform'
import { deferred } from '../../test/deferred'
import { useMacTitlebar } from './mac'

const root = document.documentElement

function installApi(initial: WindowTitlebarState | Promise<WindowTitlebarState>) {
  let emit!: (state: WindowTitlebarState) => void
  const unsubscribe = vi.fn()
  const api = {
    centerTrafficLights: vi.fn().mockReturnValue(Promise.resolve(initial)),
    revealTrafficLights: vi.fn().mockResolvedValue(undefined),
    onFullscreenChanged: vi.fn<RayfinStudioApi['onFullscreenChanged']>((cb) => {
      emit = cb
      return unsubscribe
    })
  }
  ;(window as unknown as { api: typeof api }).api = api
  return { api, unsubscribe, emit: (state: WindowTitlebarState) => act(() => emit(state)) }
}

function slide(): { animation: Animation; finish: () => Promise<void> } {
  const done = deferred<Animation>()
  const animation = { transitionProperty: 'padding-left', finished: done.promise }
  return {
    animation: animation as unknown as Animation,
    finish: () => act(async () => done.resolve(animation as unknown as Animation))
  }
}

let reducedMotion = false

beforeEach(() => {
  vi.spyOn(platform, 'isMac').mockReturnValue(true)
  root.style.setProperty('--titlebar-h', '42px')
  Object.defineProperty(document, 'getAnimations', { configurable: true, value: vi.fn(() => []) })
  vi.stubGlobal('matchMedia', (media: string) => ({ media, matches: reducedMotion }))
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  Reflect.deleteProperty(document, 'getAnimations')
  root.removeAttribute('style')
  delete root.dataset.macTitlebar
  delete root.dataset.macTitlebarSlide
  reducedMotion = false
})

describe('useMacTitlebar', () => {
  it('does nothing off macOS', () => {
    vi.mocked(platform.isMac).mockReturnValue(false)
    const { api } = installApi({ fullscreen: false, transitionId: 0 })
    renderHook(() => useMacTitlebar(1))
    expect(api.centerTrafficLights).not.toHaveBeenCalled()
    expect(api.onFullscreenChanged).not.toHaveBeenCalled()
  })

  it('centres the controls on the zoomed first row and applies the initial state', async () => {
    const { api } = installApi({ fullscreen: true, transitionId: 0 })
    const hook = renderHook(({ scale }) => useMacTitlebar(scale), { initialProps: { scale: 1 } })
    await act(async () => {})
    expect(api.centerTrafficLights).toHaveBeenLastCalledWith(21)
    expect(root.dataset.macTitlebar).toBe('fullscreen')
    expect(root.dataset.macTitlebarSlide).toBeUndefined()
    root.style.zoom = '1.5'
    hook.rerender({ scale: 1.5 })
    expect(api.centerTrafficLights).toHaveBeenLastCalledWith(31.5)
  })

  it('slides live transitions and keeps them over a late initial response', async () => {
    const initial = deferred<WindowTitlebarState>()
    const { emit, unsubscribe } = installApi(initial.promise)
    const hook = renderHook(() => useMacTitlebar(1))
    emit({ fullscreen: true, transitionId: 1 })
    await act(async () => initial.resolve({ fullscreen: false, transitionId: 0 }))
    expect(root.dataset.macTitlebar).toBe('fullscreen')
    expect(root.dataset.macTitlebarSlide).toBe('')
    hook.unmount()
    expect(unsubscribe).toHaveBeenCalledTimes(1)
  })

  it('reveals the controls only once the latest exit slide has finished', async () => {
    const { api, emit } = installApi({ fullscreen: true, transitionId: 1 })
    renderHook(() => useMacTitlebar(1))
    await act(async () => {})
    const stale = slide()
    const unrelated = { transitionProperty: 'opacity', finished: new Promise(() => {}) }
    vi.mocked(document.getAnimations).mockReturnValue([
      stale.animation,
      unrelated as unknown as Animation
    ])
    emit({ fullscreen: false, transitionId: 2 })
    emit({ fullscreen: true, transitionId: 3 })
    await stale.finish()
    expect(api.revealTrafficLights).not.toHaveBeenCalled()
    const latest = slide()
    vi.mocked(document.getAnimations).mockReturnValue([latest.animation])
    emit({ fullscreen: false, transitionId: 4 })
    expect(root.dataset.macTitlebar).toBe('window')
    expect(api.revealTrafficLights).not.toHaveBeenCalled()
    await latest.finish()
    expect(api.revealTrafficLights).toHaveBeenCalledExactlyOnceWith(4, true)
  })

  it('reveals without a fade under reduced motion', async () => {
    reducedMotion = true
    const { api } = installApi({ fullscreen: false, transitionId: 2 })
    renderHook(() => useMacTitlebar(1))
    await act(async () => {})
    expect(api.revealTrafficLights).toHaveBeenCalledExactlyOnceWith(2, false)
  })
})

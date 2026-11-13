import { useEffect } from 'react'
import type { WindowTitlebarState } from '@shared/ipc'
import { isMac } from '../platform'

/** Slides that clear the space under the native controls (see titlebar.css). */
function slides(): Animation[] {
  return document
    .getAnimations()
    .filter((animation) => (animation as CSSTransition).transitionProperty === 'padding-left')
}

/**
 * Owns the macOS main window's native controls on every screen. They centre on
 * the first `--titlebar-h` of the page, and `<html data-mac-titlebar>` tells
 * whatever bar is on top to keep clear of them (`window`) or not (`fullscreen`).
 * Live fullscreen transitions also set `data-mac-titlebar-slide`, and leaving
 * fullscreen fades the controls back in once that space has finished clearing.
 */
export function useMacTitlebar(uiScale: number | undefined): void {
  useEffect(() => {
    if (!isMac()) return
    const root = document.documentElement
    let cancelled = false
    let latest: WindowTitlebarState | null = null
    const revealAfterSlide = async (state: WindowTitlebarState): Promise<void> => {
      while (!cancelled && latest === state) {
        try {
          await Promise.all(slides().map((animation) => animation.finished))
        } catch (error) {
          if (error instanceof DOMException && error.name === 'AbortError') continue
          throw error
        }
        if (!cancelled && latest === state) {
          await window.api.revealTrafficLights(
            state.transitionId,
            !window.matchMedia('(prefers-reduced-motion: reduce)').matches
          )
        }
        return
      }
    }
    const apply = (state: WindowTitlebarState, slide: boolean): void => {
      latest = state
      root.dataset.macTitlebar = state.fullscreen ? 'fullscreen' : 'window'
      if (slide) root.dataset.macTitlebarSlide = ''
      if (!state.fullscreen) {
        void revealAfterSlide(state).catch((error: unknown) => {
          console.error('Could not reveal macOS window controls:', error)
        })
      }
    }
    const unsubscribe = window.api.onFullscreenChanged((state) => apply(state, true))
    const row = parseFloat(getComputedStyle(root).getPropertyValue('--titlebar-h'))
    const centerY = (row / 2) * (Number(root.style.zoom) || 1)
    if (centerY > 0) {
      void window.api
        .centerTrafficLights(centerY)
        .then((state) => {
          if (!cancelled && !latest) apply(state, false)
        })
        .catch((error: unknown) => {
          console.error('Could not align macOS window controls:', error)
        })
    }
    return () => {
      cancelled = true
      unsubscribe()
    }
  }, [uiScale])
}

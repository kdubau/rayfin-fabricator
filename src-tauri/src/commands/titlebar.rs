//! The main window's own title bar (`crate::titlebar`): aligning macOS's
//! native traffic lights, and placing the Windows Snap Layouts overlay.

use crate::error::{AppError, AppResult};
use crate::services::preview::PreviewBounds;
use crate::types::WindowTitlebarState;

/// Align the main window's native controls with the first row of the app bar.
#[tauri::command]
pub async fn window_center_traffic_lights(
  window: tauri::Window,
  center_y: f64,
) -> AppResult<WindowTitlebarState> {
  if window.label() != "main" || !center_y.is_finite() || !(1.0..=100.0).contains(&center_y) {
    return Err(AppError::Msg("Invalid window or app-bar position.".into()));
  }
  #[cfg(target_os = "macos")]
  {
    let target = window.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    window
      .run_on_main_thread(move || {
        let result = target
          .ns_window()
          .map_err(|error| AppError::Msg(error.to_string()))
          .and_then(|native| {
            // Tauri owns the window; AppKit is accessed only on its main thread.
            let native = unsafe { &*native.cast::<objc2_app_kit::NSWindow>() };
            crate::titlebar::set_center(native, center_y, move |fullscreen| {
              use tauri::Emitter;
              if let Err(error) = target.emit("window:fullscreen", fullscreen) {
                log::warn!("Could not report macOS fullscreen transition: {error}");
              }
            })
            .map_err(|error| AppError::Msg(error.into()))
          });
        let _ = tx.send(result);
      })
      .map_err(|error| AppError::Msg(error.to_string()))?;
    rx.await.map_err(|error| AppError::Msg(error.to_string()))?
  }
  #[cfg(not(target_os = "macos"))]
  {
    Err(AppError::Msg(
      "Window control alignment is only available on macOS.".into(),
    ))
  }
}

#[tauri::command]
pub fn window_reveal_traffic_lights(
  window: tauri::Window,
  transition_id: u32,
  animate: bool,
) -> AppResult<()> {
  if window.label() != "main" {
    return Err(AppError::Msg("Invalid window.".into()));
  }
  #[cfg(target_os = "macos")]
  {
    let target = window.clone();
    window.run_on_main_thread(move || {
      let result = target.ns_window().map_err(|error| error.to_string()).and_then(|native| {
        let native = unsafe { &*native.cast::<objc2_app_kit::NSWindow>() };
        crate::titlebar::reveal(native, transition_id, animate).map_err(str::to_owned)
      });
      if let Err(error) = result {
        log::warn!("Could not reveal macOS window controls: {error}");
      }
    }).map_err(|error| AppError::Msg(error.to_string()))?;
    Ok(())
  }
  #[cfg(not(target_os = "macos"))]
  {
    let _ = (transition_id, animate);
    Err(AppError::Msg("Window control alignment is only available on macOS.".into()))
  }
}

/// Put the native Snap Layouts overlay over the page's Maximize button
/// (`bounds` as measured for the native preview, with its pixel ratio), or
/// hide it (`None`). Hover and press on it come back on `window:maximize-button`.
#[tauri::command]
pub fn window_place_snap_overlay(window: tauri::Window, bounds: Option<PreviewBounds>) -> AppResult<()> {
  if window.label() != "main" || bounds.is_some_and(|bounds| bounds.pixel_ratio.is_none()) {
    return Err(AppError::Msg("Invalid window or Maximize button bounds.".into()));
  }
  #[cfg(windows)]
  {
    let rect = bounds
      .map(|bounds| {
        let rect = bounds.to_rect()?;
        let (tauri::Position::Physical(at), tauri::Size::Physical(size)) = (rect.position, rect.size) else {
          return Err(AppError::Msg("Invalid Maximize button bounds.".into()));
        };
        Ok(windows::Win32::Foundation::RECT {
          left: at.x,
          top: at.y,
          right: at.x.saturating_add(size.width as i32),
          bottom: at.y.saturating_add(size.height as i32),
        })
      })
      .transpose()?;
    let parent = window.hwnd().map_err(|error| AppError::Msg(error.to_string()))?.0 as isize;
    let target = window.clone();
    window
      .run_on_main_thread(move || {
        let parent = windows::Win32::Foundation::HWND(parent as _);
        let report = move |state| {
          use tauri::Emitter;
          if let Err(error) = target.emit("window:maximize-button", state) {
            log::warn!("Could not report the Maximize button's state: {error}");
          }
        };
        if let Err(error) = crate::titlebar::place_snap_overlay(parent, rect, report) {
          log::warn!("Could not place the Snap Layouts overlay: {error}");
        }
      })
      .map_err(|error| AppError::Msg(error.to_string()))
  }
  #[cfg(not(windows))]
  {
    let _ = bounds;
    Err(AppError::Msg("The Snap Layouts overlay is only available on Windows.".into()))
  }
}
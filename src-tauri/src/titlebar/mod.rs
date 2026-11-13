//! The main window's title bar. On macOS and Windows the native title bar gives
//! way to the app's own top bar, which reaches the window's top edge
//! (`src/renderer/src/titlebar/`). macOS keeps its native traffic lights and
//! aligns them with that bar (`macos`); Windows draws its caption buttons in
//! the page, with a native overlay over Maximize for Snap Layouts
//! (`snap_overlay`).

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod snap_overlay;

#[cfg(target_os = "macos")]
pub use macos::{reveal, set_center};
#[cfg(windows)]
pub use snap_overlay::place as place_snap_overlay;

/// Drop the native title bar from the main window.
pub fn main_window<'a, R: tauri::Runtime, M: tauri::Manager<R>>(
  builder: tauri::WebviewWindowBuilder<'a, R, M>,
) -> tauri::WebviewWindowBuilder<'a, R, M> {
  #[cfg(target_os = "macos")]
  let builder = builder
    .title_bar_style(tauri::TitleBarStyle::Overlay)
    .hidden_title(true);
  // Undecorated but shadowed keeps the DWM shadow, Windows 11's rounded
  // corners and resizing from every edge.
  #[cfg(windows)]
  let builder = builder.decorations(false).shadow(true);
  builder
}

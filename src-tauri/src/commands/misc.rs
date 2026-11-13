//! Misc app commands: ping, version info, open-external, open-logs, window reveal,
//! relaunch.

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::services::paths;
use crate::types::{AppVersions, OpenInEditorResult, WindowTitlebarState};

#[tauri::command]
pub fn ping() -> &'static str {
  "pong"
}

/// Version info for the About/Settings panel. Under Tauri there is no Node/V8
/// runtime, so the panel reports the app, Tauri framework, WebView2 and bundled
/// Copilot CLI versions.
#[tauri::command]
pub async fn get_versions(app: AppHandle) -> AppVersions {
  let app_version = app.package_info().version.to_string();
  let copilot = crate::services::copilot::bundled_cli_version().await;
  let copilot_bundled = crate::services::copilot::bundled_cli_pinned_version();
  AppVersions {
    app: app_version,
    tauri: tauri::VERSION.to_string(),
    webview2: webview_version(),
    copilot,
    copilot_bundled,
  }
}

fn webview_version() -> String {
  tauri::webview_version().unwrap_or_default()
}

/// Open a URL in the user's default browser (http/https only).
#[tauri::command]
pub async fn open_external(app: AppHandle, url: String) -> AppResult<()> {
  if url.starts_with("http://") || url.starts_with("https://") {
    app
      .opener()
      .open_url(url, None::<&str>)
      .map_err(|e| AppError::Msg(e.to_string()))?;
  }
  Ok(())
}

/// Open the logs folder in the OS file manager; returns its path.
#[tauri::command]
pub async fn open_logs(app: AppHandle) -> AppResult<String> {
  let dir = paths::logs_dir();
  let path = dir.to_string_lossy().to_string();
  app
    .opener()
    .open_path(path.clone(), None::<&str>)
    .map_err(|e| AppError::Msg(e.to_string()))?;
  Ok(path)
}

/// Open the project folder in VS Code (`code <dir>`). Falls back to revealing the
/// folder in the OS file manager when VS Code's CLI isn't installed.
#[tauri::command]
pub async fn open_in_editor(app: AppHandle, id: String) -> AppResult<OpenInEditorResult> {
  let project = crate::services::store::find_project(&id)
    .ok_or_else(|| AppError::Msg("Project not found.".into()))?;
  let path = project.path.clone();

  if launch_vscode(&path) {
    return Ok(OpenInEditorResult { opened: true, revealed_folder: None });
  }
  app
    .opener()
    .open_path(path.clone(), None::<&str>)
    .map_err(|e| AppError::Msg(e.to_string()))?;
  Ok(OpenInEditorResult { opened: false, revealed_folder: Some(true) })
}

/// Try to launch VS Code on `dir`, detached. Returns false when `code` isn't on PATH
/// or the spawn failed. On Windows `code` is a `.cmd` shim that must run via cmd.exe.
fn launch_vscode(dir: &str) -> bool {
  use std::process::{Command, Stdio};
  if which::which("code").is_err() {
    return false;
  }
  #[cfg(windows)]
  let mut cmd = {
    let mut c = Command::new("cmd");
    c.args(["/C", "code"]).arg(dir);
    c
  };
  #[cfg(not(windows))]
  let mut cmd = {
    let code = which::which("code").expect("checked above");
    let mut c = Command::new(code);
    c.arg(dir);
    c
  };
  cmd
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .is_ok()
}

/// Show the main window. It starts hidden and the renderer calls this once its
/// first screen is on the page, so a launch never shows an empty window or the
/// wrong theme's background. `background` (`#rrggbb`) is that screen's colour,
/// used for any frame the page hasn't painted yet.
///
/// A plain `Window`, not a `WebviewWindow`: once the preview's child webview is
/// added, the main window holds two webviews and is no longer the latter.
#[tauri::command]
pub fn window_reveal(window: tauri::Window, background: Option<String>) {
  if let Some(color) = background.as_deref().and_then(parse_hex_color) {
    let _ = window.set_background_color(Some(color));
  }
  reveal(&window);
}

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

/// Show `window` the first time; later calls leave it alone (it may have been
/// minimized or sent behind another window since).
pub fn reveal(window: &tauri::Window) {
  if window.is_visible().unwrap_or(true) {
    return;
  }
  if let Err(error) = window.show() {
    log::warn!("could not show the main window: {error}");
  }
  let _ = window.set_focus();
}

/// Bring the main window to the front, for a second launch that handed it a file,
/// or that asked for Fabricator itself. Opens it when Fabricator started only to
/// run apps (see [`open_main_window`]).
pub fn bring_forward(app: &AppHandle) {
  use tauri::Manager;
  if let Some(window) = app.get_window("main") {
    let _ = window.unminimize();
    reveal(&window);
    let _ = window.set_focus();
    return;
  }
  // Not on the event loop: making a web view there can deadlock on Windows.
  let app = app.clone();
  tauri::async_runtime::spawn(async move {
    if let Err(error) = open_main_window(&app) {
      log::error!("could not open the main window: {error}");
    }
  });
}

/// Open Fabricator's own window (`app > windows` in tauri.conf.json, which
/// doesn't create it, as a launch that only runs apps never shows it). Like
/// the configured window it starts hidden, and the renderer shows it once its
/// first screen is painted (see `window_reveal`); should the page never get
/// that far, it shows anyway after a few seconds. Closing it while app windows
/// stay open stops the projects' local backends, dev servers and Goals runs,
/// which nothing shows anymore.
pub fn open_main_window(app: &AppHandle) -> tauri::Result<()> {
  use tauri::Manager;
  if app.get_webview_window("main").is_some() {
    return Ok(());
  }
  let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main").cloned() else {
    return Err(tauri::Error::WindowNotFound);
  };
  let builder = tauri::WebviewWindowBuilder::from_config(app, &config)?;
  #[cfg(target_os = "macos")]
  let builder = builder
    .title_bar_style(tauri::TitleBarStyle::Overlay)
    .hidden_title(true);
  let window = builder.build()?;
  let handle = app.clone();
  window.on_window_event(move |event| {
    if let tauri::WindowEvent::Destroyed = event {
      let handle = handle.clone();
      tauri::async_runtime::spawn(async move {
        crate::services::dev_server::kill_all(&handle);
        if let Some(state) = handle.try_state::<crate::state::AppState>() {
          state.cancel_all_goals();
        }
        crate::services::local_backend::stop_all(&handle).await;
      });
    }
  });
  let shown = window.as_ref().window();
  std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_secs(4));
    reveal(&shown);
  });
  Ok(())
}

fn parse_hex_color(value: &str) -> Option<tauri::window::Color> {
  let hex = value.strip_prefix('#')?;
  if hex.len() != 6 || !hex.is_ascii() {
    return None;
  }
  let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
  Some(tauri::window::Color(channel(0)?, channel(2)?, channel(4)?, 255))
}

/// Restart the app (used to pick up newly installed Node/Git on PATH).
#[tauri::command]
pub fn relaunch(app: AppHandle) {
  app.restart();
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_the_renderers_background_colour() {
    let color = parse_hex_color("#0b0e14").expect("a #rrggbb colour");
    assert_eq!((color.0, color.1, color.2, color.3), (0x0b, 0x0e, 0x14, 255));
    assert!(parse_hex_color("#ECEFF4").is_some());
    for bad in ["0b0e14", "#0b0e1", "#0b0e14ff", "#zzzzzz", "", "#é0e14"] {
      assert!(parse_hex_color(bad).is_none(), "{bad:?} is not a colour");
    }
  }
}

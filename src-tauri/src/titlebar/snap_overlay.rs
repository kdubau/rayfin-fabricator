//! The Snap Layouts overlay over the page's Maximize button. Windows 11 offers
//! its Snap Layouts flyout only where the window under the pointer answers
//! `WM_NCHITTEST` with `HTMAXBUTTON`, and over the page that window is
//! WebView2's own child, which the main window can't answer for. So a small
//! child window of ours sits over the button instead: it never paints, so the
//! page's button shows through, maximizes or restores the window when clicked,
//! and reports hover and press back to the page, which can't see the pointer
//! there. Windows Terminal does the same over its XAML caption buttons.
//!
//! Everything here runs on the main thread, which owns the window.

use std::{cell::RefCell, rc::Rc};

use windows::{
  core::{w, PCWSTR},
  Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{GetStockObject, ValidateRect, HBRUSH, NULL_BRUSH},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
      HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
      Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT},
      WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, IsWindow, IsZoomed, LoadCursorW, PostMessageW,
        RegisterClassExW, SetWindowPos, SetWindowTextW, ShowWindow, HTMAXBUTTON, HWND_TOP,
        IDC_ARROW, SC_MAXIMIZE, SC_RESTORE, SM_CYFRAME, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
        WINDOW_EX_STYLE, WM_ERASEBKGND, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK,
        WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_PAINT,
        WM_SYSCOMMAND, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS,
      },
    },
  },
};

use crate::types::MaximizeButtonState;

const CLASS: PCWSTR = w!("FabricatorSnapOverlay");

type Report = Rc<dyn Fn(MaximizeButtonState)>;

struct Overlay {
  hwnd: HWND,
  parent: HWND,
  state: MaximizeButtonState,
  report: Report,
}

thread_local! {
  static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

/// Put the overlay over `rect` (the Maximize button, in the main window's
/// physical client pixels), creating it the first time, or hide it (`None`).
pub fn place(
  parent: HWND,
  rect: Option<RECT>,
  report: impl Fn(MaximizeButtonState) + 'static,
) -> windows::core::Result<()> {
  let existing = OVERLAY.with_borrow(|overlay| {
    overlay
      .as_ref()
      .filter(|overlay| overlay.parent == parent && unsafe { IsWindow(Some(overlay.hwnd)) }.as_bool())
      .map(|overlay| overlay.hwnd)
  });
  let Some(rect) = rect else {
    if let Some(hwnd) = existing {
      unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
      }
    }
    return Ok(());
  };
  let hwnd = match existing {
    Some(hwnd) => hwnd,
    None => create(parent, Rc::new(report))?,
  };
  let maximized = unsafe { IsZoomed(parent) }.as_bool();
  // Leave the restored window's top resize edge to Tauri's resize border
  // (tauri-runtime-wry's undecorated_resizing uses the same SM_CYFRAME).
  let top = if maximized {
    rect.top
  } else {
    rect.top.max(unsafe { GetSystemMetricsForDpi(SM_CYFRAME, GetDpiForWindow(parent)) })
  };
  unsafe {
    // Its window text is the name assistive tech reads for it.
    SetWindowTextW(hwnd, if maximized { w!("Restore") } else { w!("Maximize") })?;
    // Above the webview, which can be re-stacked as webviews come and go.
    SetWindowPos(
      hwnd,
      Some(HWND_TOP),
      rect.left,
      top,
      rect.right - rect.left,
      (rect.bottom - top).max(0),
      SWP_NOACTIVATE | SWP_SHOWWINDOW,
    )
  }
}

fn create(parent: HWND, report: Report) -> windows::core::Result<HWND> {
  unsafe {
    let instance = GetModuleHandleW(None)?;
    let class = WNDCLASSEXW {
      cbSize: size_of::<WNDCLASSEXW>() as u32,
      lpfnWndProc: Some(window_proc),
      hInstance: instance.into(),
      hCursor: LoadCursorW(None, IDC_ARROW)?,
      // Never painted: the page's button shows through.
      hbrBackground: HBRUSH(GetStockObject(NULL_BRUSH).0),
      lpszClassName: CLASS,
      ..Default::default()
    };
    // Fails harmlessly once the class exists, for a reopened main window.
    RegisterClassExW(&class);
    let hwnd = CreateWindowExW(
      WINDOW_EX_STYLE::default(),
      CLASS,
      w!("Maximize"),
      WS_CHILD | WS_CLIPSIBLINGS,
      0,
      0,
      0,
      0,
      Some(parent),
      None,
      Some(instance.into()),
      None,
    )?;
    OVERLAY.with_borrow_mut(|overlay| {
      *overlay = Some(Overlay { hwnd, parent, state: MaximizeButtonState::default(), report });
    });
    Ok(hwnd)
  }
}

/// Apply `change` to the overlay's pointer state and report it if it changed.
fn update(hwnd: HWND, change: impl FnOnce(&mut MaximizeButtonState)) -> Option<MaximizeButtonState> {
  let (before, after, report) = OVERLAY.with_borrow_mut(|overlay| {
    let overlay = overlay.as_mut().filter(|overlay| overlay.hwnd == hwnd)?;
    let before = overlay.state;
    change(&mut overlay.state);
    Some((before, overlay.state, overlay.report.clone()))
  })?;
  if before != after {
    report(after);
  }
  Some(before)
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
  unsafe {
    match msg {
      WM_NCHITTEST => LRESULT(HTMAXBUTTON as isize),
      WM_NCMOUSEMOVE => {
        if update(hwnd, |state| state.hover = true).is_some_and(|before| !before.hover) {
          let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE | TME_NONCLIENT,
            hwndTrack: hwnd,
            dwHoverTime: 0,
          };
          let _ = TrackMouseEvent(&mut track);
        }
        LRESULT(0)
      }
      WM_NCMOUSELEAVE => {
        update(hwnd, |state| *state = MaximizeButtonState::default());
        LRESULT(0)
      }
      WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
        update(hwnd, |state| state.pressed = true);
        LRESULT(0)
      }
      WM_NCLBUTTONUP => {
        let was = update(hwnd, |state| state.pressed = false);
        let parent = OVERLAY.with_borrow(|overlay| overlay.as_ref().map(|overlay| overlay.parent));
        if let (Some(before), Some(parent)) = (was, parent) {
          if before.pressed {
            // As the native button does, so the window animates and snaps.
            let command = if IsZoomed(parent).as_bool() { SC_RESTORE } else { SC_MAXIMIZE };
            let _ = PostMessageW(Some(parent), WM_SYSCOMMAND, WPARAM(command as usize), LPARAM(0));
          }
        }
        LRESULT(0)
      }
      WM_PAINT => {
        let _ = ValidateRect(Some(hwnd), None);
        LRESULT(0)
      }
      WM_ERASEBKGND => LRESULT(1),
      WM_NCDESTROY => {
        OVERLAY.with_borrow_mut(|overlay| {
          if overlay.as_ref().is_some_and(|overlay| overlay.hwnd == hwnd) {
            *overlay = None;
          }
        });
        DefWindowProcW(hwnd, msg, wparam, lparam)
      }
      _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, SendMessageW, WS_OVERLAPPEDWINDOW,
  };

  #[test]
  fn answers_hit_tests_as_the_maximize_button_and_reports_the_pointer() {
    unsafe {
      let parent = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("parent"),
        WS_OVERLAPPEDWINDOW,
        0,
        0,
        400,
        300,
        None,
        None,
        None,
        None,
      )
      .unwrap();
      let seen = Rc::new(RefCell::new(Vec::new()));
      let log = seen.clone();
      let rect = RECT { left: 300, top: 0, right: 346, bottom: 42 };
      place(parent, Some(rect), move |state| log.borrow_mut().push(state)).unwrap();
      let hwnd = OVERLAY.with_borrow(|overlay| overlay.as_ref().unwrap().hwnd);

      let hit = SendMessageW(hwnd, WM_NCHITTEST, None, None);
      assert_eq!(hit.0, HTMAXBUTTON as isize);

      SendMessageW(hwnd, WM_NCMOUSEMOVE, None, None);
      SendMessageW(hwnd, WM_NCLBUTTONDOWN, None, None);
      SendMessageW(hwnd, WM_NCMOUSELEAVE, None, None);
      let pressed = MaximizeButtonState { hover: true, pressed: true };
      let hovered = MaximizeButtonState { hover: true, pressed: false };
      assert_eq!(*seen.borrow(), [hovered, pressed, MaximizeButtonState::default()]);

      let _ = DestroyWindow(parent);
      assert!(OVERLAY.with_borrow(Option::is_none));
    }
  }
}

//! Main-window traffic lights. Layout notifications run synchronously in AppKit,
//! before drawing, rather than making a round trip through the renderer.

use std::{
  cell::RefCell,
  ffi::c_void,
  ptr::{null_mut, NonNull},
};

use block2::RcBlock;
use objc2::{
  define_class, msg_send,
  rc::Retained,
  runtime::{AnyObject, NSObject, ProtocolObject},
  DefinedClass, MainThreadOnly,
};
use objc2_app_kit::{
  NSAnimatablePropertyContainer, NSAnimationContext, NSButton, NSViewFrameDidChangeNotification,
  NSWindow, NSWindowButton, NSWindowDidEnterFullScreenNotification,
  NSWindowDidExitFullScreenNotification, NSWindowDidResizeNotification, NSWindowStyleMask,
  NSWindowWillCloseNotification, NSWindowWillEnterFullScreenNotification,
  NSWindowWillExitFullScreenNotification,
};
use objc2_foundation::{
  ns_string, NSDictionary, NSKeyValueChangeKey, NSKeyValueObservingOptions, NSNotification,
  NSNotificationCenter, NSObjectNSKeyValueObserverRegistration, NSObjectProtocol, NSString,
};

use crate::types::WindowTitlebarState;

thread_local! {
  static TITLEBAR: RefCell<Option<Titlebar>> = const { RefCell::new(None) };
}

struct Titlebar {
  center_y: f64,
  state: WindowTitlebarState,
  transitioning: bool,
  reveal_pending: bool,
  observers: [Retained<ProtocolObject<dyn NSObjectProtocol>>; 2],
  _title_observer: Retained<FabricatorTitleObserver>,
}

// AppKit resets the buttons on title changes even when their container stays the same size.
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Retained<NSWindow>]
    struct FabricatorTitleObserver;

    impl FabricatorTitleObserver {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn title_changed(
            &self,
            _key_path: Option<&NSString>,
            _object: Option<&AnyObject>,
            _change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>,
            _context: *mut c_void,
        ) {
            realign(self.ivars());
        }
    }

    unsafe impl NSObjectProtocol for FabricatorTitleObserver {}
);

impl FabricatorTitleObserver {
  fn new(window: Retained<NSWindow>) -> Retained<Self> {
    let observer = Self::alloc(window.mtm()).set_ivars(window);
    let observer: Retained<Self> = unsafe { msg_send![super(observer), init] };
    unsafe {
      observer.ivars().addObserver_forKeyPath_options_context(
        &observer,
        ns_string!("title"),
        NSKeyValueObservingOptions::New,
        null_mut(),
      );
    }
    observer
  }
}

impl Drop for FabricatorTitleObserver {
  fn drop(&mut self) {
    unsafe {
      self
        .ivars()
        .removeObserver_forKeyPath(self, ns_string!("title"))
    };
  }
}

impl Drop for Titlebar {
  fn drop(&mut self) {
    for observer in &self.observers {
      unsafe { NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref()) };
    }
  }
}

pub fn set_center(
  window: &NSWindow,
  center_y: f64,
  on_fullscreen: impl Fn(WindowTitlebarState) + 'static,
) -> Result<WindowTitlebarState, &'static str> {
  let state = TITLEBAR.with_borrow_mut(|state| -> Result<WindowTitlebarState, &'static str> {
    if let Some(titlebar) = state {
      titlebar.center_y = center_y;
      return Ok(titlebar.state);
    }
    let close = window
      .standardWindowButton(NSWindowButton::CloseButton)
      .ok_or("Could not find the macOS window controls.")?;
    let parent =
      unsafe { close.superview() }.ok_or("Could not find the window controls' parent.")?;
    let container =
      unsafe { parent.superview() }.ok_or("Could not find the titlebar container.")?;
    let target = close
      .window()
      .ok_or("Could not find the controls' window.")?;
    let title_observer = FabricatorTitleObserver::new(target.clone());
    let layout_target = target.clone();
    let layout_callback = RcBlock::new(move |_: NonNull<NSNotification>| realign(&layout_target));
    // AppKit resets this frame after will-exit, before fading the controls in.
    container.setPostsFrameChangedNotifications(true);
    let layout_observer = unsafe {
      NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
        Some(NSViewFrameDidChangeNotification),
        Some(&container),
        None,
        &layout_callback,
      )
    };
    let callback = RcBlock::new(move |notification: NonNull<NSNotification>| {
      let notification = unsafe { notification.as_ref() };
      let name = notification.name();
      if &*name == unsafe { NSWindowWillCloseNotification } {
        TITLEBAR.with_borrow_mut(|state| *state = None);
        return;
      }
      let entering = &*name == unsafe { NSWindowWillEnterFullScreenNotification };
      let exiting = &*name == unsafe { NSWindowWillExitFullScreenNotification };
      let entered = &*name == unsafe { NSWindowDidEnterFullScreenNotification };
      let exited = &*name == unsafe { NSWindowDidExitFullScreenNotification };
      let settled = TITLEBAR.with_borrow_mut(|state| {
        state.as_mut().and_then(|titlebar| {
          if entering || exiting {
            titlebar.state.fullscreen = entering;
            titlebar.state.transition_id = titlebar.state.transition_id.wrapping_add(1);
            titlebar.transitioning = true;
            titlebar.reveal_pending |= exiting;
          } else if entered || exited {
            titlebar.state.fullscreen = entered;
            titlebar.transitioning = false;
            return Some(titlebar.state);
          }
          None
        })
      });
      if entering
        || exiting
        || entered
        || exited
        || &*name == unsafe { NSWindowDidResizeNotification }
      {
        realign(&target);
      }
      // WebKit suspends drawing during the system animation. Start the visible
      // slide only once it resumes, with the lights hidden until the inset clears.
      if let Some(state) = settled {
        on_fullscreen(state);
      }
    });
    // A nil queue delivers inline on the posting thread. The observer filters
    // this main-thread-only window, and is removed when that window closes.
    let observer = unsafe {
      NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
        None,
        Some(window),
        None,
        &callback,
      )
    };
    let initial = WindowTitlebarState {
      fullscreen: window.styleMask().contains(NSWindowStyleMask::FullScreen),
      transition_id: 0,
    };
    *state = Some(Titlebar {
      center_y,
      state: initial,
      transitioning: false,
      reveal_pending: false,
      observers: [observer, layout_observer],
      _title_observer: title_observer,
    });
    Ok(initial)
  })?;
  if !state.fullscreen {
    align(window, center_y)?;
  }
  Ok(state)
}

pub fn reveal(window: &NSWindow, transition_id: u32, animate: bool) -> Result<(), &'static str> {
  let center = TITLEBAR.with_borrow(|state| {
    state
      .as_ref()
      .filter(|titlebar| {
        titlebar.state.transition_id == transition_id
          && !titlebar.state.fullscreen
          && !titlebar.transitioning
          && titlebar.reveal_pending
      })
      .map(|titlebar| titlebar.center_y)
  });
  if let Some(center_y) = center {
    let buttons = buttons(window)?;
    TITLEBAR.with_borrow_mut(|state| {
      if let Some(titlebar) = state {
        titlebar.reveal_pending = false;
      }
    });
    if animate {
      NSAnimationContext::beginGrouping();
      NSAnimationContext::currentContext().setDuration(0.18);
      for button in buttons {
        button.animator().setAlphaValue(1.0);
      }
      NSAnimationContext::endGrouping();
    } else {
      for button in buttons {
        button.setAlphaValue(1.0);
      }
    }
    // AppKit can reset widget frames when restoring their opacity.
    align(window, center_y)?;
  }
  Ok(())
}

fn realign(window: &NSWindow) {
  let layout = TITLEBAR.with_borrow(|state| {
    state.as_ref().map(|titlebar| {
      (
        titlebar.state.fullscreen,
        titlebar.center_y,
        titlebar.reveal_pending,
      )
    })
  });
  if let Some((fullscreen, center_y, hide)) = layout {
    let result = (|| {
      if hide {
        for button in buttons(window)? {
          button.setAlphaValue(0.0);
        }
      }
      if !fullscreen {
        align(window, center_y)?;
      }
      Ok::<_, &'static str>(())
    })();
    if let Err(error) = result {
      log::warn!("Could not align macOS window controls: {error}");
    }
  }
}

fn buttons(window: &NSWindow) -> Result<[Retained<NSButton>; 3], &'static str> {
  let buttons = [
    NSWindowButton::CloseButton,
    NSWindowButton::MiniaturizeButton,
    NSWindowButton::ZoomButton,
  ]
  .map(|kind| window.standardWindowButton(kind));
  match buttons {
    [Some(close), Some(minimize), Some(zoom)] => Ok([close, minimize, zoom]),
    _ => Err("Could not find the macOS window controls."),
  }
}

fn align(window: &NSWindow, center_y: f64) -> Result<(), &'static str> {
  let [close, minimize, zoom] = buttons(window)?;
  let parent = unsafe { close.superview() }.ok_or("Could not find the window controls' parent.")?;
  let container = unsafe { parent.superview() }.ok_or("Could not find the titlebar container.")?;
  let spacing = minimize.frame().origin.x - close.frame().origin.x;

  // Match wry's native inset approach so zoomed controls remain inside the
  // titlebar's clipping region. Only the first app-bar row reserves this space.
  let mut frame = container.frame();
  frame.size.height = center_y * 2.0;
  frame.origin.y = window.frame().size.height - frame.size.height;
  if container.frame() != frame {
    container.setFrame(frame);
  }
  for (index, button) in [close, minimize, zoom].into_iter().enumerate() {
    let mut origin = button.frame().origin;
    origin.x = 16.0 + index as f64 * spacing;
    origin.y = (parent.bounds().size.height - button.frame().size.height) / 2.0;
    button.setFrameOrigin(origin);
  }
  Ok(())
}

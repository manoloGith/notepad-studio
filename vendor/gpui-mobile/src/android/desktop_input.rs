//! Mouse and physical keyboard, for Samsung DeX and Android's desktop windowing.
//!
//! Not part of upstream gpui-mobile, which treats every pointer as a finger and takes text from
//! the IME. A mouse there cannot hover, right-click or turn its wheel, and a click that moves a
//! little is a pan instead of a click. Here:
//!
//! - Motion events of a mouse become [`MouseInput`]s (move, button down and up, wheel, leave),
//!   which the window turns into GPUI's mouse events.
//! - The keys of a physical keyboard come from `GpuiInputActivity.dispatchKeyEvent` (the Java
//!   UI thread), which resolves the character with the keyboard's layout and dead keys, and are
//!   queued here for GPUI's thread.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::Mutex;

use super::AndroidKeyEvent;

/// `MotionEvent.BUTTON_PRIMARY`, `BUTTON_SECONDARY`, `BUTTON_TERTIARY`, `BUTTON_BACK` and
/// `BUTTON_FORWARD`.
pub const BUTTON_PRIMARY: u32 = 1;
pub const BUTTON_SECONDARY: u32 = 2;
pub const BUTTON_TERTIARY: u32 = 4;
pub const BUTTON_BACK: u32 = 8;
pub const BUTTON_FORWARD: u32 = 16;
const BUTTONS: u32 = 0x1f;

/// What a mouse motion event is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseAction {
    /// `ACTION_DOWN`: the first button went down.
    Down,
    /// `ACTION_UP`: the last button went up.
    Up,
    /// `ACTION_MOVE` (a button is down) or `ACTION_HOVER_MOVE`/`ACTION_HOVER_ENTER`.
    Move,
    /// `ACTION_BUTTON_PRESS` or `ACTION_BUTTON_RELEASE`: the button state changed.
    Button,
    /// `ACTION_SCROLL`, with the wheel's horizontal and vertical steps.
    Scroll { horizontal: f32, vertical: f32 },
    /// `ACTION_HOVER_EXIT`: the pointer left the window.
    Exit,
    /// `ACTION_CANCEL`.
    Cancel,
}

/// One thing the mouse did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseKind {
    Move,
    /// One of the `BUTTON_*` bits.
    Down(u32),
    Up(u32),
    Scroll { horizontal: f32, vertical: f32 },
    Exit,
}

/// A mouse event for the window, in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct MouseInput {
    pub kind: MouseKind,
    pub x: f32,
    pub y: f32,
    /// `KeyEvent.META_*` bits.
    pub meta_state: i32,
}

/// The buttons that are down, as the last event left them.
static PRESSED: AtomicU32 = AtomicU32::new(0);

/// The mouse events one motion event stands for: where the pointer is, then the buttons whose
/// state differs from the last event (Android reports a press both as `ACTION_DOWN` and as
/// `ACTION_BUTTON_PRESS`, and some devices leave the button state at 0 on `ACTION_DOWN`).
pub fn mouse_inputs(action: MouseAction, x: f32, y: f32, button_state: u32, meta_state: i32) -> Vec<MouseInput> {
    let input = |kind| MouseInput { kind, x, y, meta_state };
    let previous = PRESSED.load(Ordering::Relaxed);
    let now = match action {
        MouseAction::Down if button_state & BUTTONS == 0 => BUTTON_PRIMARY,
        MouseAction::Down | MouseAction::Button | MouseAction::Move => button_state & BUTTONS,
        MouseAction::Up | MouseAction::Cancel | MouseAction::Exit => 0,
        MouseAction::Scroll { .. } => previous,
    };
    // A move with a button down that reports none (seen with `ACTION_DOWN` at 0) keeps it.
    let now = if action == MouseAction::Move && now == 0 && previous != 0 && button_state & BUTTONS == 0 {
        previous
    } else {
        now
    };
    PRESSED.store(now, Ordering::Relaxed);

    let mut inputs = Vec::with_capacity(2);
    if action != MouseAction::Exit {
        inputs.push(input(MouseKind::Move));
    }
    for bit in [BUTTON_PRIMARY, BUTTON_SECONDARY, BUTTON_TERTIARY, BUTTON_BACK, BUTTON_FORWARD] {
        if now & bit != 0 && previous & bit == 0 {
            inputs.push(input(MouseKind::Down(bit)));
        } else if now & bit == 0 && previous & bit != 0 {
            inputs.push(input(MouseKind::Up(bit)));
        }
    }
    match action {
        MouseAction::Scroll { horizontal, vertical } => inputs.push(input(MouseKind::Scroll { horizontal, vertical })),
        MouseAction::Exit => inputs.push(input(MouseKind::Exit)),
        _ => {}
    }
    inputs
}

/// Keys of a physical keyboard, from the Java UI thread.
static KEYS: Mutex<VecDeque<AndroidKeyEvent>> = Mutex::new(VecDeque::new());

/// Queue a key for GPUI's thread and wake it.
pub fn enqueue_key(event: AndroidKeyEvent) {
    KEYS.lock().push_back(event);
    super::frame_source::wake();
}

/// The keys queued since the last call.
pub fn take_keys() -> VecDeque<AndroidKeyEvent> {
    std::mem::take(&mut *KEYS.lock())
}

/// The first events of each kind are logged (codes and buttons, never the text), to tell from a
/// device without adb whether input arrives at all.
pub fn log_first(counter: &AtomicU32, limit: u32, message: impl FnOnce() -> String) {
    if counter.fetch_add(1, Ordering::Relaxed) < limit {
        log::info!("{}", message());
    }
}

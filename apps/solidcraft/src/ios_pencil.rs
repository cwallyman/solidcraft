//! iPadOS: what the platform knows about a frame's taps, for Pencil mode
//! (`solidcraft_ui_egui::pencil`), as the web build's `mouse_buttons` works it out from the
//! browser's pointer events. Here the frame's own touches say it: egui-winit gives every Apple
//! Pencil touch the device `egui_winit::STYLUS_TOUCH_DEVICE`, and a finger's keeps its own. A
//! trackpad or mouse arrives as a mouse (vendor/winit), so it is neither.

use std::cell::Cell;

use egui::{Event, PointerButton, Pos2, RawInput, TouchId, TouchPhase};
use solidcraft_ui_egui::pencil::{PEN_RELEASE_SLOP, PlatformTaps};

thread_local! {
    /// The Pencil touch on the screen now, if any.
    static PEN_CONTACT: Cell<Option<TouchId>> = const { Cell::new(None) };
}

/// The taps in this frame's input (`raw`, as egui-winit made it).
// Only the iPad app calls it; elsewhere the module is built for its tests.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn taps(raw: &RawInput) -> PlatformTaps {
    PEN_CONTACT.with(|contact| {
        let mut pen = contact.get();
        let taps = taps_with(raw, &mut pen);
        contact.set(pen);
        taps
    })
}

/// [`taps`], with the Pencil touch on the screen before the frame in `pen` (and after it, on
/// return).
fn taps_with(raw: &RawInput, pen: &mut Option<TouchId>) -> PlatformTaps {
    let mut taps = PlatformTaps::default();
    let mut lifted = None;
    for event in &raw.events {
        let Event::Touch { device_id, id, phase, pos, .. } = event else { continue };
        if *device_id != egui_winit::STYLUS_TOUCH_DEVICE {
            if *phase == TouchPhase::Start {
                taps.finger_down = Some(*pos);
            }
            continue;
        }
        match phase {
            TouchPhase::Start => {
                if !taps.stylus.contains(id) {
                    taps.stylus.push(*id);
                }
                *pen = Some(*id);
                taps.pen_press = Some(*pos);
                lifted = None;
            }
            TouchPhase::End if *pen == Some(*id) => {
                *pen = None;
                lifted = Some(*pos);
            }
            // A cancelled touch is never a tap.
            TouchPhase::Cancel if *pen == Some(*id) => {
                *pen = None;
                taps.pen_press = None;
                lifted = None;
            }
            _ => {}
        }
    }
    taps.pen_release = lifted.and_then(|at| primary_release_near(&raw.events, at));
    taps.pen_down = pen.is_some();
    taps
}

/// The frame's last primary release, when it is near the pen's lift `at`: the release egui-winit
/// makes for the pen's touch.
fn primary_release_near(events: &[Event], at: Pos2) -> Option<Pos2> {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, .. } => Some(*pos),
            _ => None,
        })
        .filter(|pos| pos.distance(at) <= PEN_RELEASE_SLOP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Modifiers, TouchDeviceId, pos2};

    const FINGER: TouchDeviceId = TouchDeviceId(7);

    fn touch(device_id: TouchDeviceId, id: u64, phase: TouchPhase, pos: Pos2) -> Event {
        Event::Touch { device_id, id: TouchId(id), phase, pos, force: None }
    }

    fn release(pos: Pos2) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }
    }

    fn frame(events: Vec<Event>, pen: &mut Option<TouchId>) -> PlatformTaps {
        taps_with(&RawInput { events, ..Default::default() }, pen)
    }

    #[test]
    fn a_pencil_tap_is_a_pen_press_then_a_pen_release() {
        let stylus = egui_winit::STYLUS_TOUCH_DEVICE;
        let mut pen = None;
        let down = frame(vec![touch(stylus, 1, TouchPhase::Start, pos2(10.0, 20.0))], &mut pen);
        assert_eq!(down.pen_press, Some(pos2(10.0, 20.0)));
        assert_eq!(down.stylus, vec![TouchId(1)]);
        assert!(down.pen_down);
        assert_eq!(down.finger_down, None);

        let up = frame(vec![touch(stylus, 1, TouchPhase::End, pos2(11.0, 20.0)), release(pos2(10.0, 20.0))], &mut pen);
        assert_eq!(up.pen_release, Some(pos2(10.0, 20.0)));
        assert!(!up.pen_down);
        assert_eq!(pen, None);
    }

    #[test]
    fn a_finger_is_a_finger_down_and_never_the_pen() {
        let mut pen = None;
        let taps = frame(vec![touch(FINGER, 2, TouchPhase::Start, pos2(5.0, 5.0))], &mut pen);
        assert_eq!(taps.finger_down, Some(pos2(5.0, 5.0)));
        assert!(taps.stylus.is_empty() && taps.pen_press.is_none() && !taps.pen_down);

        let up = frame(vec![touch(FINGER, 2, TouchPhase::End, pos2(5.0, 5.0)), release(pos2(5.0, 5.0))], &mut pen);
        assert_eq!(up.pen_release, None);
    }

    #[test]
    fn a_cancelled_pencil_touch_or_a_far_release_is_no_tap() {
        let stylus = egui_winit::STYLUS_TOUCH_DEVICE;
        let mut pen = None;
        frame(vec![touch(stylus, 3, TouchPhase::Start, pos2(0.0, 0.0))], &mut pen);
        let cancelled = frame(vec![touch(stylus, 3, TouchPhase::Cancel, pos2(0.0, 0.0)), release(pos2(0.0, 0.0))], &mut pen);
        assert_eq!(cancelled.pen_release, None);
        assert!(!cancelled.pen_down);

        frame(vec![touch(stylus, 4, TouchPhase::Start, pos2(0.0, 0.0))], &mut pen);
        let far = frame(vec![touch(stylus, 4, TouchPhase::End, pos2(0.0, 0.0)), release(pos2(100.0, 0.0))], &mut pen);
        assert_eq!(far.pen_release, None);
    }
}

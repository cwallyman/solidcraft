//! A number pad for the sketch tools' value boxes (`sketch_dims`), in Pencil mode: a tap on a box
//! opens it beside the box, in place of the system keyboard, which would cover the drawing. The
//! boxes stay where they are and keep following the shape. Keys type into the box as typing does
//! (the first replaces the shown value); Clear empties it; Esc closes the pad and puts the box
//! back as it was; OK keeps the value, and places the point once every box has one (as Enter).

use crate::tools::Tool;
use crate::ui_kit::{self, Container, Surface};
use egui::{Pos2, Rect, vec2};
use solidcraft_theme::Tokens;

use crate::sketch_dims::DimBox;

/// The pad on screen: which box it types into, and that box as it was when the pad opened.
#[derive(Clone, Debug, Default)]
struct Pad {
    index: usize,
    /// The shape's placed points when the pad opened: a new point closes it.
    points: usize,
    before: (String, bool),
    /// No key pressed yet: the first replaces the shown value.
    fresh: bool,
}

fn pad_id() -> egui::Id {
    egui::Id::unique("sc_dim_pad")
}

fn place_id() -> egui::Id {
    egui::Id::unique("sc_dim_pad_place")
}

/// A key on the pad.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Key {
    Insert(char),
    Backspace,
    Clear,
    Esc,
    Ok,
}

/// The keys, row by row.
const ROWS: [[(&str, Key); 4]; 4] = [
    [("7", Key::Insert('7')), ("8", Key::Insert('8')), ("9", Key::Insert('9')), ("⌫", Key::Backspace)],
    [("4", Key::Insert('4')), ("5", Key::Insert('5')), ("6", Key::Insert('6')), ("sketch.number_pad.clear", Key::Clear)],
    [("1", Key::Insert('1')), ("2", Key::Insert('2')), ("3", Key::Insert('3')), ("sketch.number_pad.esc", Key::Esc)],
    [("−", Key::Insert('-')), ("0", Key::Insert('0')), (".", Key::Insert('.')), ("sketch.number_pad.ok", Key::Ok)],
];

/// Whether the pad is open.
pub(crate) fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Pad>(pad_id()).is_some())
}

/// Whether OK asked to place the point: once, on the frame after.
pub(crate) fn take_place(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(place_id())).unwrap_or(false)
}

/// Open the pad for box `index` (`b`, of a shape with `points` placed) when it was tapped in
/// Pencil mode.
pub(crate) fn open_on_tap(pencil_mode: bool, response: &egui::Response, b: &DimBox, index: usize, points: usize) {
    if !pencil_mode || !response.clicked() {
        return;
    }
    let pad = Pad { index, points, before: (b.value.clone(), b.locked), fresh: true };
    response.ctx.data_mut(|d| d.insert_temp(pad_id(), pad));
}

/// While the pad is open no box keeps the keyboard, so the system keyboard stays down.
pub(crate) fn hold_focus(ui: &egui::Ui, response: &egui::Response) {
    if is_open(ui.ctx()) && (response.has_focus() || response.gained_focus()) {
        response.surrender_focus();
        ui.ctx().output_mut(|o| o.ime = None);
    }
}

/// Apply `key` to `b`. Returns whether the pad closes.
fn press(pad: &mut Pad, b: &mut DimBox, key: Key) -> bool {
    match key {
        Key::Insert(c) => {
            if pad.fresh {
                b.value.clear();
            }
            b.value.push(c);
            b.locked = true;
        }
        Key::Backspace => {
            if pad.fresh {
                b.value.clear();
            } else {
                b.value.pop();
            }
            b.locked = true;
        }
        Key::Clear => {
            b.value.clear();
            b.locked = true;
        }
        Key::Esc => {
            (b.value, b.locked) = pad.before.clone();
            return true;
        }
        Key::Ok => {
            // An emptied box follows the shape again (a construction line's Length: infinite).
            if b.value.trim().is_empty() {
                b.locked = false;
            }
            return true;
        }
    }
    pad.fresh = false;
    false
}

/// Draw the pad beside its box (`boxes[i]` is where box `i` was drawn).
pub(crate) fn show(ui: &egui::Ui, t: &mut Tool, boxes: &[Option<Rect>]) {
    let ctx = ui.ctx().clone();
    let Some(mut pad) = ctx.data(|d| d.get_temp::<Pad>(pad_id())) else { return };
    let anchor = boxes.get(pad.index).copied().flatten();
    let (Some(anchor), true) = (anchor, t.pts.len() == pad.points && pad.index < t.dims.len()) else {
        ctx.data_mut(|d| d.remove_temp::<Pad>(pad_id()));
        return;
    };
    let tk = Tokens::get();
    let key = vec2(tk.control_height * 2.0, tk.control_height * 1.6);
    let gap = tk.space_3;
    let size = vec2(4.0 * key.x + 3.0 * gap, 4.0 * key.y + 3.0 * gap) + vec2(2.0 * tk.space_8, 2.0 * tk.space_8);
    let screen = ctx.content_rect();
    let below = Pos2::new(anchor.left(), anchor.bottom() + tk.space_3);
    let at = if below.y + size.y <= screen.bottom() { below } else { Pos2::new(anchor.left(), anchor.top() - tk.space_3 - size.y) };
    let at = Pos2::new(at.x.clamp(screen.left(), (screen.right() - size.x).max(screen.left())), at.y.max(screen.top()));
    let mut pressed = None;
    Container::new(&ctx, Surface::Canvas).area(egui::Id::unique("sc_dim_pad_area")).fixed_pos(at).order(egui::Order::Foreground).show(&ctx, |ui| {
        let frame = Container::popup_frame(ui.style()).inner_margin(egui::Margin::same(tk.space_8 as i8));
        Container::show_backing(ui, frame, |ui| {
            let origin = ui.cursor().min;
            ui.allocate_space(size - vec2(2.0 * tk.space_8, 2.0 * tk.space_8));
            for (row, keys) in ROWS.iter().enumerate() {
                for (col, (label, k)) in keys.iter().enumerate() {
                    let min = origin + vec2(col as f32 * (key.x + gap), row as f32 * (key.y + gap));
                    let text = if label.contains('.') && label.len() > 1 { solidcraft_ui_text::tr(label) } else { label };
                    let style = if *k == Key::Ok { ui_kit::Button::Primary } else { ui_kit::Button::Normal };
                    if ui_kit::button_at(ui, &format!("sc_dim_pad:{label}"), Rect::from_min_size(min, key), text, style).clicked() {
                        pressed = Some(*k);
                    }
                }
            }
        });
    });
    let Some(k) = pressed else { return };
    let Some(b) = t.dims.get_mut(pad.index) else { return };
    if press(&mut pad, b, k) {
        ctx.data_mut(|d| d.remove_temp::<Pad>(pad_id()));
        if k == Key::Ok && t.dims.iter().all(|b| b.locked) {
            ctx.data_mut(|d| d.insert_temp(place_id(), true));
        }
    } else {
        ctx.data_mut(|d| d.insert_temp(pad_id(), pad));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_engine::doc::expr::Kind;

    fn pad(value: &str, locked: bool) -> (Pad, DimBox) {
        let mut b = DimBox::new("Length", Kind::Length);
        b.value = value.into();
        b.locked = locked;
        (Pad { index: 0, points: 1, before: (value.into(), locked), fresh: true }, b)
    }

    #[test]
    fn the_first_key_replaces_the_shown_value_and_later_ones_add_to_it() {
        let (mut p, mut b) = pad("42.17", false);
        for k in [Key::Insert('2'), Key::Insert('5'), Key::Insert('.'), Key::Insert('5')] {
            assert!(!press(&mut p, &mut b, k));
        }
        assert_eq!((b.value.as_str(), b.locked), ("25.5", true));
        press(&mut p, &mut b, Key::Backspace);
        assert_eq!(b.value, "25.");
    }

    #[test]
    fn esc_puts_the_box_back_and_clear_then_ok_lets_it_follow_again() {
        let (mut p, mut b) = pad("10", false);
        press(&mut p, &mut b, Key::Insert('7'));
        assert!(press(&mut p, &mut b, Key::Esc));
        assert_eq!((b.value.as_str(), b.locked), ("10", false));

        let (mut p, mut b) = pad("10", true);
        press(&mut p, &mut b, Key::Clear);
        assert!(press(&mut p, &mut b, Key::Ok));
        assert!(!b.locked);
    }
}

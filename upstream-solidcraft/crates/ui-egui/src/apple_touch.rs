//! Host touch routing uses actual egui regions, including menus and resized panels.
use std::cell::{Cell, RefCell};

use egui::{Context, Event, Id, Key, Modifiers, Pos2, Rect, pos2, vec2};
use serde_json::{Value, json};
use solidcraft_engine::geom::Vec3;

use crate::{SolveApp, scenario, viewport};

struct HitState {
    ctx: Context,
    viewport: Option<Rect>,
    controls: Vec<Rect>,
    projection: Option<viewport::Proj>,
}

thread_local! {
    static PENCIL_INPUT: Cell<bool> = const { Cell::new(false) };
    static FINGER_NAVIGATION: Cell<bool> = const { Cell::new(false) };
    static PENDING_ROLL: Cell<f64> = const { Cell::new(0.0) };
    static PENDING_ZOOM: Cell<Option<(f64, Pos2)>> = const { Cell::new(None) };
    static HIT: RefCell<Option<HitState>> = const { RefCell::new(None) };
    static LAYOUT: RefCell<Value> = const { RefCell::new(Value::Null) };
    static NUMBER_FOCUS: Cell<Option<(Id, Rect)>> = const { Cell::new(None) };
    static KEYPAD_RECT: Cell<Option<Rect>> = const { Cell::new(None) };
    static KEYPAD_EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static SYSTEM_KEYBOARD: Cell<Option<Id>> = const { Cell::new(None) };
    static KEYPAD_ENABLED: Cell<bool> = const { Cell::new(false) };
    static FIELD_UNIT: RefCell<Option<(Id, String)>> = const { RefCell::new(None) };
}

/// The unit of a plain value such as "20 mm" or "45 deg"; none for expressions.
fn plain_unit(text: &str) -> Option<String> {
    let text = text.trim();
    let split = text.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))?;
    let (number, unit) = text.split_at(split);
    let unit = unit.trim();
    (number.chars().any(|c| c.is_ascii_digit()) && !unit.is_empty() && unit.chars().all(|c| c.is_alphabetic() || matches!(c, '°' | '"' | '\'')))
        .then(|| unit.to_owned())
}

/// A number field whose text carries its unit. A bare number typed on the pad gets the unit the
/// field had when it was focused ("20 mm", typed "25", becomes "25 mm").
pub fn number_text(response: &egui::Response, value: &mut String) {
    number_input(response);
    if !KEYPAD_ENABLED.with(Cell::get) {
        return;
    }
    if response.has_focus() {
        FIELD_UNIT.with(|unit| {
            let mut unit = unit.borrow_mut();
            if unit.as_ref().is_none_or(|(id, _)| *id != response.id) {
                *unit = Some((response.id, plain_unit(value).unwrap_or_default()));
            }
        });
    } else if response.lost_focus() {
        let unit = FIELD_UNIT.with(|unit| unit.borrow_mut().take_if(|(id, _)| *id == response.id)).map(|(_, unit)| unit);
        let bare = value.chars().any(|c| c.is_ascii_digit()) && value.chars().all(|c| c.is_ascii_digit() || " .+-*/()".contains(c));
        if let Some(unit) = unit.filter(|unit| bare && !unit.is_empty()) {
            *value = format!("{} {unit}", value.trim());
        }
    }
}

/// Marks a number field; while it has focus the iPad shows the ten-key pad instead of the
/// system keyboard.
pub fn number_input(response: &egui::Response) {
    if response.has_focus() {
        NUMBER_FOCUS.with(|focus| focus.set(Some((response.id, response.rect))));
    }
}

/// Feeds the keys tapped on the pad to the focused field, and keeps that field focused while
/// the pad itself is pressed.
pub fn keypad_input(ctx: &Context, raw: &mut egui::RawInput) {
    KEYPAD_ENABLED.with(|enabled| enabled.set(true));
    raw.events.extend(KEYPAD_EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut())));
    let on_pad = KEYPAD_RECT.with(Cell::get).is_some_and(|pad| {
        let points = raw.events.iter().filter_map(|event| match event {
            Event::PointerMoved(pos) | Event::PointerButton { pos, .. } => Some(*pos),
            _ => None,
        });
        points.chain(ctx.input(|i| i.pointer.latest_pos())).any(|pos| pad.contains(pos / ctx.zoom_factor()) || pad.contains(pos))
    });
    ctx.options_mut(|options| {
        options.input_options.surrender_focus_on = if on_pad { egui::SurrenderFocusOn::Never } else { egui::SurrenderFocusOn::Clicks };
    });
}

fn press(key: Key, modifiers: Modifiers) -> [Event; 2] {
    [true, false].map(|pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers })
}

/// The backspace symbol: a left-pointing tag with a cross in it.
fn paint_backspace(ui: &egui::Ui, rect: Rect, color: egui::Color32) {
    let c = rect.center();
    let (w, h) = (26.0, 18.0);
    let (left, right, top, bottom) = (c.x - w / 2.0, c.x + w / 2.0, c.y - h / 2.0, c.y + h / 2.0);
    let tag = vec![pos2(left, c.y), pos2(left + 8.0, top), pos2(right, top), pos2(right, bottom), pos2(left + 8.0, bottom)];
    let stroke = egui::Stroke::new(2.0, color);
    ui.painter().add(egui::Shape::closed_line(tag, stroke));
    let x = c.x + 3.0;
    for (a, b) in [(vec2(-4.0, -4.0), vec2(4.0, 4.0)), (vec2(-4.0, 4.0), vec2(4.0, -4.0))] {
        ui.painter().line_segment([pos2(x, c.y) + a, pos2(x, c.y) + b], stroke);
    }
}

/// The ten-key pad beside a focused number field. Values are expressions ("20 mm", "w/2"),
/// so the pad also offers the arithmetic operators; ABC hands the field to the system keyboard.
fn keypad(ctx: &Context) -> Option<Rect> {
    let focus = NUMBER_FOCUS.with(|focus| focus.take()).filter(|(id, _)| ctx.memory(|m| m.has_focus(*id)));
    let Some((id, field)) = focus else {
        SYSTEM_KEYBOARD.with(|system| system.set(None));
        return None;
    };
    if SYSTEM_KEYBOARD.with(Cell::get) == Some(id) {
        return None;
    }
    SYSTEM_KEYBOARD.with(|system| system.set(None));
    // No text agent focus, so the system keyboard stays down.
    ctx.output_mut(|output| output.ime = None);

    const KEYS: [&str; 20] = ["7", "8", "9", "/", "⌫", "4", "5", "6", "*", "C", "1", "2", "3", "-", "Next", "ABC", "0", ".", "+", "Enter"];
    let key = vec2(60.0, 50.0);
    let gap = 6.0;
    let size = vec2(5.0 * key.x + 6.0 * gap, 4.0 * key.y + 5.0 * gap);
    let screen = ctx.content_rect();
    let below = field.bottom() + gap;
    let y = if below + size.y <= screen.bottom() { below } else { (field.top() - gap - size.y).max(screen.top()) };
    let x = field.left().clamp(screen.left(), (screen.right() - size.x).max(screen.left()));
    let area = egui::Area::new(Id::new("sc_keypad")).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(gap).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            egui::Grid::new("sc_keypad_grid").spacing(vec2(gap, gap)).show(ui, |ui| {
                for (i, label) in KEYS.into_iter().enumerate() {
                    let text = egui::RichText::new(match label { "/" => "÷", "*" => "×", "-" => "−", "⌫" => "", other => other }).size(22.0);
                    let button = ui.add(egui::Button::new(text).min_size(key));
                    if label == "⌫" {
                        paint_backspace(ui, button.rect, ui.style().interact(&button).fg_stroke.color);
                    }
                    if button.clicked() {
                        let events: Vec<Event> = match label {
                            "⌫" => press(Key::Backspace, Modifiers::NONE).into(),
                            "C" => [press(Key::A, Modifiers::COMMAND), press(Key::Backspace, Modifiers::NONE)].concat(),
                            "Next" => press(Key::Tab, Modifiers::NONE).into(),
                            "Enter" => press(Key::Enter, Modifiers::NONE).into(),
                            "ABC" => {
                                SYSTEM_KEYBOARD.with(|system| system.set(Some(id)));
                                Vec::new()
                            }
                            text => vec![Event::Text(text.into())],
                        };
                        KEYPAD_EVENTS.with(|pending| pending.borrow_mut().extend(events));
                        ctx.request_repaint();
                    }
                    if i % 5 == 4 {
                        ui.end_row();
                    }
                }
            });
        });
    });
    Some(area.response.rect)
}

/// The host identifies real Pencil contact separately from translated mouse events.
pub fn set_pencil_input(pencil: bool) {
    PENCIL_INPUT.with(|input| input.set(pencil));
}

pub fn pencil_input() -> bool {
    PENCIL_INPUT.with(Cell::get)
}

/// The host reports when fingers drive the camera; their gestures then follow the iPad Touch
/// preset whatever mouse preset is chosen.
pub fn set_finger_navigation(active: bool) {
    FINGER_NAVIGATION.with(|state| state.set(active));
}

pub fn finger_navigation() -> bool {
    FINGER_NAVIGATION.with(Cell::get)
}

/// A two-finger twist from the host (radians, clockwise on screen); applied on the next frame.
pub fn add_roll(radians: f64) {
    if radians.is_finite() {
        PENDING_ROLL.with(|roll| roll.set(roll.get() + radians));
    }
}

/// A pinch from the host: the change in finger spread (above 1 spreads) about a point in CSS
/// pixels; applied on the next frame.
pub fn add_zoom(ratio: f64, x: f32, y: f32) {
    if ratio.is_finite() && ratio > 0.0 {
        PENDING_ZOOM.with(|zoom| {
            let total = zoom.get().map_or(1.0, |(r, _)| r);
            zoom.set(Some((total * ratio, pos2(x, y))));
        });
    }
}

/// Zoom the view by the pinch gathered since the last frame. Spreading zooms in, toward the
/// model point between the fingers.
pub fn apply_zoom(app: &mut SolveApp, zoom_factor: f32) -> bool {
    let Some((ratio, at)) = PENDING_ZOOM.with(|zoom| zoom.take()) else { return false };
    let Some(rect) = app.viewport.rect else { return false };
    let at = at / zoom_factor;
    let f = ratio.powf(-crate::prefs::zoom_sign(app)).clamp(0.05, 20.0);
    let anchor = crate::navigation::zoom_anchor(app, rect, rect.contains(at).then_some(at), true);
    app.viewport.orbit_free = None;
    app.cancel_view_animation();
    app.cam.zoom_at(f, anchor);
    true
}

/// Roll the view by the twist gathered since the last frame.
pub fn apply_roll(app: &mut SolveApp) -> bool {
    let radians = PENDING_ROLL.with(|roll| roll.replace(0.0));
    if radians == 0.0 {
        return false;
    }
    app.cancel_view_animation();
    app.cam.roll_by(radians);
    true
}

fn rect_json(rect: Rect) -> Value {
    json!([rect.left(), rect.top(), rect.width(), rect.height()])
}

pub fn update(app: &SolveApp, ctx: &Context) {
    let pad = keypad(ctx);
    KEYPAD_RECT.with(|rect| rect.set(pad));
    let view = if app.home.open { None } else { app.viewport.rect };
    // Floating shell cards (ribbon, History, browser, palettes, dialogs) sit over the full-bleed
    // viewport on its own layer, so they are listed explicitly rather than found by layer.
    let mut controls: Vec<_> = ["nav:outer", "axis_view:outer"].into_iter().filter_map(scenario::rect_of).collect();
    controls.extend(crate::shell::covered_rects(app));
    controls.extend(pad);
    HIT.with(|h| {
        *h.borrow_mut() = Some(HitState { ctx: ctx.clone(), viewport: view, controls, projection: view.map(|r| viewport::projection(app, r)) })
    });
    let handles: serde_json::Map<String, Value> = [
        "ribbon:section:INSERT:2D",
        "dropdown:sketch.insert_dxf",
        "browser:card",
        "browser:header",
        "nav:outer",
        "nav:fit",
        "nav:lookat",
        "nav:home",
        "nav:perspective",
        "nav:style",
        "nav:settings",
        "axis_view:outer",
    ]
    .into_iter()
    .filter_map(|name| scenario::rect_of(name).map(|r| (name.into(), rect_json(r))))
    .collect();
    let state = app.session.model.state();
    let mesh_bodies: Vec<_> = state.bodies.iter().filter(|b| b.body.is_mesh()).collect();
    LAYOUT.with(|layout| {
        *layout.borrow_mut() = json!({
            "mesh_body_count": mesh_bodies.len(),
            "mesh_body_name": mesh_bodies.first().map(|b| b.name.clone()),
            "cad_shell_count": state.bodies.iter().filter(|b| !b.body.is_mesh()).map(|b| b.body.shell_count()).sum::<usize>(),
            "viewport": view.map(rect_json), "zoom": ctx.zoom_factor(), "rects": handles,
            "browser_collapsed": app.tree.panel_collapsed, "browser_width": app.preferences.browser_width,
            "imported_sketch_count": app.session.doc.features.iter().filter(|f| f.name.starts_with("DXF ")).count(),
            "imported_curve_count": app.session.doc.features.iter().filter(|f| f.name.starts_with("DXF ")).map(|f| match &f.kind {solidcraft_engine::doc::FeatureKind::Sketch{sketch,..} => sketch.curves.len(), _ => 0}).sum::<usize>(),
            "active_sketch": app.session.active_sketch,
            "selection_count": app.session.selection.len(), "camera": app.cam,
        })
    });
}

pub fn layout() -> String {
    LAYOUT.with(|layout| layout.borrow().to_string())
}

/// Host actions can arrive while the web painter is idle.
pub fn request_repaint() {
    HIT.with(|h| {
        if let Some(h) = h.borrow().as_ref() {
            h.ctx.request_repaint();
        }
    });
}

/// A host can place an overlay or test input at a model coordinate using the real camera.
pub fn project(x: f64, y: f64, z: f64) -> String {
    if !x.is_finite() || !y.is_finite() || !z.is_finite() {
        return "null".into();
    }
    HIT.with(|h| {
        let h = h.borrow();
        h.as_ref()
            .and_then(|h| h.projection.as_ref().and_then(|p| p.to_screen(Vec3::new(x, y, z))).map(|p| p * h.ctx.zoom_factor()))
            .map(|p| json!([p.x, p.y]).to_string())
            .unwrap_or_else(|| "null".into())
    })
}

/// Coordinates are CSS points relative to the canvas; user zoom is applied here.
pub fn is_model_point(x: f32, y: f32) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    HIT.with(|h| {
        let h = h.borrow();
        let Some(h) = h.as_ref() else { return false };
        let p = pos2(x, y) / h.ctx.zoom_factor();
        h.viewport.is_some_and(|r| r.contains(p))
            && !h.controls.iter().any(|r| r.contains(p))
            && !h.ctx.layer_id_at(p).is_some_and(|layer| matches!(layer.order, egui::Order::Middle | egui::Order::Foreground))
    })
}

/// Align to a face without selecting it or changing the active modelling tool.
pub fn align_face(app: &mut SolveApp, point: Pos2) -> bool {
    let Some(rect) = app.viewport.rect.filter(|r| r.contains(point)) else { return false };
    let proj = viewport::projection(app, rect);
    let hit = viewport::pick_3d(app, &proj, point).into_iter().find_map(|hit| match hit {
        viewport::Hit::Face { body, index, point } => Some((body, index, point)),
        _ => None,
    });
    let Some((body, index, at)) = hit else { return false };
    let target = crate::dialogs::planar_face(&app.session, &body, index).or_else(|| {
        let state = app.session.world_state();
        let mesh = state.body(&body)?.mesh();
        let mut nearest: Option<(f64, Vec3)> = None;
        for (i, triangle) in mesh.triangles.iter().enumerate() {
            if mesh.tri_face.get(i).copied().map(|face| face as usize) != Some(index) {
                continue;
            }
            let (Some(a), Some(b), Some(c)) =
                (mesh.positions.get(triangle[0] as usize), mesh.positions.get(triangle[1] as usize), mesh.positions.get(triangle[2] as usize))
            else {
                continue;
            };
            let Some(normal) = (*b - *a).cross(*c - *a).normalized() else { continue };
            let distance = ((*a + *b + *c) / 3.0).dist(at);
            if nearest.is_none_or(|(best, _)| distance < best) {
                nearest = Some((distance, normal));
            }
        }
        nearest.map(|(_, normal)| (at, normal))
    });
    let Some((centre, normal)) = target else { return false };
    let mut to = app.cam.looking_from(normal);
    to.target = centre;
    to.fov = 0.0;
    app.ui.perspective = false;
    app.animate_to(to);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Harness;

    fn sample() -> Harness {
        let mut h = Harness::new();
        h.app.ui.apple_touch = true;
        h.app.home.open = false;
        h.app.preferences.look_at_anim = false;
        h.app.run("solid.box", json!({"length":40,"width":30,"height":10})).unwrap();
        h.call("ui.view", json!({"view":"iso"}));
        h.settle();
        h.frames(3);
        h
    }

    #[test]
    fn apple_touch_controls_are_above_model_and_do_not_pick() {
        let h = sample();
        update(&h.app, &h.ctx);
        let view = h.app.viewport.rect.unwrap();
        let nav = scenario::rect_of("nav:outer").unwrap();
        assert!(nav.top() <= view.top() + 8.0);
        assert!(nav.bottom() < view.bottom() - 100.0);
        assert!(!is_model_point(nav.center().x, nav.center().y));
        let axes = scenario::rect_of("axis_view:outer").unwrap();
        assert!(!axes.intersect(nav).is_positive());
        assert!(!is_model_point(axes.center().x, axes.center().y));
        assert!(!is_model_point(100.0, view.center().y));
        assert!(is_model_point(view.center().x, view.center().y));
        for icon in ["fit", "lookat", "home", "perspective", "style", "settings"] {
            assert!(scenario::rect_of(&format!("nav:{icon}")).unwrap().width() >= 40.0);
        }
    }

    #[test]
    fn apple_touch_face_alignment_preserves_selection() {
        let mut h = sample();
        let at = h.at(&json!({"world":[20,15,10]})).unwrap();
        let selection = h.app.session.selection.clone();
        assert!(align_face(&mut h.app, pos2(at[0] as f32, at[1] as f32)));
        assert!(h.app.cam.back().dot(Vec3::Z) > 0.9999);
        assert_eq!(h.app.cam.fov, 0.0);
        assert_eq!(h.app.session.selection, selection);
    }
}

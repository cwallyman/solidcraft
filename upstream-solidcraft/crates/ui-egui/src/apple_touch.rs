//! Host touch routing uses actual egui regions, including menus and resized panels.
use std::cell::{Cell, RefCell};

use egui::{Context, Pos2, Rect, pos2};
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
    static HIT: RefCell<Option<HitState>> = const { RefCell::new(None) };
    static LAYOUT: RefCell<Value> = const { RefCell::new(Value::Null) };
}

/// The host identifies real Pencil contact separately from translated mouse events.
pub fn set_pencil_input(pencil: bool) {
    PENCIL_INPUT.with(|input| input.set(pencil));
}

pub fn pencil_input() -> bool {
    PENCIL_INPUT.with(Cell::get)
}

fn rect_json(rect: Rect) -> Value {
    json!([rect.left(), rect.top(), rect.width(), rect.height()])
}

pub fn update(app: &SolveApp, ctx: &Context) {
    let view = if app.home.open { None } else { app.viewport.rect };
    let controls: Vec<_> = ["nav:outer", "axis_view:outer"].into_iter().filter_map(scenario::rect_of).collect();
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

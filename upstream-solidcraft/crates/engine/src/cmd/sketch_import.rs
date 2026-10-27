//! Sketch drawing exchange: Insert DXF, Insert SVG and sketch DXF export.

use std::collections::HashMap;

use serde_json::{Value, json};
use solidcraft_geom::Vec2;
use solidcraft_io::{Geom2, MAX_DRAWING_BYTES};
use solidcraft_sketch::{CurveKind, LinkKind, LinkSource, Sketch};

use super::CommandSpec;
use super::sketch::{edit, ids_of};
use crate::params::{bad, num, str_, vec2};
use crate::{EngineError, Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("sketch.insert_dxf", "Insert DXF", insert_dxf)
        .at("SKETCH", "INSERT")
        .icon("sketch_dxf")
        .params(concat!("path | text: ASCII DXF (blocks expanded); at?: [x,y] offset, scale?, tolerance?: mm within which ends join (default 0.0001); plane?: (when no sketch is active, a new sketch on it; default XY)", created_doc!())),
    CommandSpec::new("sketch.insert_svg", "Insert SVG", insert_svg)
        .at("SKETCH", "INSERT")
        .icon("svg")
        .params(concat!("path | text: SVG; at?: [x,y] offset, scale?, tolerance?: mm within which ends join (default 0.0001); plane?: (when no sketch is active, a new sketch on it; default XY)", created_doc!())),
    CommandSpec::new("sketch.export_dxf", "Save As DXF", export_dxf).icon("sketch_dxf")
        .noundo()
        .params("sketch?: id|name (default active); path?: file to write (else the DXF text is returned)"),
];

/// The sketch import command for a drawing file name, regardless of extension case.
pub fn drawing_import_command(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path).extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("dxf") {
        Some("sketch.insert_dxf")
    } else if ext.eq_ignore_ascii_case("svg") {
        Some("sketch.insert_svg")
    } else {
        None
    }
}

fn source_text(p: &Value, cmd: &str) -> Result<String> {
    if let Some(t) = str_(p, "text") {
        if t.len() > MAX_DRAWING_BYTES {
            return Err(bad(cmd, "the drawing is too large"));
        }
        return Ok(t.to_string());
    }
    let path = str_(p, "path").filter(|x| !x.trim().is_empty() && x.len() < 4096).ok_or_else(|| bad(cmd, "give `path` or `text`"))?;
    let bytes = solidcraft_io::vfs::read(path, MAX_DRAWING_BYTES).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

/// Add imported geometry to a sketch, sharing end points that meet.
/// Adds the geometry; end points closer than `q` (mm) become one point, so drawings whose
/// ends almost meet still make closed profiles.
fn add_geometry(sk: &mut Sketch, geom: &[Geom2], f: &dyn Fn(Vec2) -> Vec2, k: f64, q: f64) -> Result<(Vec<usize>, Vec<Geom2>)> {
    let mut map: HashMap<(i64, i64), Vec<(Vec2, usize)>> = HashMap::new();
    let mut pt = |sk: &mut Sketch, p: Vec2| -> Result<usize> {
        let p = f(p);
        let key = ((p.x / q).floor() as i64, (p.y / q).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = map.get(&(key.0 + dx, key.1 + dy))
                    && let Some((_, i)) = v.iter().find(|(o, _)| o.dist(p) <= q)
                {
                    return Ok(*i);
                }
            }
        }
        let i = sk.add_point(p, None)?;
        map.entry(key).or_default().push((p, i));
        Ok(i)
    };
    let mut curves = Vec::new();
    let mut texts = Vec::new();
    for g in geom {
        let made = match g {
            Geom2::Point(p) => {
                pt(sk, *p)?;
                None
            }
            Geom2::Line(a, b) => {
                let (a, b) = (pt(sk, *a)?, pt(sk, *b)?);
                (a != b).then_some(CurveKind::Line { a, b })
            }
            Geom2::Circle(c, r) => Some(CurveKind::Circle { c: pt(sk, *c)?, r: r * k }),
            Geom2::Arc { c, a, b } => {
                let (c, a, b) = (pt(sk, *c)?, pt(sk, *a)?, pt(sk, *b)?);
                (a != b && c != a).then_some(CurveKind::Arc { c, a, b })
            }
            Geom2::Ellipse { c, major, minor } => Some(CurveKind::Ellipse { c: pt(sk, *c)?, m: pt(sk, *major)?, r: minor * k }),
            Geom2::Spline { pts, control, degree } => {
                let mut idx = Vec::new();
                for p in pts.iter().take(solidcraft_sketch::MAX_SPLINE_POINTS) {
                    let i = pt(sk, *p)?;
                    if idx.last() != Some(&i) {
                        idx.push(i);
                    }
                }
                (idx.len() >= 2).then_some(CurveKind::Spline { pts: idx, control: *control, degree: *degree, periodic: false })
            }
            Geom2::Conic { a, apex, b, rho } => {
                let (a, x, b) = (pt(sk, *a)?, pt(sk, *apex)?, pt(sk, *b)?);
                (a != b && x != a && x != b).then_some(CurveKind::Conic { a, b, apex: x, rho: *rho })
            }
            Geom2::Text { text, at, height, angle } => {
                texts.push(Geom2::Text { text: text.clone(), at: f(*at), height: height * k, angle: *angle });
                None
            }
        };
        if let Some(kind) = made {
            curves.push(sk.add_curve(kind, None)?);
        }
    }
    Ok((curves, texts))
}

fn insert(s: &mut Session, p: &Value, cmd: &str, svg: bool) -> Result<Value> {
    let text = source_text(p, cmd)?;
    let geom = if svg { solidcraft_io::read_svg(&text) } else { solidcraft_io::read_dxf(&text) }.map_err(|e| bad(cmd, e.to_string()))?;
    let point_budget = geom.iter().fold(0usize, |n, g| n.saturating_add(geometry_points(g)));
    let existing = s.active_sketch.and_then(|id| s.doc.sketch(id).ok()).map(|sk| sk.points.len()).unwrap_or(0);
    if !svg && point_budget.saturating_add(existing) > solidcraft_sketch::MAX_POINTS {
        return insert_layered(s, p, cmd, &text);
    }
    if geom.is_empty() {
        return Err(bad(cmd, "the drawing has no geometry SolidCraft reads"));
    }
    let k = num(p, "scale").filter(|k| *k > 1e-9 && *k < 1e6).unwrap_or(1.0);
    let off = p.get("at").and_then(vec2).unwrap_or(Vec2::ZERO);
    let mut created = None;
    if s.active_sketch.is_none() {
        let mut cp = json!({"plane": p.get("plane").cloned().unwrap_or(json!("XY")), "project_edges": false});
        if let Some(n) = str_(p, "name") {
            cp["name"] = json!(n);
        }
        let r = (super::find_command("sketch.create").ok_or_else(|| bad(cmd, "sketch.create"))?.run)(s, &cp)?;
        created = r.get("sketch").and_then(Value::as_u64);
    }
    let tol = num(p, "tolerance").filter(|t| t.is_finite() && *t > 0.0).unwrap_or(1e-4).clamp(1e-9, 10.0);
    let f = move |q: Vec2| q * k + off;
    let ((curves, texts), info) = edit(s, p, cmd, false, |sk, _| {
        let (cs, texts) = add_geometry(sk, &geom, &f, k, tol)?;
        Ok((ids_of(sk, &cs), texts))
    })?;
    // Text becomes sketch text.
    let id = s.active_sketch.ok_or_else(|| bad(cmd, "no sketch"))?;
    let mut n_text = 0;
    for t in texts {
        if let Geom2::Text { text, at, height, angle } = t {
            let mut doc = (*s.doc).clone();
            let mut sk = doc.sketch(id)?.clone();
            if super::sketch_project::add_link(
                s,
                &doc,
                id,
                &mut sk,
                LinkKind::Text,
                LinkSource::Text { text, at, height, angle, font: None, style: Default::default(), exprs: Default::default() },
            )
            .is_ok()
            {
                *doc.sketch_mut(id)? = sk;
                *s.doc_mut() = doc;
                n_text += 1;
            }
        }
    }
    Ok(json!({"sketch": id, "created_sketch": created, "curves": curves.len(), "texts": n_text, "info": info}))
}

/// Conservative upper bound on new points, before coincident ends are merged.
fn geometry_points(g: &Geom2) -> usize {
    match g {
        Geom2::Point(_) | Geom2::Circle(..) => 1,
        Geom2::Line(..) | Geom2::Ellipse { .. } => 2,
        Geom2::Arc { .. } | Geom2::Conic { .. } => 3,
        Geom2::Spline { pts, .. } => pts.len(),
        Geom2::Text { .. } => solidcraft_sketch::MAX_POINTS,
    }
}

/// Large drawings remain editable and keep their coordinates and units. Each
/// layer gets separate sketches, with bounded parts when necessary. No entities
/// are dropped to fit a sketch. The outer Session command remains one undo step.
fn insert_layered(s: &mut Session, p: &Value, cmd: &str, text: &str) -> Result<Value> {
    const POINT_BUDGET: usize = 7_999; // Leave room for the sketch origin.
    const MAX_PARTS: usize = 256;
    let layers = solidcraft_io::read_dxf_layers(text).map_err(|e| bad(cmd, e.to_string()))?;
    let layer_count = layers.len();
    let mut parts: Vec<(String, Vec<Geom2>)> = Vec::new();
    for (layer, geometry) in layers {
        let mut current = Vec::new();
        let mut points = 0usize;
        let mut part = 1usize;
        for g in geometry {
            let budget = geometry_points(&g);
            if !current.is_empty() && points.saturating_add(budget) > POINT_BUDGET {
                parts.push((format!("DXF {layer} ({part})"), std::mem::take(&mut current)));
                part += 1;
                points = 0;
            }
            points = points.saturating_add(budget);
            current.push(g);
            if parts.len() >= MAX_PARTS {
                return Err(bad(cmd, "drawing needs more than 256 sketch parts; import a smaller drawing"));
            }
        }
        if !current.is_empty() {
            parts.push((format!("DXF {layer} ({part})"), current));
        }
        if parts.len() > MAX_PARTS {
            return Err(bad(cmd, "drawing needs more than 256 sketch parts; import a smaller drawing"));
        }
    }
    let k = num(p, "scale").filter(|k| *k > 1e-9 && *k < 1e6).unwrap_or(1.0);
    let off = p.get("at").and_then(vec2).unwrap_or(Vec2::ZERO);
    let tol = num(p, "tolerance").filter(|t| t.is_finite() && *t > 0.0).unwrap_or(1e-4).clamp(1e-9, 10.0);
    let prior = s.active_sketch;
    let plane = prior.and_then(|id| s.doc.feature(id)).and_then(|f| match &f.kind {
        solidcraft_doc::FeatureKind::Sketch { plane, .. } => Some(plane.clone()),
        _ => None,
    });
    let create = super::find_command("sketch.create").ok_or_else(|| bad(cmd, "sketch.create"))?;
    let finish = super::find_command("sketch.finish").ok_or_else(|| bad(cmd, "sketch.finish"))?;
    if prior.is_some() {
        (finish.run)(s, &json!({}))?;
    }
    let mut sketches = Vec::new();
    let mut curves = 0usize;
    let mut entities = 0usize;
    for (name, geometry) in parts {
        let r = (create.run)(s, &json!({"plane":p.get("plane").cloned().unwrap_or(json!("XY")),"name":name,"project_edges":false}))?;
        let id = r["sketch"].as_u64().ok_or_else(|| bad(cmd, "sketch creation returned no id"))?;
        if let Some(plane) = &plane
            && let Some(f) = s.doc_mut().feature_mut(id)
            && let solidcraft_doc::FeatureKind::Sketch { plane: target, .. } = &mut f.kind
        {
            *target = plane.clone();
        }
        let ((added, texts), _) = edit(s, &json!({}), cmd, false, |sk, _| add_geometry(sk, &geometry, &|q| q * k + off, k, tol))?;
        curves += added.len();
        entities += geometry.len();
        for t in texts {
            if let Geom2::Text { text, at, height, angle } = t {
                let mut doc = (*s.doc).clone();
                let mut sk = doc.sketch(id)?.clone();
                super::sketch_project::add_link(s, &doc, id, &mut sk, LinkKind::Text, LinkSource::Text { text, at, height, angle, font: None, style: Default::default(), exprs: Default::default() })?;
                *doc.sketch_mut(id)? = sk;
                *s.doc_mut() = doc;
            }
        }
        (finish.run)(s, &json!({}))?;
        sketches.push(id);
    }
    // Leave every imported part visible and restore the user's active sketch.
    s.active_sketch = prior;
    s.visibility.shown_sketches.extend(sketches.iter().copied());
    let mut warnings = vec![format!(
        "Large DXF imported as {} editable sketch parts from {layer_count} layers. Parts retain drawing coordinates; profiles crossing parts must be joined before extrusion.",
        sketches.len()
    )];
    let unsupported = solidcraft_io::dxf_unsupported_entities(text).map_err(|e| bad(cmd, e.to_string()))?;
    if !unsupported.is_empty() {
        warnings.push(format!(
            "Not imported (unsupported): {}.",
            unsupported.iter().map(|(kind, count)| format!("{count} {kind}")).collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(json!({"sketches":sketches,"layers":layer_count,"curves":curves,"entities":entities,"warnings":warnings}))
}

fn insert_dxf(s: &mut Session, p: &Value) -> Result<Value> {
    insert(s, p, "sketch.insert_dxf", false)
}

fn insert_svg(s: &mut Session, p: &Value) -> Result<Value> {
    insert(s, p, "sketch.insert_svg", true)
}

fn export_dxf(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.export_dxf";
    let id = match p.get("sketch") {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(x)) => s.doc.find_feature(x).map(|f| f.id),
        _ => s.active_sketch,
    }
    .ok_or_else(|| bad(cmd, "no such sketch (and none is active)"))?;
    let st = s.model.state();
    let ss = st.sketch(id).ok_or_else(|| EngineError::Other("that sketch is not evaluated".into()))?;
    let sk = &ss.sketch;
    let mut geom = Vec::new();
    for (i, c) in sk.curves.iter().enumerate() {
        if c.construction {
            continue;
        }
        let pt = |k: usize| sk.point(k).unwrap_or_default();
        match &c.kind {
            CurveKind::Line { a, b } => geom.push(Geom2::Line(pt(*a), pt(*b))),
            CurveKind::Circle { c, r } => geom.push(Geom2::Circle(pt(*c), *r)),
            CurveKind::Arc { c, a, b } => geom.push(Geom2::Arc { c: pt(*c), a: pt(*a), b: pt(*b) }),
            _ => {
                let poly = sk.polyline(i);
                geom.extend(poly.windows(2).map(|w| Geom2::Line(w[0], w[1])));
            }
        }
    }
    let text = solidcraft_io::write_dxf(&geom);
    if let Some(path) = str_(p, "path").filter(|x| !x.trim().is_empty()) {
        solidcraft_io::vfs::write(path, text.as_bytes()).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
        return Ok(json!({"path": path, "entities": geom.len()}));
    }
    Ok(json!({"dxf": text, "entities": geom.len()}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn large_dxf_keeps_all_geometry_in_bounded_layer_parts() {
        let mut dxf = "0\nSECTION\n2\nENTITIES\n".to_string();
        for i in 0..11_000 {
            dxf.push_str(&format!("0\nLINE\n8\nLayer{}\n10\n{}\n20\n0\n11\n{}\n21\n1\n", i % 2, i * 2, i * 2 + 1));
        }
        dxf.push_str("0\nENDSEC\n0\nEOF\n");
        let mut s = Session::default();
        let prior = s.execute("sketch.create", &json!({"plane":"XY","offset":12})).unwrap()["sketch"].as_u64().unwrap();
        let result = s.execute("sketch.insert_dxf", &json!({"text":dxf})).unwrap();
        assert_eq!(s.active_sketch, Some(prior));
        assert_eq!(result["curves"], 11_000);
        assert_eq!(result["layers"], 2);
        let ids = result["sketches"].as_array().unwrap();
        assert_eq!(ids.len(), 4);
        let mut points = 0;
        for id in ids {
            let id = id.as_u64().unwrap();
            let sk = s.doc.sketch(id).unwrap();
            assert!(sk.points.len() <= 8_000);
            points += sk.points.len();
            assert!(s.visibility.shown_sketches.contains(&id));
            let original = s.doc.feature(prior).unwrap();
            let part = s.doc.feature(id).unwrap();
            if let (solidcraft_doc::FeatureKind::Sketch { plane: a, .. }, solidcraft_doc::FeatureKind::Sketch { plane: b, .. }) =
                (&original.kind, &part.kind)
            {
                assert_eq!(a, b);
            }
        }
        assert_eq!(points, 22_004); // Four sketch origins plus every imported point.
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.features.len(), 1);
    }

    #[test]
    fn dxf_and_svg_round_trip_into_sketches() {
        let mut s = Session::default();
        s.execute("sketch.create", &json!({"plane": "XY"})).unwrap();
        s.execute("sketch.rectangle.two_point", &json!({"p0": [0, 0], "p1": [30, 20]})).unwrap();
        s.execute("sketch.circle.center", &json!({"center": [10, 10], "radius": 4})).unwrap();
        let out = s.execute("sketch.export_dxf", &json!({})).unwrap();
        let dxf = out["dxf"].as_str().unwrap().to_string();
        s.execute("sketch.finish", &json!({})).unwrap();
        // Insert it again as a new sketch, offset: same profiles.
        let r = s.execute("sketch.insert_dxf", &json!({"text": dxf, "at": [100, 0]})).unwrap();
        assert_eq!(r["curves"], 5, "{r}");
        let id = r["sketch"].as_u64().unwrap();
        let ss = s.model.state().sketch(id).unwrap().clone();
        let mut areas: Vec<f64> = ss.profiles.iter().map(|p| p.area).collect();
        areas.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(areas.len(), 2, "{areas:?}");
        assert!((areas[1] - (600.0 - std::f64::consts::PI * 16.0)).abs() < 1e-6, "{areas:?}");
        // SVG with a path and text, into the active sketch.
        let svg =
            r#"<svg width="40mm" height="40mm" viewBox="0 0 40 40"><path d="M0 0 H40 V40 H0 Z"/><text x="5" y="20" font-size="10">Hi</text></svg>"#;
        let r = s.execute("sketch.insert_svg", &json!({"text": svg})).unwrap();
        assert_eq!(r["curves"], 4, "{r}");
        assert_eq!(r["texts"], 1, "{r}");
        assert!(s.execute("sketch.insert_svg", &json!({"text": "<svg></svg>"})).is_err());
        assert!(s.execute("sketch.insert_dxf", &json!({"path": "/nonexistent.dxf"})).is_err());
        s.execute("sketch.finish", &json!({})).unwrap();
        // Ends that miss by 0.00005 still close the square; with a tighter tolerance they don't.
        let gappy = "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nLINE\n10\n10.00005\n20\n0\n11\n10\n21\n10\n0\nLINE\n10\n10\n20\n10\n11\n0\n21\n10\n0\nLINE\n10\n0\n20\n10.00003\n11\n0\n21\n0.00004\n0\nENDSEC\n0\nEOF\n";
        for (tol, n) in [(1e-4, 1), (1e-6, 0)] {
            let r = s.execute("sketch.insert_dxf", &json!({"text": gappy, "tolerance": tol})).unwrap();
            let ss = s.model.state().sketch(r["sketch"].as_u64().unwrap()).unwrap().clone();
            assert_eq!(ss.profiles.len(), n, "tolerance {tol}");
            s.execute("sketch.finish", &json!({})).unwrap();
        }
    }

    /// A degree-9 SPLINE (solidcraft#5269) with fewer control points than its degree needs still
    /// comes in as one spline over all its points, which the sketch draws at the degree they allow.
    #[test]
    fn a_degree_nine_spline_with_few_control_points_comes_in_whole() {
        let mut s = Session::default();
        for n in 2..=10 {
            let points: String = (0..n).map(|i| format!("10\n{i}\n20\n{}\n", i % 2)).collect();
            let dxf = format!("0\nSECTION\n2\nENTITIES\n0\nSPLINE\n71\n9\n{points}0\nENDSEC\n0\nEOF\n");
            let r = s.execute("sketch.insert_dxf", &json!({"text": dxf})).unwrap();
            assert_eq!(r["curves"], 1, "{n} points: {r}");
            s.execute("sketch.finish", &json!({})).unwrap();
        }
    }
}

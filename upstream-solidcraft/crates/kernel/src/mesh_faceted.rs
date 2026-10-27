//! Conservative faceted reconstruction: deduplicate faces, separate touching
//! shells through manifold edges, certify each shell, and retain all geometry.
use std::collections::{BTreeMap, BTreeSet};

use crate::{Body, KernelError, Result};

fn invalid(message: &str) -> KernelError {
    KernelError::Invalid(format!("faceted mesh conversion: {message}"))
}

pub fn convert_faceted_mesh(body: &Body, tolerance: f64) -> Result<Vec<Body>> {
    crate::guard("faceted mesh conversion", || {
        if !tolerance.is_finite() || !(1e-6..=1.).contains(&tolerance) {
            return Err(invalid("tolerance must be between 0.000001 and 1 mm"));
        }
        let input = body.mesh.as_ref().ok_or_else(|| invalid("select a mesh body"))?;
        if input.triangles.len() > 25_000 {
            return Err(invalid("mesh exceeds the 25000-triangle faceted conversion budget"));
        }
        let welded = crate::mesh_convert::weld(input, tolerance)?;
        let mesh = welded.mesh.as_ref().ok_or_else(|| invalid("empty mesh"))?;
        let mut unique = BTreeSet::new();
        let triangles: Vec<_> = mesh
            .triangles
            .iter()
            .copied()
            .filter(|t| {
                let mut key = *t;
                key.sort();
                unique.insert(key)
            })
            .collect();
        let mut edges = BTreeMap::<(u32, u32), Vec<usize>>::new();
        for (i, t) in triangles.iter().enumerate() {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                edges.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        let mut adjacency = vec![Vec::new(); triangles.len()];
        for uses in edges.values() {
            if let [a, b] = uses.as_slice() {
                if let Some(v) = adjacency.get_mut(*a) {
                    v.push(*b);
                }
                if let Some(v) = adjacency.get_mut(*b) {
                    v.push(*a);
                }
            }
        }
        let mut visited = vec![false; triangles.len()];
        let mut shells = Vec::new();
        let mut shell_bounds: Vec<(solidcraft_geom::Aabb3, f64)> = Vec::new();
        for start in 0..triangles.len() {
            if visited.get(start).copied().unwrap_or(true) {
                continue;
            }
            let mut stack = vec![start];
            let mut group = Vec::new();
            if let Some(v) = visited.get_mut(start) {
                *v = true;
            }
            while let Some(i) = stack.pop() {
                if let Some(t) = triangles.get(i) {
                    group.push(*t);
                }
                for &next in adjacency.get(i).into_iter().flatten() {
                    if let Some(v) = visited.get_mut(next)
                        && !*v
                    {
                        *v = true;
                        stack.push(next);
                    }
                }
            }
            if shells.len() >= 256 {
                return Err(invalid("mesh exceeds the 256-shell conversion budget"));
            }
            let mut counts = BTreeMap::<(u32, u32), (usize, i32)>::new();
            for t in &group {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let v = counts.entry((a.min(b), a.max(b))).or_default();
                    v.0 += 1;
                    v.1 += if a < b { 1 } else { -1 };
                }
            }
            let open = counts.values().filter(|v| v.0 == 1).count();
            let nonmanifold = counts.values().filter(|v| v.0 > 2).count();
            if open > 0 || nonmanifold > 0 {
                return Err(invalid(&format!("shell has {open} open edges and {nonmanifold} non-manifold edges; no geometry was converted")));
            }
            let volume6: f64 = group
                .iter()
                .filter_map(|t| Some((mesh.positions.get(t[0] as usize)?, mesh.positions.get(t[1] as usize)?, mesh.positions.get(t[2] as usize)?)))
                .map(|(a, b, c)| a.dot(b.cross(*c)))
                .sum();
            let mut bounds = solidcraft_geom::Aabb3::EMPTY;
            for t in &group {
                for &vertex in t {
                    if let Some(&point) = mesh.positions.get(vertex as usize) {
                        bounds.add(point);
                    }
                }
            }
            let contains = |a: solidcraft_geom::Aabb3, b: solidcraft_geom::Aabb3| {
                a.min.x <= b.min.x && a.min.y <= b.min.y && a.min.z <= b.min.z && a.max.x >= b.max.x && a.max.y >= b.max.y && a.max.z >= b.max.z
            };
            // A mixed-direction shell inside another shell's bounds may be a
            // cavity. Refuse it conservatively rather than silently fill it.
            if shell_bounds.iter().any(|(prior, volume)| {
                (*volume > 0. && volume6 < 0. && contains(*prior, bounds)) || (*volume < 0. && volume6 > 0. && contains(bounds, *prior))
            }) {
                return Err(invalid("nested cavities or enclosed inward shells require repair before faceted conversion"));
            }
            shell_bounds.push((bounds, volume6));
            // Normalize winding within each shell using the existing mesh-body path.
            let shell = crate::mesh_body(&mesh.positions, &group)?;
            let shell_mesh = shell.mesh.as_ref().ok_or_else(|| invalid("empty shell"))?;
            if !crate::meshbody::is_closed(shell_mesh) {
                return Err(invalid("shell cannot be consistently oriented; no geometry was converted"));
            }
            let converted = crate::polybool::rebuild_mesh_shell(shell_mesh)?;
            if !converted.validity().is_empty() {
                return Err(invalid("reconstructed shell failed topology validation"));
            }
            shells.push(converted);
        }
        if shells.is_empty() {
            return Err(invalid("empty mesh"));
        }
        Ok(shells)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_geom::Vec3;

    fn boxes(origins: &[Vec3], duplicates: bool) -> Body {
        let mut points = Vec::new();
        let mut triangles = Vec::new();
        for &origin in origins {
            let mesh = crate::box_solid(origin, origin + Vec3::new(1., 1., 1.)).unwrap().tessellate(0.01).unwrap();
            let offset = points.len() as u32;
            points.extend(mesh.positions);
            triangles.extend(mesh.triangles.iter().map(|t| t.map(|v| v + offset)));
        }
        if duplicates {
            triangles.extend(triangles.clone());
        }
        crate::mesh_body(&points, &triangles).unwrap()
    }

    #[test]
    fn repairs_duplicates_and_separates_edge_touching_shells() {
        let source = boxes(&[Vec3::ZERO, Vec3::new(1., 1., 0.)], true);
        let converted = convert_faceted_mesh(&source, 0.05).unwrap();
        assert!(converted.iter().all(|b| !b.is_mesh()));
        assert!(converted.iter().all(|b| b.validity().is_empty()));
        assert_eq!(converted.len(), 2);
        assert_eq!(converted.iter().map(|b| b.faces(0.01).unwrap().len()).sum::<usize>(), 12);
        assert!((converted.iter().map(|b| crate::measure(b).unwrap().volume).sum::<f64>() - 2.).abs() < 1e-6);
        assert!(source.is_mesh());
    }

    #[test]
    fn rejects_open_mesh_and_invalid_tolerances() {
        let source = boxes(&[Vec3::ZERO], false);
        let mesh = source.mesh.as_ref().unwrap();
        let open = crate::mesh_body(&mesh.positions, &mesh.triangles[..mesh.triangles.len() - 1]).unwrap();
        assert!(convert_faceted_mesh(&open, 0.05).unwrap_err().to_string().contains("open edges"));
        for tolerance in [0., 2., f64::NAN, f64::INFINITY] {
            assert!(convert_faceted_mesh(&source, tolerance).is_err());
        }
    }

    #[test]
    fn converts_non_prismatic_tetrahedron() {
        let source = crate::mesh_body(&[Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z], &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]).unwrap();
        let converted = convert_faceted_mesh(&source, 0.05).unwrap();
        assert_eq!(converted[0].faces(0.01).unwrap().len(), 4);
        assert!((crate::measure(&converted[0]).unwrap().volume - 1. / 6.).abs() < 1e-6);
    }

    #[test]
    fn does_not_fill_inward_cavity_shells() {
        let outer = boxes(&[Vec3::ZERO], false);
        let inner = crate::box_solid(Vec3::new(0.25, 0.25, 0.25), Vec3::new(0.75, 0.75, 0.75)).unwrap().tessellate(0.01).unwrap();
        let mesh = outer.mesh.as_ref().unwrap();
        let offset = mesh.positions.len() as u32;
        let mut points = mesh.positions.clone();
        points.extend(inner.positions);
        let mut triangles = mesh.triangles.clone();
        triangles.extend(inner.triangles.iter().map(|t| [t[0] + offset, t[2] + offset, t[1] + offset]));
        let source = crate::mesh_body(&points, &triangles).unwrap();
        assert!(convert_faceted_mesh(&source, 0.05).unwrap_err().to_string().contains("nested cavities"));
    }
}

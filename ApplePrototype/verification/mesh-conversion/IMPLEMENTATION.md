# Faceted mesh conversion

This update adds a Faceted method to MESH → MODIFY → Convert Mesh. Prismatic remains the default and retains its existing constant-section analytic reconstruction behavior.

The new path is written locally in Rust using SolidCraft's existing mesh and planar B-rep builders. It does not embed or call Autodesk software.

## Conversion behavior

1. Validate tolerance and enforce a 25,000-triangle input budget.
2. Weld nearly coincident vertices using the existing conservative weld tolerance.
3. Remove duplicate triangle faces using their welded vertex identities.
4. Traverse adjacency only across edges used by exactly two triangles. Touching parts sharing an edge with four triangles can therefore become separate shells.
5. Require every resulting shell to be closed. Report counts of open/non-manifold edges on failure.
6. Refuse inward shells enclosed by outward shell bounding boxes as potential cavities. This is a conservative check and may also reject enclosed reversed parts.
7. Normalize winding within each accepted shell and orient it outward.
8. Rebuild planar B-rep faces with the existing polygon topology builder. Coplanar triangles merge; curved regions retain planar facets.
9. Validate each rebuilt shell and return it as a separate CAD body under one conversion feature.

Limits are 25,000 input triangles and 256 shells. Invalid input returns an error through the existing transactional command path. The source mesh remains embedded upstream in the timeline; undo restores it. Saved documents record the method, and older conversion features default to Prismatic when the new field is absent.

## What this does not do

- It does not automatically patch holes or guess missing geometry.
- It does not fuse overlapping or touching shells into a single material volume.
- It does not reconstruct cylinders, fillets, or the original design history in Faceted mode.
- It does not support nested inward cavity shells in the faceted repair path.
- It does not make an arbitrary self-intersecting mesh into a certified material solid. Validation establishes closed topology, meshing, and positive volume; it is not a comprehensive geometric intersection audit.

## Source changes

- `crates/kernel/src/mesh_faceted.rs`: new bounded repair/conversion path and analytic regression cases.
- `crates/kernel/src/polybool.rs`: internal wrapper for rebuilding one mesh shell into merged planar faces.
- `crates/kernel/src/mesh_convert.rs`: shares the existing welding helper.
- `crates/kernel/src/lib.rs`: exports the new faceted conversion function.
- `crates/doc/src/document.rs`: backward-compatible stored method flag.
- `crates/doc/src/eval.rs`: evaluates the selected method, creates separately named CAD bodies, and retains source-body color.
- `crates/engine/src/cmd/mesh.rs`: validates the method parameter.
- `crates/engine/src/tests/mesh_convert.rs`: persistence/undo and optional local-STL/export checks.
- `crates/ui-egui/src/dialogs_surface.rs`: Method selector and separate-shell notice.
- `crates/ui-egui/src/apple_touch.rs`: mesh-body and CAD-shell diagnostics for physical tests.
- Apple bridge: reusable palette-command helper for the existing debug workflow checks. The experimental mesh-specific device harness was removed after it encountered recovery/picker state issues.

## Local sample diagnosis

The supplied STL contains 1,100 triangles and 608 exact unique vertices. It has no open boundary edges, but 40 edges have four incident triangles. There are 24 duplicate triangle faces. Removing duplicates leaves four edges with four incident triangles. Splitting at those edges yields 38 individually closed shells. One shell has inward winding.

The original STL is not modified. A separately named copy in the app's Documents folder is used for physical-device testing. Generated STEP and screenshots belong in this verification output folder, outside the upstream source repository.

## Verified native engine results

Five engine conversion tests passed, including default Prismatic behavior, failure atomicity, Faceted method persistence/undo, and the actual STL conversion/STEP round trip. The actual file produced 38 individually closed CAD bodies, 436 faces, and a sum of outward-shell volumes of 216.36805509063853 mm³. A cold design reopen retained the bodies and geometry. This sum is not a fused material-volume measurement.

`locomotive-faceted.step` contains 38 `MANIFOLD_SOLID_BREP` entries, 436 `ADVANCED_FACE` entries, and zero `BREP_WITH_VOIDS` entries. Reimport returned 38 valid bodies with no warnings. An earlier compound-body draft incorrectly exported disconnected shells as voids; the final document evaluator creates separate bodies to avoid that error.

## Device and build verification

- Four final kernel regression tests passed.
- Five final engine conversion tests passed, including the actual STL and STEP round trip.
- Twelve existing JavaScript input regression tests passed.
- The final web engine release build and signed iPad build/install/launch passed.
- The user confirmed the installed converter is working in a hands-on check.
- Automated device conversion tests did not pass: the first attempt was blocked by a recovery prompt, and the subsequent attempt remained in the File menu rather than reaching the native fixture picker. These runs are retained as failure evidence, not successful conversion evidence.
- The recovery attempt reported that `/opfs/recovery.solidcraft` was not UTF-8 JSON. A separate file-save command completed afterward, but that does not prove the original pending recovery document was successfully restored. The general web recovery/container-reading issue remains unresolved.
- The experimental mesh-specific test-launch hooks were removed from source. No further device relaunch or installation was performed after the user reported success, to avoid interrupting the active session.
- Full repository CI remains blocked by existing `nonminimal_bool` lints in `crates/sketch/src/solver.rs:1937` and `crates/kernel/src/coplanar.rs:908`. Format, asset attribution, license and layer checks passed before that failure.
- Remote HEAD was checked as `15b5b51bf5cf51d33d337bc3a4f6e294b8857ece`. No upstream integration, commit, push or PR was performed.

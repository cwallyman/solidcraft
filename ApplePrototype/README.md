# Solidcraft for iPad

Maintained in [cwallyman/solidcraft, branch ipad-solidcraft](https://github.com/cwallyman/solidcraft/tree/ipad-solidcraft). The separate `cwallyman/ipad-solidcraft` repository is an archived backup.

A signed iPad application that bundles a locally compiled SolidCraft WebAssembly CAD engine and customized egui interface in WKWebView. Modeling runs on the device; the app does not fetch the engine from a website. This is a feasibility prototype, not a native SwiftUI CAD editor.

## Run

Open `SolidcraftForIPad.xcodeproj`, select the **SolidcraftForIPad** scheme and your iPad, and run. Automatic signing is configured for the development team already present on this Mac. The bundle ID is `com.cwallyman.SolidcraftForIPad`, separate from the installed desktop app. Builds made before the rename used `com.cwallyman.SolidCraftTest` and install as a separate "SolidCraft Test" app; move its files out and delete it.

## Test workflow

1. The sample plate opens automatically.
2. Drag **one finger in model space to orbit**, drag **two fingers to pan**, pinch to zoom, and twist two fingers to rotate the view. Swipes scroll panels and lists, and a tap outside the File menu or command search closes it. **Finger taps operate the interface, menus, view controls, browser and dialogs. Apple Pencil selects and sketches in model space. Pencil taps accumulate lines, edges and faces until a tap in empty model space clears the selection; tapping an already selected item deselects it while retaining the other picks. Mouse clicks replace the selection; Shift-click adds or removes items.** Double-tap a face with a finger for an orthographic view normal to that face, without selecting it. Orbit waits for 100 ms and at least 6 points of movement so a second finger can land without an initial rotation. Cursor relocation is processed with the navigation button released to avoid a landing jump. Lifting one finger after a two-finger gesture pauses navigation until both fingers lift. Pencil contact suppresses concurrent finger/palm navigation. With a sketch drawing tool selected, press and drag with Pencil to preview a shape, then lift to place its next point. Lines, two-point rectangles, circles and polygons complete on lift; arcs, ellipses and other multi-point tools accept their remaining points with taps or further drags. Separate point-by-point taps still work. Mouse input is supported on iPad.
3. The lower floating navigation bar is removed. Fit, Look At, projection, visual style and settings are larger finger-tappable buttons at the upper right, above the orientation widget. Drag the browser's right divider to resize it. Tap its folder button to collapse to a narrow rail; tap the folder again to reopen. The width and collapse state are saved with preferences.
4. Edit a feature through the timeline, change its dimension, and apply it.
5. Use File → Save to persist the design in the application's WebKit storage.
6. Use File → Open and reopen the saved design.
7. In the file panel use **Download open design**, or export STEP/STL. The native bridge copies the exported bytes into `Documents/Exports/<unique-id>/`. Exports are available in Files under **On My iPad → Solidcraft for iPad**.
8. The native top bar is removed; only the SolidCraft interface remains. Runtime diagnostics are written to the app’s Documents folder.

`SolidcraftForIPadUITests` launches the real application, waits for the WebAssembly runtime, records the rendered model before and after a touch drag, and checks the storage read/write result. Actual coverage and limitations are recorded in `verification/RESULTS.md`.

## Implementation

- `LocalWebServer.swift`: bundle assets served exclusively at `127.0.0.1:18784`. A stable origin preserves WebKit storage across launches. No network server on the Mac is used.
- `input.js`: automatic finger navigation, Pencil tap accumulation, and ordinary mouse selection on iPad.
- `bridge.js`: Blob-download export to native Documents and runtime diagnostics.
- `SolidcraftForIPadApp.swift`: SwiftUI shell, WKWebView, Files sharing, and startup diagnostics.
- `Web/asset-manifest.json`: SHA-256 hashes and original URLs of the bundled upstream build.

The initial binary assets were downloaded from the upstream published web build on October 13, 2026 (hash `a8b5c0f256114b90`). They are backed up in `prebuilt-web-backup/`. The current engine is compiled locally from `upstream-solidcraft`, merged with upstream release v0.24.0, with the touch layout, browser docking, hit-region and face-view changes. `scripts/bundle-local-web.py` bundles that build and records SHA-256 hashes in `Web/asset-manifest.json`. Rust release optimization is enabled; wasm-opt is disabled for quick iteration. `scripts/build-release-web.sh` builds the size-optimized App Store engine and `scripts/archive-release.sh` makes the Release archive. Upstream MIT/Apache licenses, notices, and attribution are bundled in `Web/`. The icon is copied from upstream `assets/icon.png`.

## Boundaries

The CAD layout remains desktop-oriented. This does not establish native iOS Rust rendering (planned in [NATIVE_IOS_PLAN.md](NATIVE_IOS_PLAN.md)), Pencil pressure tools, iCloud synchronization, or production readiness. Pencil routing has automated regression coverage. The user confirmed button-name hover works on the physical iPad; the latest sketch drawing and selection toggling still need explicit hands-on confirmation. On hover-capable iPads, native Pencil hover forwards to the same button tooltips used by the mouse. Toolbar panel labels (Create, Modify, and the other panels) toggle their dropdown closed when tapped again. WebKit owns the saved-design storage; export important work to Files. Upstream periodic recovery has a 30-second interval, so recent unsaved edits can be lost on forced termination. The native export bridge accepts files up to 128 MB. External web navigation is blocked in the prototype.

The application starts with the sample on each cold launch; saved documents are available through File → Open. Native Files import uses the browser's standard file picker. Signatures and provisioning lifetime follow the selected Apple development account.

Open and Insert now use the native Files picker on Apple hosts. DXF (ASCII) and SVG files import as editable sketch geometry using SolidCraft's existing importer; with no active sketch, an XY sketch is created. DWG import is not supported: export ASCII DXF first. The upper-right row includes Home again.

Large DXF drawings automatically import as layer sketches, with oversized layers divided into bounded editable parts. Parts retain original coordinates and units. Profiles crossing part boundaries need joining before extrusion. Unsupported top-level entities (such as HATCH) are reported after a large import.

Convert Mesh now includes a **Faceted** method alongside the existing Prismatic method. Faceted removes duplicate triangles and reconstructs separately closed parts as individual CAD bodies, merging coplanar triangles. It does not fuse parts or fill holes. Input is limited to 25,000 triangles and 256 shells; potential nested inward cavity shells are refused. The source mesh remains in the upstream timeline and undo restores it. See `verification/mesh-conversion/IMPLEMENTATION.md` for details and limits.


The complete implementation handoff, including October 14 hover, dropdown, drag drawing, Pencil/mouse selection and browser-icon changes, is in [IPAD_APP_IMPLEMENTATION.md](IPAD_APP_IMPLEMENTATION.md#20-pencil-mouse-toolbar-and-browser-update--october-14-2026). Revision-specific checks are recorded in [verification/RESULTS.md](verification/RESULTS.md).


See the [iPad screenshot gallery](../docs/screenshots/ipad/README.md) for captured interface examples and their capture methods.

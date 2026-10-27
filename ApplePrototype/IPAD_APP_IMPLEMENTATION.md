# Solidcraft for iPad — complete implementation handoff

Documented October 13, 2026; updated October 14, 2026. This report describes the current local implementation, the changes recorded during development, the screen-layout correction, and the October 14 Pencil/mouse interaction updates. It is based on the source, build scripts, README, and `verification/RESULTS.md`. Historical test results below are recorded results; they were not all rerun when this document was written.

## 1. What we built

We made SolidCraft run as an installed iPad application by wrapping its existing Rust/WebAssembly CAD engine and egui interface in a SwiftUI application containing a WKWebView. The engine, JavaScript, fonts, and supporting assets are bundled inside the app. Modeling runs locally on the iPad.

The app uses a small HTTP server bound to the iPad's own loopback address to load those assets. It does not require a web server running on the Mac or download the CAD engine at startup. The current product remains an Apple feasibility prototype with a desktop-derived CAD interface.

| Item | Current implementation |
| --- | --- |
| App display name | Solidcraft for iPad |
| Bundle identifier | `com.cwallyman.SolidcraftForIPad` |
| Xcode project and scheme | `SolidcraftForIPad.xcodeproj` / `SolidcraftForIPad` |
| Deployment target | iOS 17.0 |
| Device family | iPad |
| Physical development device | iPad Air 11-inch (M3) |
| Rendering path | Bundled WebAssembly, eframe/egui, WebGL2 in WKWebView |
| Local asset origin | `http://127.0.0.1:18784/` |
| Initial design | Built-in sample plate on cold launch |
| Optional Mac target | Mac Catalyst |

The bundle identifier is separate from the existing desktop SolidCraft application.

## 2. Application architecture

```text
SwiftUI application
  └─ CADModel + WKWebView
       ├─ LocalWebServer → bundled Web/index.html, JavaScript and WASM
       ├─ input.js → translates touch/Pencil gestures into eframe input
       ├─ bridge.js → Files picker, downloads, diagnostics and layout bridge
       └─ Rust web application
            ├─ egui CAD interface and Apple touch layout
            ├─ existing command/engine/modeling pipeline
            ├─ browser-owned saved-design storage
            └─ DXF/SVG and existing CAD import/export services
```

We retained the working Rust modeling engine and command architecture. The Apple shell supplies platform services and translates input; it does not duplicate the CAD kernel in Swift.

## 3. SwiftUI and WebKit shell

`SolidcraftForIPad/SolidcraftForIPadApp.swift` defines the application, the SwiftUI CAD view, the `UIViewRepresentable` wrapper, and `CADModel`.

The model creates and retains one WKWebView with a persistent WebKit data store. JavaScript injection happens at document start, before the web app initializes. The shell installs the `solidcraft` script-message handler and loads the two bundled bridge scripts.

The shell also:

- Enables WebKit inspection for development.
- Disables WebView scrolling and bounce so the CAD canvas receives gestures.
- Restricts page navigation to the configured loopback host and port.
- Reports startup, navigation, JavaScript, and WebKit process errors.
- Writes runtime diagnostics into the app's Documents folder.
- Exposes runtime and layout information through the WebView accessibility value for device tests.
- Presents native Files pickers and receives exported file bytes.

The original native top bar was removed, including its title, instructions, status, share, and help buttons. This leaves the SolidCraft interface as the main view. Exported files remain accessible through Files, and diagnostics remain available locally.

## 4. Local asset server and offline startup

`SolidcraftForIPad/LocalWebServer.swift` uses Apple's Network framework to serve the bundled web assets from `127.0.0.1:18784`.

The launch URL is:

```text
http://127.0.0.1:18784/index.html?sample&webgl&apple
```

The query enables the sample, WebGL rendering, and the Apple touch layout. The fixed origin gives browser storage a consistent identity between launches.

The server handles GET requests, returns MIME types including `application/wasm`, and closes each connection after responding. Request headers are bounded at 16 KB. File paths are normalized and checked against the bundled root before reading. Responses use `Cache-Control: no-cache`.

The Info.plist permits local networking and web-content loads needed by this HTTP startup path. These are prototype settings that should be reviewed during production hardening.

## 5. Building and bundling the Rust web engine

The prototype initially used the upstream published web build. That original asset set is preserved in `prebuilt-web-backup/`. We then moved to a locally compiled web target so the Apple-specific behavior could be changed in Rust.

The source was imported at upstream commit `2bb24d1` and merged with upstream release v0.24.0 (`5965439`); the exact base revision and bundled file hashes are captured in `SolidcraftForIPad/Web/asset-manifest.json`. The manifest marks the build as modified source. A base commit alone does not reproduce the local modifications: retain the working source changes as well.

`scripts/build-local-web.sh`:

1. Locates the sibling upstream source tree.
2. Creates a temporary Apple HTML entry point.
3. Disables wasm-opt for prototype iteration while retaining a Rust release build.
4. Builds with Trunk, four Cargo build jobs, release LTO disabled, relative public URLs, and no SRI.
5. Runs the asset-bundling script and removes the temporary entry point.

`scripts/bundle-local-web.py` replaces the app's bundled `Web/` output with `upstream-solidcraft/dist/web`, copies licenses/notices/attribution, and writes SHA-256 hashes for the bundled files. It preserves an initial backup if one does not already exist. This changes bundled engine resources, not the iPad's saved documents.

Upstream licenses, notices, third-party license inventory, and asset attribution are shipped with the web resources. The app icon comes from the upstream asset set.

## 6. Touch and Apple Pencil input

The initial adapter was corrected to send the event types eframe actually consumes: `pointerdown`, `pointerup`, and `mousemove`. Sending only mouse-down/mouse-up events did not drive the renderer correctly.

We removed the manual navigation-mode switch and implemented automatic routing in `input.js`.

| Contact/action | Behavior |
| --- | --- |
| One finger dragged in model space | Orbit |
| Two fingers moved together | Pan using their centroid |
| Two-finger distance change | Pinch zoom |
| Finger tap or drag on CAD controls | Primary UI input |
| Finger double-tap on a model face | Align view normal to the face |
| Pencil tap on model geometry | Add an item, or deselect it if already selected; retain other picks |
| Pencil tap in empty model space | Clear the selection |
| Pencil press and drag with a sketch drawing tool | Preview while held; lift to place the next point |
| Pencil hover over buttons | Show the same name tooltip used by the mouse |
| Finger/palm during a Pencil stroke | Suppressed |
| Lift one finger after a two-finger gesture | Navigation pauses until all fingers lift |
| Mouse on iPad or Mac Catalyst | Click replaces selection; Shift-click adds or removes items |

Trusted original touch events are stopped before egui can interpret fingers as model selection. Translated events are allowed through. Mouse model selection is now enabled on iPad as well as Mac Catalyst. The host identifies accepted Pencil and mouse presses separately through `touch_set_pencil`; synthetic mouse events do not erase the original source. Ignored palm contacts cannot switch an active Pencil stroke to mouse selection. In this handoff, **click** means mouse input and **tap** means Pencil input unless a finger gesture is explicitly named.

Gesture stability changes prevent camera nudges when fingers land:

- One-finger orbit waits for 100 ms and at least 6 points of movement.
- A second contact before orbit commitment starts pan instead.
- Cursor/centroid relocation is processed in a released-button frame before navigation starts.
- Cancellation and focus loss release input state.
- Pencil contact clears conflicting finger navigation and UI-contact state.

Pencil hover labels are implemented and the user confirmed they work on the physical iPad after the hover-routing correction. Pressure-sensitive modeling is not implemented. Physical precision and the latest drawing/selection behavior remain separate hands-on validation items.

## 7. Routing fingers to the correct interface region

We added `crates/ui-egui/src/apple_touch.rs` so JavaScript can ask the actual Rust interface whether a point belongs to model space.

The classification uses the current viewport, navigation controls, orientation widget, browser divider, and egui floating/menu layers. It accounts for egui's zoom factor rather than assuming fixed screen coordinates. This lets fingers use menus, dialogs, toolbar buttons, and browser dividers without accidentally orbiting the model.

The web target exposes:

- `touch_is_model(x, y)` for hit classification.
- `touch_layout()` for current layout and diagnostic state.
- `touch_project(x, y, z)` for projecting model coordinates, including test probes.
- `touch_align_face(x, y)` for queued view alignment.

Host-triggered alignment explicitly requests a repaint so it also works when the web painter is idle.

## 8. Toolbar, browser, and view-control changes

The lower floating navigation bar was removed for the Apple layout. Gesture navigation replaces its orbit, spin, pan, and zoom buttons.

The current upper-right control row contains Fit, Look At, Home, projection, visual style, and settings. Home was initially removed and subsequently restored. Controls use 40-point hit rectangles in a 44-point grid. The orientation widget sits beneath the row.

The browser's right divider is finger-resizable, with a larger side grab radius. The folder icon toggle, with its B overlay removed, collapses the browser to a 52-point rail and restores the retained width when reopened. Width and collapsed state are saved in preferences; restored widths are constrained to 160–560 points.

Double-tap face alignment preserves selection. Planar faces use their exact normal; curved faces use the nearest rendered triangle normal at the hit. The view switches to orthographic and looks normal to that face. A curved-face alignment can therefore produce a different orientation from an overhead planar-face view.

Insert DXF and Insert SVG were also added to the SOLID INSERT panel using the existing shared command list.

## 9. Native Files integration

Apple Open and Insert now present `UIDocumentPickerViewController` instead of depending exclusively on the browser's origin-private file panel.

The JavaScript bridge supplies `window.showOpenFilePicker()` and returns a structural file handle with a name and asynchronous `getFile()` function. Swift reads the selected copy with security-scoped access and passes the bytes back to JavaScript as base64. JavaScript constructs a real `File` for the existing importer.

The picker:

- Uses Files and available iCloud/document providers.
- Selects one file at a time.
- Reads a copy rather than overwriting the source.
- Enforces a 128 MB import limit.
- Treats cancellation as cancellation rather than an import error.
- Uses the app Documents directory as the initial location only under the debug UI-test launch flag; normal use retains provider browsing behavior.

We fixed a “not a file” error caused by testing native handles with `instanceof FileSystemFileHandle`. The native bridge returns structural JavaScript objects, so the importer now validates the name and `getFile()` capability, awaits a real File, checks the size, and then reads bytes.

A separate `pick_insert` host service preserves Insert intent. Desktop services retain their fallback behavior.

DXF and SVG selections run the existing sketch import commands. With an active sketch they use the existing sketch-import path; otherwise an XY sketch can be created. The web app fits the view after successful import and surfaces returned warnings. Other supported formats continue through the existing Open/Insert paths.

DWG remains unsupported. The app displays guidance to export ASCII DXF before insertion.

## 10. Saved designs and native exports

Saved designs use the web application's browser storage. The stable loopback origin and persistent WKWebView data store support reopening saved work. The bridge runs a small storage write/read/remove probe during startup.

The export bridge intercepts Blob-backed download anchors, reads their bytes, and sends them to Swift. Swift sanitizes the output name and writes to:

```text
Documents/Exports/<unique-id>/<filename>
```

Each export gets a unique folder to avoid collisions. The JavaScript export limit is 128 MB, and Swift also bounds encoded messages before decoding. Files sharing and document-opening support are enabled in Info.plist. Users can retrieve exports from Files under On My iPad → Solidcraft for iPad.

The real engine workflow was verified by changing sample thickness from 8 to 16 mm, saving, reopening, and exporting JSON. The returned 9,341-byte JSON was parsed and contained `16 mm` exactly. This proves that recorded engine workflow; it does not establish every export format or every on-screen editing dialog.

The app currently starts with the sample on cold launch. Browser storage retains saved designs, and upstream periodic recovery has a 30-second interval. Force-termination recovery and iCloud synchronization are not established by this implementation.

## 11. Large DXF support

A supplied large drawing exceeded the existing 20,000-point single-sketch limit. We changed the DXF importer to preserve supported geometry in multiple editable sketches rather than forcing it into one oversized sketch.

The implementation:

- Estimates point requirements before insertion.
- Keeps small DXF imports on their original single-sketch path.
- Groups large DXF geometry by layer.
- Splits oversized layers into parts with an 8,000-point budget including the sketch origin.
- Allows up to 256 sketch parts and reports an error beyond that limit.
- Preserves units, coordinates, and the active sketch plane.
- Leaves imported parts visible and restores the prior active sketch.
- Keeps the overall import as one undo step.
- Propagates curve insertion errors instead of silently dropping failed curves.
- Reports part counts and unsupported entity types.
- Retains legacy POLYLINE vertices in their parent's layer and block geometry under the top-level insertion layer.

Profiles crossing part boundaries need joining before extrusion. Unsupported entities are not converted into supported geometry. HATCH remains unsupported.

The recorded physical-device import produced 38,994 curves in 23 sketch parts, with 44,037 stored points. Settings remained responsive after import. The source drawing was preserved; testing used a copy. Source drawing contents are intentionally omitted from this handoff.

## 12. Full-screen layout and system-bar correction

The CAD canvas already fills its web page using 100% width and height. The native SwiftUI wrapper originally respected all safe-area edges, limiting the area available to that page.

We first added an expanding frame and ignored all container safe-area edges. We also disabled automatic WebView scroll-content inset adjustment. The signed update built, installed, and launched successfully, but the user's screenshot showed the iPad system menu bar covering the upper CAD interface.

We corrected the wrapper to respect the top safe area while extending to the horizontal and bottom edges:

```swift
CADWebView(webView: model.webView)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .ignoresSafeArea(.container, edges: [.horizontal, .bottom])
```

The WebView configuration also contains:

```swift
webView.scrollView.contentInsetAdjustmentBehavior = .never
```

This gives SwiftUI responsibility for the top system inset without adding a second automatic WebView inset. The final update built, installed, and launched on the iPad. The user subsequently reported “great.” No new automated screenshot assertion was added for this final safe-area change.

## 13. Xcode project configuration

The app uses automatic development signing, a generated launch screen, Swift 5 configuration, version `0.1.0`, and iPad-only targeting. Portrait, upside-down portrait, and both landscape orientations are declared. Mac Catalyst is enabled for the shared shell.

`scripts/create-project.rb` contains the initial project-generation setup. The checked-in Xcode project is the current build input; do not assume regenerating from the initial script preserves every later project adjustment.

The UI-test target depends on the application and uses the same configured development-signing team. The app includes Web resources as a folder resource and bundles `input.js`, `bridge.js`, and its asset catalog.

## 14. Source and file inventory

Paths in this section are relative to the workspace root.

| File or directory | Purpose/change |
| --- | --- |
| `ApplePrototype/SolidcraftForIPad/SolidcraftForIPadApp.swift` | SwiftUI shell, WebView, native picker/export, diagnostics, final safe-area behavior |
| `ApplePrototype/SolidcraftForIPad/LocalWebServer.swift` | Loopback-only bundled asset service |
| `ApplePrototype/SolidcraftForIPad/input.js` | Finger/Pencil routing and stable navigation gestures |
| `ApplePrototype/SolidcraftForIPad/bridge.js` | Native Files shim, Blob exports, runtime/layout bridge, workflow harness |
| `ApplePrototype/SolidcraftForIPad/Info.plist` | Files sharing, document access, orientations, web/local networking settings |
| `ApplePrototype/SolidcraftForIPad/Web/` | Locally compiled web engine and licensed assets |
| `ApplePrototype/prebuilt-web-backup/` | Original downloaded web asset backup |
| `ApplePrototype/scripts/build-local-web.sh` | Trunk release build for the Apple bundle |
| `ApplePrototype/scripts/bundle-local-web.py` | Resource replacement, license copying, asset hashes |
| `ApplePrototype/scripts/build-and-install.sh` | Signed device build and install (does not launch) |
| `ApplePrototype/scripts/create-project.rb` | Initial Xcode project generator |
| `ApplePrototype/scripts/input-tests.cjs` | JavaScript input regression coverage |
| `ApplePrototype/SolidcraftForIPadUITests/RuntimeTests.swift` | Real-device runtime, UI, workflow, and import checks |
| `ApplePrototype/verification/RESULTS.md` | Historical verification results and limits |
| `upstream-solidcraft/apps/solidcraft-web/src/files.rs` | Native structural-handle validation and byte import |
| `upstream-solidcraft/apps/solidcraft-web/src/main.rs` | Apple mode, WASM bridge exports, native Open/Insert routing, import warnings |
| `upstream-solidcraft/apps/solidcraft/src/main.rs` | Desktop compatibility for the added Insert service |
| `upstream-solidcraft/crates/ui-egui/src/apple_touch.rs` | Hit regions, layout diagnostics, projection, face alignment, tests |
| `upstream-solidcraft/crates/ui-egui/src/lib.rs` | Apple UI state, browser preferences, Insert service, resize tolerance |
| `upstream-solidcraft/crates/ui-egui/src/browser.rs` | Resizable/collapsible browser |
| `upstream-solidcraft/crates/ui-egui/src/viewport.rs` | Apple view controls and viewport behavior |
| `upstream-solidcraft/crates/ui-egui/src/viewport_layout.rs` | Apple control layout |
| `upstream-solidcraft/crates/ui-egui/src/toolbar.rs` | Toolbar integration |
| `upstream-solidcraft/crates/ui-egui/src/sketch_tools.rs` | SOLID INSERT DXF/SVG entries |
| `upstream-solidcraft/crates/engine/src/cmd/sketch_import.rs` | Large layered DXF import, error propagation, atomic undo tests |
| `upstream-solidcraft/crates/io/src/dxf.rs` | Layer-aware DXF extraction and unsupported-entity reporting |
| `upstream-solidcraft/crates/io/src/lib.rs` | Exposes the new DXF helpers |

The current upstream checkout contains local tracked modifications and the new untracked `apple_touch.rs` file. The ApplePrototype folder is not its own Git repository. The records do not claim an upstream commit, push, or PR for these changes.

## 15. Rebuild and install

From `ApplePrototype/`, rebuild web assets only when Rust/web source changes:

```bash
bash scripts/build-local-web.sh
```

List connected devices and use the intended iPad identifier:

```bash
xcrun devicectl list devices
bash scripts/build-and-install.sh <IPAD_UDID>
```

The installation script runs:

```bash
xcodebuild -project SolidcraftForIPad.xcodeproj -scheme SolidcraftForIPad \
  -configuration Debug -destination "id=<IPAD_UDID>" \
  -derivedDataPath build -allowProvisioningUpdates build

xcrun devicectl device install app --device <IPAD_UDID> \
  build/Build/Products/Debug-iphoneos/SolidcraftForIPad.app

xcrun devicectl device process launch --device <IPAD_UDID> \
  com.cwallyman.SolidcraftForIPad
```

For a Swift-only shell change, the existing bundled web output can be reused. Development signing and an available paired iPad are required. Avoid uninstalling the app when updating because saved work belongs to its existing container.

Optional Mac Catalyst compilation check:

```bash
xcodebuild -project SolidcraftForIPad.xcodeproj -scheme SolidcraftForIPad \
  -destination 'platform=macOS,variant=Mac Catalyst' \
  CODE_SIGNING_ALLOWED=NO REGISTER_APP_WITH_LAUNCH_SERVICES=NO build
```

Disabling Launch Services registration avoided a recorded Mac build stall. The build also warns that `DERIVE_MACCATALYST_PRODUCT_BUNDLE_IDENTIFIER` is unsupported; this configuration cleanup remains outstanding. Compilation does not prove Mac runtime behavior.

## 16. Validation performed

These results span several revisions. A pass at an earlier revision should not be interpreted as a complete latest-revision suite pass.

| Area | Recorded evidence |
| --- | --- |
| Initial bundled runtime and model workflow | `ModelBridgeTests.xcresult`: 2 tests, 0 failures; sample render and thickness/save/reopen/export screenshots inspected |
| Touch navigation and Pencil policy | Input regression suite passed at successive stages; `PencilControlsTests.xcresult`: 2 physical tests passed |
| Touch interface/browser/view controls | 12 JavaScript tests and 4 Rust Apple-touch tests passed; `TouchInterfaceVerifiedTests.xcresult`: 3 physical tests passed |
| Gesture landing stability | `StableGestureTests.xcresult` passed; `ContactOrientationTests.xcresult` confirmed identical orientation-widget pixels on first contact |
| Native top-bar removal | `NoBarTests.xcresult`: runtime/navigation and workflow tests passed; screenshot inspected |
| Restored Home/native Files | Focused physical test passed in `NativeFilesFocusedTests.xcresult`; native picker screenshot observed |
| DXF picker end-to-end | `DXFPickerEndToEndTests.xcresult` passed using a four-line rectangle; imported editable sketch screenshot inspected |
| Large DXF | 2 engine and 3 IO tests passed; `LargeDXFDeviceTests.xcresult` passed; physical import screenshot inspected |
| Latest screen-layout changes | Mac Catalyst build passed for the initial expansion; signed iPad builds, installation, and launch passed for both expansion and top-inset correction |

A debug-only workflow launch drives real eframe pointer, clipboard, and keyboard events to execute commands. This was used because XCTest `typeText` initially did not enter palette commands. It exercises the real engine path but is not proof that every manual soft-keyboard editing interaction is usable.

Device automation initially had timeouts enabling automation mode. Direct installation and serialized UI tests (`-parallel-testing-enabled NO`) worked. A test assumption was adjusted for the observed 1.35 UI scale.

One later broad touch-interface retry failed its overhead-face pitch assertion after a curved-face-like alignment, although its picker/Home steps succeeded. XCTest's attempted two-finger-tap API also failed canvas hit testing before delivering that gesture. Staggered finger landing has JavaScript regression coverage; that failed API attempt is not device proof of the exact gesture.

Repository format, asset attribution, license inventory, and layer checks passed in the recorded work. Full upstream CI did not pass: existing strict lint failures were reported in `crates/sketch/src/solver.rs`, `crates/ui-egui/src/canvas.rs`, and `crates/ui-egui/src/issue_report.rs`. No complete repository CI pass is claimed.

## 17. Remaining limitations and follow-up work

- This is a locally signed prototype, not an archived/uploaded App Store release.
- The interface remains derived from the desktop layout.
- Pencil hover labels have user confirmation on the physical iPad. Pressure-sensitive modeling is not implemented; physical precision and the latest drawing/selection changes have not received explicit hands-on confirmation.
- Manual soft-keyboard input across all editing dialogs remains incompletely verified.
- STEP/STL export behavior is not established by the recorded JSON workflow.
- DWG and DXF HATCH remain unsupported.
- Large DXF sketch parts may require joining before profile extrusion.
- Saved-design recovery after force termination and iCloud synchronization remain unverified.
- Mac Catalyst runtime behavior is not established by compilation alone.
- The final safe-area correction has successful device build/install/launch and positive user feedback, but no new automated screenshot assertion.
- The local source modifications must be preserved and versioned to make the customized build reproducible.
- Existing upstream CI lint failures and the Catalyst build-setting warning remain to be resolved before claiming a clean production build pipeline.

## 18. Practical usage check

After launching the installed update:

1. Confirm the top toolbar sits below the system menu/status area.
2. Orbit with one finger, pan with two fingers, and pinch to zoom.
3. Tap toolbar and settings controls with a finger.
4. Resize the browser divider; collapse/reopen with the folder button.
5. Double-tap a planar face with a finger to align the view.
6. Use Pencil taps to select multiple items, tap a selected item to remove it, and tap empty space to clear. Choose a sketch tool, press and drag with Pencil, then lift to place the next point. Hover over a button to show its name. Use mouse clicks for replacement selection and Shift-click for multiple selection.
7. Use File → Open or SOLID → INSERT → Insert DXF/SVG to select a file through Files.
8. Save important designs and retrieve native exports through Files.

For underlying historical evidence and revision-specific caveats, retain `verification/RESULTS.md`, the `.xcresult` bundles, and the local verification screenshots alongside this report.

## 19. Faceted mesh conversion update

Convert Mesh now offers two methods. Prismatic retains the existing constant-section analytic reconstruction. Faceted repairs duplicate triangles, separates touching closed shells, normalizes shell directions, and rebuilds each accepted shell as its own CAD body. Adjacent coplanar triangles are merged; curved mesh regions retain planar facets. One timeline conversion feature owns all resulting bodies, and undo restores the original mesh.

The new method handles up to 25,000 input triangles and 256 shells. It refuses open/non-manifold shells and potential nested inward cavity shells. It does not patch holes, fuse overlapping parts, or recreate the original modeling history. The source STL is not changed.

The supplied locomotive STL converted into 38 CAD bodies with 436 faces. Saved-design reopening preserved that result. STEP export produced 38 `MANIFOLD_SOLID_BREP` entries and no `BREP_WITH_VOIDS`; reimport returned 38 valid bodies without warnings. Five engine conversion tests passed, including that actual-file round trip. The sum of outward-shell volumes was approximately 216.368055 mm³; overlapping parts are not fused, so this is not a union-volume claim.

The method choice is stored with the feature. Older saved conversion features default to Prismatic. The dialog includes a Method selector and explains that Faceted parts remain separate. Detailed implementation and revision-specific verification are in `verification/mesh-conversion/IMPLEMENTATION.md` and `verification/RESULTS.md`.


## 20. Pencil, mouse, toolbar and browser update — October 14, 2026

This section records all changes from the October 14 interaction session. The final installed state supersedes earlier descriptions of Pencil-only model selection, non-toggling Pencil picks, and the folder B overlay.

### Final input conventions and selection

The user's terminology is explicit: **click = mouse; tap = Pencil**. Finger navigation and finger operation of interface controls remain available and are identified explicitly in these notes.

Pencil taps add sketch lines, edges and faces to the current selection without a keyboard modifier. Tapping an already selected item deselects that item while retaining other picks; tapping it once more selects it again. Tapping empty model space clears the entire selection. Grouped kernel edge/face pieces use the existing grouped-selection logic so one visible item toggles consistently. Pencil selection bypasses repeated-mouse-click candidate cycling and automatic double-click chain selection. Pencil double taps on faces do not invoke the finger face-alignment behavior. Existing dimension editing remains available.

Mouse clicks replace the selection. Shift-click adds or removes an item using the existing mouse modifier behavior. The former iPad mouse-selection gate was removed. Pencil identity is passed separately through the WASM bridge rather than emulating a held Shift key, preserving drawing and navigation behavior. Accepted mouse/finger input resets the source appropriately; ignored palm input during Pencil contact does not reset it.

Implementation: `SolidcraftForIPad/input.js`, `apps/solidcraft-web/src/main.rs` (`touch_set_pencil`), `crates/ui-egui/src/apple_touch.rs`, and `crates/ui-egui/src/viewport.rs` (`select_input`). Rust paths are relative to the sibling `upstream-solidcraft` checkout.

### Pencil hover button names

The Swift shell installs a Pencil-only `UIHoverGestureRecognizer` and forwards its coordinates to `window.solidcraftPencilHover`. The recognizer allows simultaneous recognition with WebKit recognizers. UIKit and WebKit pen-hover paths share one cursor handler. A five-point movement threshold prevents hand tremor from continually restarting egui's stationary-tooltip delay. Compatibility mouse movement/leave events are suppressed while Pencil hover owns the cursor; exit clears hover. Active drawing, navigation and interface contacts take precedence over hover.

The first installed attempt did not work according to the user. After the routing, gesture-recognition and cursor-stability fixes, the user explicitly reported **“works now.”** This confirms physical Pencil hover button labels for that revision; it does not establish pressure-sensitive tools or every subsequent interaction change.

A first hover event reaching the input bridge writes `Documents/pencil-hover-check.txt`. This diagnostic is event-delivery evidence, not proof that a particular tooltip rendered. Implementation: `SolidcraftForIPadApp.swift`, `input.js`, and `scripts/input-tests.cjs`.

### Toolbar dropdown toggling

Create, Modify and the other toolbar panel triggers now close their menu on a second click or tap. Both full panel labels and collapsed panel triggers share `toggle_panel_menu`; selecting another panel replaces the open menu. Existing command selection, outside dismissal and Escape behavior remain. Implementation and regression tests are in `crates/ui-egui/src/toolbar.rs`.

### Sketch press-and-drag drawing

With a sketch drawing tool selected, press with Pencil at the start, drag to preview, then lift to place the next point. The implementation also accepts the equivalent mouse drag. For a fresh shape, the press origin supplies the first point. For an in-progress shape or line chain, dragging places the next point without duplicating the existing start. Separate point-by-point taps/clicks remain supported.

Lines, two-point and center rectangles, two-point and center circles, and polygons can complete on lift. Arcs, three-point rectangles, ellipses, slots and other multi-point tools retain the remaining point stages. Extended point-based tools share the gesture path; splines still use their existing point/finish workflow, not freehand curve fitting. Point placement, snapping, constraints and command execution remain in the existing drawing pipeline. Navigation gestures and non-drawing selection tools do not enter this drawing path.

Implementation: `crates/ui-egui/src/viewport.rs` (`sketch_draw_drag`), `crates/ui-egui/src/sketch_tools.rs` (`supports_draw_drag`), and `crates/ui-egui/src/sketch_neutral_click_tests.rs`.

### Browser folder icon

Removed only the painted B text overlay from the browser collapse/reopen folder icon in `crates/ui-egui/src/browser.rs`. The folder drawing, tooltip, hit region and collapse/reopen behavior remain.

### Verification and delivery

- Dropdown regressions: two tests passed, covering panel toggle/switch state and rendered Create/Modify trigger interactions.
- Hover correction: 14 JavaScript input tests passed at that revision. The user confirmed hover works on the physical iPad.
- Sketch regressions: three tests passed, covering drag completion and one-step undo across six drawing commands, remaining-point completion for arcs/three-point rectangles/ellipses, and repeated-first-point neutrality. These three tests passed again after the selection-source changes.
- Final selection regressions: two tests passed, covering Pencil add/remove/reselect/empty-space clear, independent edge/face toggling, ordinary mouse replacement, and Shift-click accumulation.
- Input routing: 16 JavaScript tests passed after source separation and the palm-input guard.
- Updated WASM release builds and signed iPad builds succeeded. Each change was bundled, installed and launched on the connected iPad Air 11-inch (M3); the final installed revision includes the folder-icon cleanup and all preceding changes.
- A physical UI startup smoke test did not initialize because device authentication was canceled. It is a failed automation attempt, not a passing device UI test. No new screenshot assertion or complete device suite pass is claimed for this session.
- The user explicitly confirmed hover only. Latest sketch drawing, selection toggling, dropdown behavior and the icon cleanup have automated/source/build evidence and successful installation, but no explicit final hands-on confirmation in this session.
- Full upstream CI was not rerun; earlier documented baseline lint limitations remain. No commit, push, PR, upstream merge, App Store archive or upload was performed in this session.

See `verification/RESULTS.md` and `verification/2026-10-14-interactions/` for retained test/build evidence. Temporary build logs were used during iteration; selected logs and delivery summaries have been copied into that verification folder for this handoff.


## 21. GitHub checkpoint — October 14, 2026

The initial GitHub checkpoint was created as the private repository `cwallyman/ipad-solidcraft`. That standalone repository subsequently became public and is now retained as an archived backup; the maintained home is [cwallyman/solidcraft, branch ipad-solidcraft](https://github.com/cwallyman/solidcraft/tree/ipad-solidcraft). The enclosing checkout contains both `ApplePrototype/` and `upstream-solidcraft/`; the Rust source is a Git subtree based on `2bb24d1`, with the original project's history retained. This replaces the former nested Git checkout so one commit can capture the matching Swift shell, bundled WASM, Rust adaptations and documentation. Original nested checkout metadata is backed up locally under the enclosing `.git/` directory and is not published.

The root README documents the project, build/install workflow, licensing and future `git subtree pull --prefix=upstream-solidcraft upstream main` updates. For the initial checkpoint, `origin` identified the standalone repository. Current configuration uses `origin` for `cwallyman/solidcraft`, `upstream` for the original developer's repository, and `archive` for the standalone backup. The original developer's newer commits were checked but have not been merged into this checkpoint. Build output, device `.xcresult` bundles, local verification media/logs, imported drawings, the initial web-bundle backup and signing credentials are excluded. The current bundled web engine is included for the installed prototype. Historical verification Markdown remains included, even when its referenced local artifacts are excluded.

Before this checkpoint, `cargo fmt --all` normalized Rust formatting. `cargo xtask ci` then passed formatting, asset attribution, license inventory and dependency layers; it stopped on existing `clippy::nonminimal_bool` failures in `crates/sketch/src/solver.rs` and `crates/kernel/src/coplanar.rs`. A full CI pass is not claimed. Earlier targeted tests and the successful build/install results are recorded in section 20; formatting does not change their behavior, but no new physical-device validation was performed for the GitHub checkpoint itself. No App Store release is part of this operation.


## 22. Current iPad screenshot gallery — October 14, 2026

At the user's request, twelve reviewed screenshots are retained in `docs/screenshots/ipad/2026-10-14/` at the repository root, with a captioned gallery at `docs/screenshots/ipad/README.md` and a method/selection-count manifest alongside the PNGs. The root README links the gallery and embeds the workspace screenshot. These images show the real physical iPad Air 11-inch (M3) app using its bundled engine, the public Sample Plate and a small demonstration sketch; no user drawing files were published. This explicitly requested gallery is versioned on GitHub; bulk device bundles, private designs, diagnostic logs and unsuccessful capture attempts remain excluded.

Images cover the workspace and plain folder icon, Fit hover label, Create dropdown open/closed, multiple selected faces and one deselected face, rectangle drag preview and completion, browser collapse and resize, view settings and face-normal alignment. The hover/dropdown/selection/sketch states were staged with injected UI input through a temporary DEBUG fixture using the existing workflow-test callback. Native screenshots captured the actual device renderer; these are not independent physical-Pencil hardware tests. The fixture was removed from the source before publication. The gallery describes each capture method and records selection counts of two, then one for the face example.

The physical-device focused test `RuntimeTests/testFingerInterfaceBrowserResizeCollapseAndFaceAlignment` passed with zero failures in approximately 18 seconds and supplied the browser/view screenshots. This supersedes the earlier lack of a passing current browser/view UI test, but does not turn the earlier authentication-canceled attempt into a pass or establish a full device suite/CI pass. An initial staging attempt left the line tool active during attempted sketch-selection captures; those images were rejected during review. The iPad locked before additional selection retakes could launch. The gallery includes the valid face-selection sequence instead and does not present failed attempts as selection evidence.

The original production source was restored, and a normal signed build is used after the capture session. No application feature changes are included in this screenshot/documentation update. The current physical Pencil hover confirmation remains the user's earlier hands-on report. See the gallery for full-resolution PNGs and individual captions.


## 23. Consolidated repository home — October 14, 2026

The maintained home is [cwallyman/solidcraft, branch ipad-solidcraft](https://github.com/cwallyman/solidcraft/tree/ipad-solidcraft), verified by GitHub as a fork of `bherbruck/solidcraft`. The complete iPad source, bundled engine, documentation and screenshot gallery are on that branch. The fork's default branch is `ipad-solidcraft`; its existing `main` branch is preserved.

The local branch is named `ipad-solidcraft` and tracks `origin/ipad-solidcraft`. `origin` points to the fork, `upstream` points to the original developer, and the read-only `archive` remote points to `cwallyman/ipad-solidcraft`. The duplicate standalone repository is archived rather than deleted, retaining its original commits and links as a backup. Future changes are pushed to the fork. The root README contains branch-specific clone and subtree-update commands.

This is repository/documentation maintenance. No app source, bundled assets, user designs or original-developer branches were changed. The screenshot gallery remains in `docs/screenshots/ipad/` on the maintained branch.

## 24. Upstream v0.24.0 sync, iPad fixes and rename — October 2026

The subtree was merged with upstream release v0.24.0 (`5965439`; the project now lives at `tangency-labs/solidcraft`). Upstream redesigned the shell as a full-bleed viewport with floating cards, async file pickers and a separate navigation module; the iPad changes were re-applied on top. Fixes after the merge:

- `LocalWebServer.swift` serves CSS, SVG, PNG and manifest files with their real MIME types; WebKit ignored the stylesheet when it was `text/plain`, which left a black screen.
- Finger taps on the floating ribbon and History cards are routed to the interface (`shell::covered_rects` in `apple_touch.rs`).
- Faceted mesh conversion accepts up to 25,000 triangles.
- An **iPad Touch** navigation preset is the default on iPad. Finger gestures (one-finger orbit, two-finger pan, pinch zoom, two-finger twist roll) work whatever preset is chosen; a connected mouse follows the chosen preset.
- A tap on the model closes the File menu and command search.
- Swipes on the interface scroll with momentum; holding 200 ms before moving drags instead.
- `build-and-install.sh` installs only; it no longer launches the app.

Release scripts: `build-release-web.sh` bundles the size-optimized engine (full LTO, `wasm-opt -Oz`, no debug info) and `archive-release.sh` makes a Release archive for the Xcode Organizer.

The app was renamed **Solidcraft for iPad**: Xcode project, target, scheme and source folder `SolidcraftForIPad`, bundle identifier `com.cwallyman.SolidcraftForIPad`. The earlier `com.cwallyman.SolidCraftTest` build is a different app on the device. Records in `verification/` keep the old names.

The planned move to a native Rust/Metal app is in [NATIVE_IOS_PLAN.md](NATIVE_IOS_PLAN.md).

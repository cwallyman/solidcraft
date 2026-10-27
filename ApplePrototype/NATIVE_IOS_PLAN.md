# Native iOS plan

Goal: run SolidCraft on the iPad as native Rust that draws with Metal, with no WKWebView, wasm, local server or JS bridge. The engine and UI crates stay shared with upstream so `git subtree` merges keep working.

## Rules for every agent (Claude Code or Codex)

- Do **one phase per session**, in a fresh session. Starting prompt: `Read ApplePrototype/NATIVE_IOS_PLAN.md and do Phase N.`
- Read only the files the phase lists, plus whatever a compile error points at. No broad exploration.
- Checks you may run:
  - `cargo check`
  - the build and install scripts the phase names
- Never launch the app, and never use the simulator.
- Never run tests or benchmarks. The user tests on the iPad.
- All work happens on branch `native-ios-solidcraft`, never on `ipad-solidcraft`.
  - Phase 0 creates it from `ipad-solidcraft` (`git switch -c native-ios-solidcraft`).
  - Every later phase first runs `git switch native-ios-solidcraft` and stops if there are uncommitted changes from someone else.
  - Commit once at the end of the phase. Don't push or merge unless the user asks.
- Keep the WKWebView app working until Phase 8.
- Upstream-friendly edits:
  - New code goes in `upstream-solidcraft/apps/solidcraft-ios/` or `ApplePrototype/`.
  - A change inside a shared crate must be small and gated with `#[cfg(target_os = "ios")]`.
- When done:
  - Add at most 5 lines to the Progress log at the bottom: what changed, what to test, and any open issue.
  - Then stop. Don't start the next phase.
- When the user reports a bug: fix it in the same session if it's still open; otherwise start a new session with the bug text and the phase number.
- Only one agent works on the branch at a time.

## Architecture

- **Rust**: new crate `upstream-solidcraft/apps/solidcraft-ios`, a `staticlib` for `aarch64-apple-ios`.
  - It owns an `egui::Context`, `ui_egui::SolveApp`, and an `egui_wgpu::Renderer` on a wgpu Metal surface made from a `CAMetalLayer`.
  - It copies the per-frame logic from `apps/solidcraft-web/src/main.rs` (`logic()`, `apple_touch::update`, `apply_roll`). The JS and wasm parts are dropped.
  - It exposes a small C ABI, `sc_*`, declared in `solidcraft_ios.h`.
- **Swift**: a new Xcode target, `SolidcraftForIPadNative`, with bundle id `com.cwallyman.SolidcraftForIPadNative`, so it installs beside the current app.
  - A `UIViewController` hosts a `CAMetalLayer` view, driven by `CADisplayLink`.
  - It turns UIKit touches, Pencil, hover, keyboard and document pickers into `sc_*` calls.
- **Build**:
  - `ApplePrototype/scripts/build-ios-native.sh` runs `cargo build --release --target aarch64-apple-ios -p solidcraft-ios` and copies the `.a` file and the header into `ApplePrototype/Native/`.
  - `build-and-install-native.sh UDID` runs xcodebuild and then `devicectl install`. It must not launch the app.

Why not use eframe or winit directly: eframe has no supported iOS path. Pencil hover and pressure, the software keyboard and the Files picker need UIKit anyway. A thin Swift host with egui-wgpu is less code than fighting eframe.

## Phases

Each phase lists **who**, what to **read**, what to **do**, and what the **user tests**.

### Phase 0: make the shared crates compile for iOS
- **Who**: Codex (mechanical).
- **Read**:
  - `Cargo.toml` (workspace)
  - `crates/ui-egui/Cargo.toml`, `crates/gpu/Cargo.toml`, `crates/io/Cargo.toml`
- **Do**:
  1. Create the branch: `git switch ipad-solidcraft && git switch -c native-ios-solidcraft`.
  2. `rustup target add aarch64-apple-ios`.
  3. Run `cargo check --target aarch64-apple-ios -p solidcraft-ui-egui`. Use the crate's real package name.
  4. Fix each error with the smallest cfg gate. The likely trouble spots are `rfd`, desktop-only paths in `io` (`design_lock`, `vfs`), `space-mouse` and `threedx-client`.
  5. Gate desktop-only dependencies with `[target.'cfg(not(target_os = "ios"))'.dependencies]`.
- **User tests**: nothing. Done when `cargo check` is clean.

### Phase 1: first pixels
- **Who**: Claude Code.
- **Read**:
  - `apps/solidcraft-web/src/main.rs`: the `logic`/`ui` setup only
  - `crates/ui-egui/src/lib.rs`: the `SolveApp` constructor and its update entry point
  - `vendor/egui-wgpu` render example usage
- **Do**:
  1. Create the `solidcraft-ios` crate with these functions: `sc_create(layer, width_px, height_px, scale) -> *mut App`, `sc_resize`, `sc_frame`, `sc_destroy`.
  2. Load the sample document the way `?sample` does.
  3. Write the build script.
  4. Add the `SolidcraftForIPadNative` Xcode target: Swift host, a bridging header, and linking to the `.a` file plus the Metal and QuartzCore frameworks.
  5. Build and install.
- **User tests**: the app opens and shows the ribbon, browser and the sample model, correctly sized and sharp.

### Phase 2: fingers and Pencil
- **Who**: Claude Code.
- **Read**:
  - `ApplePrototype/SolidcraftForIPad/input.js`: the gesture rules to port
  - `crates/ui-egui/src/apple_touch.rs`
  - the Phase 1 Swift host
- **Do**:
  - Add an ABI that takes logical points: `sc_pointer(kind, phase, x, y, pressure, modifiers)`, `sc_scroll(dx, dy)`, `sc_zoom(factor, x, y)`, `sc_roll(radians)`, `sc_hover(x, y)`, `sc_finger_navigation(bool)`.
  - Port the `input.js` behavior in Swift, using the same constants:
    - One finger orbits; two fingers pan; pinch zooms; twist rolls.
    - The Pencil acts as the primary button.
    - Hover uses `UIHoverGestureRecognizer`.
    - A tap on the model closes overlays.
    - UI swipes scroll, with the `UI_HOLD_MS` drag and the glide.
    - Double-tap aligns to a face.
  - Feed real Pencil pressure and hover; don't fake them through the mouse.
- **User tests**: everything in the README's "Current iPad behavior" list, plus ribbon and History taps and swipe scrolling.

### Phase 3: keyboard and text
- **Who**: Codex.
- **Read**:
  - the Swift host
  - the `sc_*` ABI
  - egui's `Event::Text`, `Event::Key` and IME handling
- **Do**:
  - Show and hide the software keyboard from `ctx.wants_keyboard_input()`, using `UIKeyInput` on the Metal view.
  - Read the hardware keyboard through `pressesBegan`/`pressesEnded`, with modifiers.
  - Support copy and paste through `UIPasteboard` and egui's `PlatformOutput`.
- **User tests**: type dimensions and names, Cmd-Z/Cmd-C/Cmd-V, and Tab on a hardware keyboard.

### Phase 4: Files
- **Who**: Claude Code.
- **Read**:
  - `apps/solidcraft-web/src/files.rs`
  - the `ui-egui` `disk` module and its `FilePickerFuture`
  - `ApplePrototype/SolidcraftForIPad/bridge.js`: what the current app supports
- **Do**:
  - Write a native `disk` backend:
    - Rust asks Swift through a callback to show a `UIDocumentPickerViewController` (open or import, with the allowed types).
    - Swift reads the file with a security-scoped URL and passes the bytes and name back through `sc_file_opened`.
    - Save and export go through `sc_request_save`, which shows the document picker in export mode.
  - Remove the 128 MB cap.
- **User tests**:
  - Open and save a `.solidcraft` design.
  - Import STL, STEP and DXF, including the large DXF split.
  - Convert the AEM7 STL.
  - Export STL.

### Phase 5: persistence and lifecycle
- **Who**: Codex.
- **Read**:
  - the `ui-egui` prefs persistence
  - `crates/io/src/vfs.rs`
  - the Swift host
- **Do**:
  - Store prefs and autosave in Application Support.
  - Save on `sceneDidEnterBackground`.
  - Pause `CADisplayLink` in the background, and repaint only when egui asks.
  - Handle the safe area, Stage Manager and Split View resizing, and light and dark mode.
- **User tests**:
  - Prefs survive a relaunch.
  - Kill the app mid-edit, then recover.
  - Resize the window in Stage Manager.

### Phase 6: performance
- **Who**: Claude Code.
- **Read**: any `cfg(target_arch = "wasm32")` single-thread fallbacks in `kernel`, `render` and `render/src/job/cpu_budget.rs`.
- **Do**: let iOS take the native multithreaded paths (rayon and job threads), and cap the thread count at the number of performance cores.
- **User tests**: compare the AEM7 mesh conversion, large imports and the render against the web app, and watch for crashes or overheating.

### Phase 7: polish
- **Who**: Codex.
- **Do**:
  - App icon, launch screen and supported orientations.
  - The Pencil double-tap / squeeze action, if wanted.
  - Accessibility labels through egui AccessKit, if feasible; otherwise skip and note it.
- **User tests**: a quick pass over everything.

### Phase 8: switch over
- **Who**: Codex.
- **Do**:
  - Make `SolidcraftForIPadNative` the main target.
  - Delete `LocalWebServer.swift`, the WKWebView view, `input.js`, `bridge.js`, the `Web/` bundle and the web build scripts.
  - Point `archive-release.sh` at the native target. Release builds use `lto = true` and `codegen-units = 1`.
  - Update the README, keeping the license notices.
- **User tests**: a final full pass, then archive.

## Token-saving notes

- This file is the only shared context, so keep it short, and keep log entries short.
- Use a lower effort or cheaper model for the Codex-marked phases; they're mechanical.
- Paste the compiler error, not the whole log. Paste the bug symptom plus the steps, not long narratives.
- Don't re-explain the project in prompts; the phase points at the files.

## Progress log

<!-- Phase N (agent, date): change. Test: … Open: … -->

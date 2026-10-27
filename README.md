# ipad-solidcraft

The maintained iPad branch is [cwallyman/solidcraft → ipad-solidcraft](https://github.com/cwallyman/solidcraft/tree/ipad-solidcraft), a fork of the original project. The separate `cwallyman/ipad-solidcraft` repository is retained as an archived backup.

Clone this branch to get the iPad app:

```sh
git clone --branch ipad-solidcraft https://github.com/cwallyman/solidcraft.git
```

An installed iPad prototype of [SolidCraft](https://github.com/tangency-labs/solidcraft), using a SwiftUI/WKWebView shell and a bundled Rust/WebAssembly CAD engine. Modeling runs locally on the iPad.

[View the iPad screenshot gallery](docs/screenshots/ipad/README.md).

![SolidCraft running on iPad](docs/screenshots/ipad/2026-10-14/01-workspace.png)

## Project layout

- `ApplePrototype/`: Xcode app, Pencil/finger/mouse input bridge, native Files support, build scripts and implementation notes.
- `upstream-solidcraft/`: the original SolidCraft source and our iPad adaptations, retained as a Git subtree with upstream history.

Open `ApplePrototype/SolidCraftTest.xcodeproj` and select your own signing team to run on an iPad. A current bundled engine is included. Rebuild it after Rust changes:

```sh
bash ApplePrototype/scripts/build-local-web.sh
```

The script requires Rust with the `wasm32-unknown-unknown` target, Trunk, and the project's Rust dependencies. Xcode is required to build/install the Apple app. The command-line install helper accepts your connected iPad UDID:

```sh
bash ApplePrototype/scripts/build-and-install.sh YOUR_IPAD_UDID
```

### App Store build

`build-local-web.sh` skips wasm-opt and LTO so iteration stays fast. For an App Store submission, bundle the size-optimized engine (full LTO, `wasm-opt -Oz`, no debug info; much slower, and it uses its own `target/appstore` cache), then archive a Release build:

```sh
bash ApplePrototype/scripts/build-release-web.sh
bash ApplePrototype/scripts/archive-release.sh
```

The archive script refuses to run unless the App Store engine is bundled. It writes `ApplePrototype/build/SolidCraftTest.xcarchive`; open it in Xcode's Organizer to validate and upload. Bump `MARKETING_VERSION` / `CURRENT_PROJECT_VERSION` in Xcode before each upload, and run `build-local-web.sh` afterwards to return to the quick engine.

## Current iPad behavior

- One finger orbits; two fingers pan and pinch to zoom. Finger gestures also operate interface controls.
- Pencil hover shows button names. The user confirmed it works on the physical iPad.
- Select a sketch tool, press and drag with Pencil, then lift to place the next point. Multi-point shapes retain their remaining stages.
- Pencil taps add or remove individual selections; a tap in empty model space clears all selections. Mouse clicks replace selection; Shift-click adds/removes.
- Toolbar dropdowns close on a second tap/click. The browser folder toggle has no B overlay.
- Native Files handles supported imports; large DXFs can be split into editable layer parts. Faceted mesh conversion rebuilds accepted closed mesh shells as separate CAD bodies.

See [the full implementation handoff](ApplePrototype/IPAD_APP_IMPLEMENTATION.md), [app instructions](ApplePrototype/README.md), and [verification results](ApplePrototype/verification/RESULTS.md) for details and limitations. Build/install success does not establish physical validation of every interaction. This is a development prototype, not an App Store release.

## Updating from the original developer

The subtree was imported at upstream commit `2bb24d1` (v0.8.0) and has since been merged with upstream release v0.24.0 (`5965439`). The `upstream` remote points to `tangency-labs/solidcraft` (formerly `bherbruck/solidcraft`, which GitHub redirects); `origin` points to `cwallyman/solidcraft`. The maintained branch is `ipad-solidcraft` (also the fork's default branch). The optional `archive` remote identifies the archived standalone repository. Work on an update branch from a clean checkout:

```sh
git fetch origin
git switch ipad-solidcraft
git pull --ff-only
git switch -c update-solidcraft
git subtree pull --prefix=upstream-solidcraft upstream main
```

Resolve conflicts while preserving the iPad changes, run the checks in `upstream-solidcraft/AGENTS.md`, rebuild the bundled web engine, and validate the installed app's Pencil input, navigation, selection, menus and file handling before merging. Review upstream release and dependency changes; don't assume a conflict-free merge proves runtime compatibility.

Build directories, device test bundles, local verification media/logs, imported drawings, original downloaded bundles, and signing credentials stay local. Committed verification Markdown may reference those local artifacts.

## Licensing

SolidCraft is licensed under MIT OR Apache-2.0; its history, notices, licenses and attribution remain in `upstream-solidcraft/`. The Apple shell additions use the same MIT OR Apache-2.0 choice; see the root license files. Bundled third-party fonts and assets retain their respective licenses and attribution in the app's `Web/` directory. SolidCraft remains an independent project; this repository contains our iPad adaptation.

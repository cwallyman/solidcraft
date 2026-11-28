# SolidCraft on iPad (experimental)

The desktop app (`apps/solidcraft`) also builds for iPadOS. The CAD engine, the UI and the GPU
viewport are the same code as on macOS; only the parts that need a desktop are left out or stand
in differently. This is an early port: build it yourself and install it on your own iPad.

## Build and install

You need a Mac with Xcode (26 or later, for the Icon Composer app icon), Rust from
[rustup](https://rustup.rs), and an Apple Account added to Xcode (a free one is enough for your
own iPad).

1. Connect the iPad, unlock it and trust the Mac. Turn on
   **Settings › Privacy & Security › Developer Mode** (the iPad restarts).
2. Open `packaging/ios/SolidCraft-iPad.xcodeproj` in Xcode.
3. Select the **SolidCraft** target › **Signing & Capabilities**: choose your team, and change the
   bundle identifier to one of your own (for example `com.example.solidcraft`):
   `io.tangencylabs.solidcraft` belongs to the project's team.
4. Choose your iPad as the run destination and press **Run**. The first build compiles the whole
   workspace for iPadOS and takes a while; later builds are incremental.
5. The first launch on the iPad says the developer isn't trusted: open
   **Settings › General › VPN & Device Management**, choose your Apple Account and trust it.

From the command line, with the team and bundle id given:

```sh
xcodebuild -project packaging/ios/SolidCraft-iPad.xcodeproj -scheme SolidCraft \
  -configuration Debug -destination 'platform=iOS,name=<your iPad>' \
  -allowProvisioningUpdates DEVELOPMENT_TEAM=<team id> PRODUCT_BUNDLE_IDENTIFIER=<your id> build
```

The Xcode project has no sources of its own: its build phase (`packaging/ios/build-rust.sh`)
runs `cargo build --locked -p solidcraft --target aarch64-apple-ios` (or `aarch64-apple-ios-sim`
for the Simulator) and puts the binary in the app bundle, which Xcode then signs. As
`packaging/macos/package.sh` does for the Mac, it adds the licence files, stamps the version from
`Cargo.toml` into the Info.plist (`assets/platforms/ios/Info.plist`, beside the Mac's
`Info.plist.in`) and checks the bundle. Debug builds use Cargo's `dev` profile (optimised, with
debug info), Release (and Archive) builds `release`. Only the target for the run destination is
built. On a Mac with 8 GB of memory a first Debug build takes about 35 minutes and a rebuild after
a one-line edit in the UI about 2 minutes; the `profiling` profile was slower for both, and
capping Cargo's jobs (`CARGO_BUILD_JOBS=4` in the environment or as an `xcodebuild` setting) did
not help. The bundle id is the Mac app's, `io.tangencylabs.solidcraft`,
so the two can become one App Store app.

To check that the code builds for iPadOS without Xcode:

```sh
cargo check -p solidcraft --target aarch64-apple-ios
```

## Files

**Open**, **Import**, **Save** and **Choose folder** show the iPad's document picker (the Files
browser): iCloud Drive, On My iPad, or another file provider.

- Open and Import pick a file where it is, so **Save** writes it back there.
- Save picks the **folder** to save in, because iPadOS has no save dialog an app can name a file
  in. The design is written there under its name ("Part 2.sc" when "Part.sc" is there already),
  so nothing is replaced by mistake.
- The app's **Documents** folder (On My iPad › SolidCraft in the Files app) is the Library's
  folder on Home.
- Choosing a program (a slicer) isn't possible; that answers as cancelled.

## Apple Pencil and touch

**Pencil mode** (Preferences › Sketch) is on by default on iPad and works as in the web app:

- **Pencil tap** on an item adds it to the selection, or takes it out; a tap on empty space clears
  the selection. A **Pencil double-tap** selects the whole body or sketch.
- **Fingers** in the view move it: one finger orbits, two pan, pinch zooms. A **finger
  double-tap** on a flat face starts a sketch on it. Fingers on panels and buttons tap them.
- A palm resting on the screen while the Pencil draws doesn't take over the Pencil.
- An **Apple Pencil Pro squeeze** is Escape: it cancels the tool or the step in progress. A
  squeeze set to be ignored or to run a shortcut (Settings › Apple Pencil) is left to iPadOS.
- **Hovering** the Pencil (or a trackpad or mouse pointer) over a button shows its tooltip.
- A **trackpad or mouse** click is a click, as on a computer: it replaces the selection, and
  Shift-click adds.

## iPad layout

**iPad layout** is on by default on iPad and off on a Mac or PC (for a touch screen, turn it on).
It is switched on Home (**iPad layout: on/off**, between Preferences and Keyboard shortcuts), in
Preferences › Interface › Window, or with the `view.touch_layout` command. With it on, the
interface is drawn a little larger and a tap a few points beside a control still reaches it.

## Typing values

While a sketch tool draws, its value boxes (Length, Angle, Width…) stay beside the shape and keep
following it, as on the desktop. The on-screen keyboard comes up only when a box is **tapped**, so
it never covers the drawing by itself; a hardware keyboard types into the boxes as on a Mac.

With Pencil mode on, a **Pencil tap** on a value box opens a **number pad** beside it instead of
the keyboard: the digits, minus and point, ⌫, **Clear** (empties the box), **Esc** (puts the box
back as it was) and **OK** (keeps the value; once every box has one, it places the point, as Enter
does).

## What isn't there yet

| Area | On iPad | Why |
|---|---|---|
| Plugins | Refused with "not supported yet: plugins on iPad" | iPadOS doesn't let apps run code they compile (Wasmtime's Cranelift). Wasmtime's Pulley interpreter would. |
| `solidcraft-cli`, MCP, CLI agents in the assistant | Not available | iPadOS apps can't start other programs. API agents may work (untested). |
| 3D mouse, navigation bridge | Not available | No HID access for apps. |
| Native menu bar | Not available | The macOS menu becomes the iPad keyboard-shortcut menu later. |
| Updates | Not offered | Installs come from Xcode (later, the App Store). |
| Several windows, torn-off tabs | Untested | iPadOS scenes differ from desktop windows. |
| Touch-first layout | The desktop layout | An iPad layout is the next step. |

Results from running it on an iPad are below.

## Tested on an iPad

On an iPad Air 11-inch (M3) with iPadOS 27 and an Apple Pencil, built with Xcode 27.2:

- The app starts, shows the desktop layout, and opens, edits and saves designs.
- The document picker opens and saves; after cancelling it, fingers move the view straight away.
- Pencil mode: Pencil taps select, fingers orbit, pan and zoom, and Pencil hover shows tooltips.
- Sketching: the value boxes stay beside the shape without the keyboard; the number pad types into
  them.

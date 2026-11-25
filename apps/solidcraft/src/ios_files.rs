//! Files on iPadOS, where rfd has no dialogs (docs/ios.md).
//!
//! Open, Import, Save and Choose folder show the system's document picker
//! (`ios_document_picker`). The app's Documents folder, which the Files app shows under
//! On My iPad › SolidCraft (`UIFileSharingEnabled` and `LSSupportsOpeningDocumentsInPlace` in the
//! Info.plist), is the Library's folder. A program can't be picked: that answer is a cancel.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;
use solidcraft_ui_egui::{Services, SolveApp};

use crate::ios_document_picker;

/// The Library has been pointed at Documents once this run (when it had no folder of its own).
static LIBRARY_SET: AtomicBool = AtomicBool::new(false);

/// The app's Documents folder, made when it is missing.
pub fn documents() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join("Documents");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// The file services of the iPad build.
pub fn install(services: &mut Services) {
    services.pick_open = Some(Box::new(|_| ios_document_picker::pick_file()));
    services.pick_file = Some(Box::new(|_, _| ios_document_picker::pick_file()));
    services.pick_save = Some(Box::new(|name: &str, _| ios_document_picker::pick_save(name)));
    services.pick_folder = Some(Box::new(ios_document_picker::pick_folder));
    services.pick_program = None;
    services.show_in_folder = None;
}

/// Every frame: the Library starts on Documents.
pub fn tick(app: &mut SolveApp) {
    if !LIBRARY_SET.swap(true, Ordering::Relaxed) {
        let mut prefs = app.library.prefs();
        if prefs.get("root").is_none_or(serde_json::Value::is_null)
            && let Some(dir) = documents()
        {
            prefs["root"] = json!(dir);
            app.library.load(Some(&prefs));
        }
    }
}

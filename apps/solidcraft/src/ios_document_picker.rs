//! iPadOS: the system's document picker (the Files browser) for Open, Import and Save, and for
//! choosing a folder.
//!
//! Open and Import pick a file in place (`UIDocumentPickerMode::Open`): iCloud Drive, On My
//! iPad, a file provider. Save picks the folder to save in; the design is written there under the
//! name SolidCraft gives it, numbered when a file of that name is there already, as iPadOS has no
//! save dialog an app can name a file in. The app may read and write what was picked for as long
//! as it runs (`startAccessingSecurityScopedResource`).
//!
//! The picker answers through a delegate, an Objective-C object SolidCraft defines; defining one
//! and asking for access to a picked file have no safe API, hence the `unsafe_code` allowance on
//! [`PickerDelegate`] and [`answer`] (`open_documents.rs` adds a method to winit's macOS delegate
//! similarly).

use std::cell::RefCell;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSArray, NSString, NSURL};
// The picker's type-identifier form (`initWithDocumentTypes:inMode:`) is deprecated; its
// content-type form takes a UTType, from a crate SolidCraft doesn't build.
#[allow(deprecated)]
use objc2_ui_kit::UIDocumentPickerMode;
use objc2_ui_kit::{UIApplication, UIDocumentPickerViewController, UIViewController, UIWindowScene};
use solidcraft_ui_egui::FilePickerFuture;

/// Any file: SolidCraft checks what it was given by its extension, as it does on the desktop.
const ANY_FILE: &str = "public.item";
/// A folder.
const FOLDER: &str = "public.folder";

/// What the picker is for.
#[derive(Clone, Debug)]
enum Pick {
    /// A file to open or import.
    File,
    /// A folder to save `name` in.
    SaveIn { name: String },
    /// A folder.
    Folder,
}

/// The picker's answer, for the future that waits for it.
#[derive(Default)]
struct Slot {
    /// `Some` once answered: the path, or `None` when cancelled.
    answer: Option<Option<String>>,
    waker: Option<Waker>,
}

type Shared = Rc<RefCell<Slot>>;

/// Waits for the picker's answer.
struct Answer(Shared);

impl Future for Answer {
    type Output = Option<String>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<String>> {
        let Ok(mut slot) = self.0.try_borrow_mut() else {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        };
        match slot.answer.take() {
            Some(answer) => Poll::Ready(answer),
            None => {
                slot.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

/// Answers the waiting future and wakes the UI.
fn settle(slot: &Shared, answer: Option<String>) {
    let waker = match slot.try_borrow_mut() {
        Ok(mut slot) => {
            slot.answer = Some(answer);
            slot.waker.take()
        }
        Err(_) => None,
    };
    if let Some(waker) = waker {
        waker.wake();
    }
}

/// The delegate's state: what it was asked for and where the answer goes.
struct Ivars {
    pick: Pick,
    slot: Shared,
}

mod delegate {
    // Defining an Objective-C class has no safe API (module docs).
    #![allow(unsafe_code)]

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
    use objc2_foundation::{NSArray, NSURL};
    use objc2_ui_kit::{UIDocumentPickerDelegate, UIDocumentPickerViewController};

    use super::{Ivars, answer, forget_delegate, settle};

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and `PickerDelegate` doesn't implement
        // `Drop`.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "SolidCraftDocumentPickerDelegate"]
        #[ivars = Ivars]
        pub(super) struct PickerDelegate;

        unsafe impl NSObjectProtocol for PickerDelegate {}

        unsafe impl UIDocumentPickerDelegate for PickerDelegate {
            #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
            fn did_pick(&self, _picker: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
                let ivars = self.ivars();
                settle(&ivars.slot, urls.firstObject().and_then(|url| answer(&url, &ivars.pick)));
                forget_delegate();
            }

            #[unsafe(method(documentPickerWasCancelled:))]
            fn cancelled(&self, _picker: &UIDocumentPickerViewController) {
                settle(&self.ivars().slot, None);
                forget_delegate();
            }
        }
    );

    impl PickerDelegate {
        pub(super) fn new(mtm: MainThreadMarker, ivars: Ivars) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(ivars);
            // SAFETY: `init` on an NSObject subclass whose ivars are set.
            unsafe { msg_send![super(this), init] }
        }
    }
}

use delegate::PickerDelegate;

thread_local! {
    /// The delegate of the picker on screen: a picker holds its delegate weakly.
    static DELEGATE: RefCell<Option<Retained<PickerDelegate>>> = const { RefCell::new(None) };
}

fn forget_delegate() {
    DELEGATE.with(|delegate| {
        if let Ok(mut delegate) = delegate.try_borrow_mut() {
            *delegate = None;
        }
    });
}

/// The path to give the app for the picked `url`, with access to it for as long as the app runs.
#[allow(unsafe_code)]
fn answer(url: &NSURL, pick: &Pick) -> Option<String> {
    // SAFETY: `url` is the picker's, as it requires; the access is never given back, so files
    // opened from it can be saved again later.
    let _ = unsafe { url.startAccessingSecurityScopedResource() };
    let picked = PathBuf::from(url.path()?.to_string());
    let path = match pick {
        Pick::File | Pick::Folder => picked,
        Pick::SaveIn { name } => unused_name(&picked, name),
    };
    Some(path.to_string_lossy().into_owned())
}

/// `name` in `folder`, numbered ("Part 2.sc") when a file of that name is there already.
fn unused_name(folder: &Path, name: &str) -> PathBuf {
    let name = Path::new(name).file_name().map_or_else(|| "Untitled".into(), |n| n.to_string_lossy().into_owned());
    let first = folder.join(&name);
    if !first.exists() {
        return first;
    }
    let file = Path::new(&name);
    let stem = file.file_stem().map_or_else(|| name.clone(), |s| s.to_string_lossy().into_owned());
    let ext = file.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..1000).map(|n| folder.join(format!("{stem} {n}{ext}"))).find(|path| !path.exists()).unwrap_or(first)
}

/// The view controller on top, to present the picker over.
fn top_view_controller(mtm: MainThreadMarker) -> Option<Retained<UIViewController>> {
    let app = UIApplication::sharedApplication(mtm);
    let scene = app.connectedScenes().allObjects().iter().find_map(|scene| scene.downcast::<UIWindowScene>().ok())?;
    let window = scene.keyWindow().or_else(|| scene.windows().firstObject())?;
    let mut top = window.rootViewController()?;
    while let Some(next) = top.presentedViewController() {
        top = next;
    }
    Some(top)
}

/// Shows the picker for `pick`; the future answers with the path, or `None` when cancelled or
/// when there is nothing to show it over.
fn present(pick: Pick) -> FilePickerFuture {
    let slot = Shared::default();
    if show(pick, &slot).is_none() {
        settle(&slot, None);
    }
    Box::pin(Answer(slot))
}

fn show(pick: Pick, slot: &Shared) -> Option<()> {
    let mtm = MainThreadMarker::new()?;
    let top = top_view_controller(mtm)?;
    let kind = match pick {
        Pick::File => ANY_FILE,
        Pick::SaveIn { .. } | Pick::Folder => FOLDER,
    };
    let types = NSArray::from_retained_slice(&[NSString::from_str(kind)]);
    #[allow(deprecated)]
    let picker =
        UIDocumentPickerViewController::initWithDocumentTypes_inMode(UIDocumentPickerViewController::alloc(mtm), &types, UIDocumentPickerMode::Open);
    let delegate = PickerDelegate::new(mtm, Ivars { pick, slot: slot.clone() });
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    DELEGATE.with(|kept| kept.try_borrow_mut().map(|mut kept| *kept = Some(delegate)).ok())?;
    top.presentViewController_animated_completion(&picker, true, None);
    Some(())
}

/// A file to open (Open, and the importers).
pub fn pick_file() -> FilePickerFuture {
    present(Pick::File)
}

/// Where to save `name`: a folder picked, and the name in it.
pub fn pick_save(name: &str) -> FilePickerFuture {
    present(Pick::SaveIn { name: name.to_owned() })
}

/// A folder.
pub fn pick_folder() -> FilePickerFuture {
    present(Pick::Folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_never_replaces_a_file_already_there() {
        let dir = std::env::temp_dir().join(format!("solidcraft-ios-picker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unused_name(&dir, "Part.sc"), dir.join("Part.sc"));
        std::fs::write(dir.join("Part.sc"), b"").unwrap();
        assert_eq!(unused_name(&dir, "Part.sc"), dir.join("Part 2.sc"));
        std::fs::write(dir.join("Part 2.sc"), b"").unwrap();
        assert_eq!(unused_name(&dir, "../elsewhere/Part.sc"), dir.join("Part 3.sc"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

//! macOS URL scheme handler for `enya://` deep links.
//!
//! Registers an Apple Event handler for `kInternetEventClass`/`kAEGetURL` so that
//! when macOS opens an `enya://snapshot/<id>` URL, the native app receives it.
//!
//! Call [`init_url_handler`] before starting the eframe event loop, then poll
//! [`drain_pending_urls`] each frame to retrieve any received URLs.

use std::sync::OnceLock;
use std::sync::mpsc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send, msg_send_id, mutability, sel};
use objc2_foundation::{NSAppleEventManager, NSObject, NSObjectProtocol};

use parking_lot::Mutex;

/// Buffered channel for URLs received from macOS Apple Events.
static URL_CHANNEL: OnceLock<(mpsc::Sender<String>, Mutex<mpsc::Receiver<String>>)> =
    OnceLock::new();

/// Apple Event constants.
/// `kInternetEventClass` = 'GURL' and `kAEGetURL` = 'GURL'.
const K_INTERNET_EVENT_CLASS: u32 = 0x4755524C;
const K_AE_GET_URL: u32 = 0x4755524C;
/// `keyDirectObject` = '----' (the direct parameter of an Apple Event).
const KEY_DIRECT_OBJECT: u32 = 0x2D2D2D2D;

declare_class!(
    struct UrlEventHandler;

    unsafe impl ClassType for UrlEventHandler {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "EnyaUrlEventHandler";
    }

    impl DeclaredClass for UrlEventHandler {}

    unsafe impl NSObjectProtocol for UrlEventHandler {}

    unsafe impl UrlEventHandler {
        /// Callback for `kAEGetURL` Apple Events.
        /// Signature: `-(void)handleGetURLEvent:(NSAppleEventDescriptor*)event withReplyEvent:(NSAppleEventDescriptor*)reply`
        #[method(handleGetURLEvent:withReplyEvent:)]
        fn handle_get_url_event(&self, event: &AnyObject, _reply: &AnyObject) {
            // Extract the direct object parameter (the URL string) from the Apple Event.
            // event.paramDescriptorForKeyword_(keyDirectObject).stringValue
            unsafe {
                let direct_param: *mut AnyObject =
                    msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT];
                if direct_param.is_null() {
                    log::warn!("URL event had no direct object parameter");
                    return;
                }
                let ns_string: *mut AnyObject = msg_send![direct_param, stringValue];
                if ns_string.is_null() {
                    log::warn!("URL event direct object had no string value");
                    return;
                }
                // Convert NSString to Rust String via UTF-8 bytes.
                let utf8: *const u8 = msg_send![ns_string, UTF8String];
                if utf8.is_null() {
                    log::warn!("URL event string UTF8String was null");
                    return;
                }
                let c_str = std::ffi::CStr::from_ptr(utf8 as *const std::ffi::c_char);
                if let Ok(url) = c_str.to_str() {
                    log::info!("Received URL via Apple Event: {url}");
                    if let Some((tx, _)) = URL_CHANNEL.get() {
                        let _ = tx.send(url.to_string());
                    }
                }
            }
        }
    }
);

impl UrlEventHandler {
    fn new() -> Retained<Self> {
        unsafe { msg_send_id![Self::alloc(), init] }
    }
}

/// Initialize the macOS URL scheme handler.
///
/// Must be called on the main thread before `eframe::run_native()`.
/// Registers an Apple Event handler for `kInternetEventClass`/`kAEGetURL`
/// which catches `enya://` URLs opened by the OS.
pub fn init_url_handler() {
    // Set up the buffered channel.
    URL_CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (tx, Mutex::new(rx))
    });

    let handler = UrlEventHandler::new();

    // Register with NSAppleEventManager for GURL/GURL events.
    unsafe {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        let sel: Sel = sel!(handleGetURLEvent:withReplyEvent:);
        let _: () = msg_send![
            &*manager,
            setEventHandler: &*handler
            andSelector: sel
            forEventClass: K_INTERNET_EVENT_CLASS
            andEventID: K_AE_GET_URL
        ];
    }

    // Prevent handler from being deallocated — it must live for the app lifetime.
    std::mem::forget(handler);

    log::info!("macOS URL scheme handler registered for enya://");
}

/// Drain any URLs received since the last call. Non-blocking.
///
/// Call this each frame from `EnyaApp::update()` to process incoming deep links.
pub fn drain_pending_urls() -> Vec<String> {
    let mut urls = Vec::new();
    if let Some((_, rx)) = URL_CHANNEL.get() {
        let rx = rx.lock();
        while let Ok(url) = rx.try_recv() {
            urls.push(url);
        }
    }
    urls
}

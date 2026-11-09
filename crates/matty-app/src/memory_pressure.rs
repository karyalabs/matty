//! The system's memory pressure warnings (#794). On macOS a dispatch
//! source reports when memory runs short; the app then lets go of every
//! decoded picture off screen and of the encoded bytes it holds, which it
//! can get back from disk. Other platforms send no warning.

use gpui_kit::App;

/// Starts listening for memory pressure warnings, for the life of the app.
pub fn watch(cx: &mut App) {
    let Some(warnings) = platform::listen() else {
        return;
    };
    cx.spawn(async move |cx| {
        while warnings.recv().await.is_ok() {
            tracing::info!("memory pressure: letting decoded images go");
            cx.update(crate::decoded_images::relieve);
        }
    })
    .detach();
}

// `DispatchSource::new` in `dispatch2` is unsafe too, so the source is
// made through the C calls directly, the app's only unsafe code.
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "dispatch has no safe memory-pressure source")]
mod platform {
    use std::ffi::c_void;
    use std::sync::OnceLock;

    /// `DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL`.
    const WARN_OR_CRITICAL: usize = 0x2 | 0x4;
    /// `QOS_CLASS_UTILITY`, for the queue the handler runs on.
    const QOS_CLASS_UTILITY: isize = 0x11;

    unsafe extern "C" {
        static _dispatch_source_type_memorypressure: c_void;
        fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut c_void;
        fn dispatch_source_create(
            kind: *const c_void,
            handle: usize,
            mask: usize,
            queue: *mut c_void,
        ) -> *mut c_void;
        fn dispatch_source_set_event_handler_f(
            source: *mut c_void,
            handler: extern "C" fn(*mut c_void),
        );
        fn dispatch_resume(object: *mut c_void);
    }

    static WARNINGS: OnceLock<async_channel::Sender<()>> = OnceLock::new();

    extern "C" fn warned(_context: *mut c_void) {
        if let Some(warnings) = WARNINGS.get() {
            // One waiting warning is enough: a full channel drops the rest.
            let _ = warnings.try_send(());
        }
    }

    pub(super) fn listen() -> Option<async_channel::Receiver<()>> {
        let (sender, receiver) = async_channel::bounded(1);
        WARNINGS.set(sender).ok()?;
        // SAFETY: the source type is a static the system exports, the
        // queue a global one, and the source is never released, so the
        // handler, a plain function, stays valid for the app's life.
        unsafe {
            let queue = dispatch_get_global_queue(QOS_CLASS_UTILITY, 0);
            let source = dispatch_source_create(
                &raw const _dispatch_source_type_memorypressure,
                0,
                WARN_OR_CRITICAL,
                queue,
            );
            if source.is_null() {
                return None;
            }
            dispatch_source_set_event_handler_f(source, warned);
            dispatch_resume(source);
        }
        Some(receiver)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub(super) fn listen() -> Option<async_channel::Receiver<()>> {
        None
    }
}

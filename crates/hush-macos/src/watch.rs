//! Counting each app's accessibility events, which is how Hush knows when an
//! app has finished reacting (see `hush_core::settle`).
//!
//! One background thread runs a Core Foundation run loop holding one
//! AXObserver per watched app. The callback only bumps a counter, so it never
//! slows the app or the run loop down.
//!
//! Every app Hush lists also gets a light observer for new windows only. A
//! window that opens on another Space never shows up in `AXWindows`
//! (see `windows`), but its creation notification carries its element, and
//! receiving it gives the window an element id that later token lookups
//! find.

use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use hush_core::Activity;
use objc2_application_services::{AXError, AXObserver, AXUIElement};
use objc2_core_foundation::{
    CFRetained, CFRunLoop, CFRunLoopRunResult, CFString, kCFRunLoopDefaultMode,
};

use crate::ax::Element;
use crate::cf;

/// Notifications that mean "the UI changed". Registered on the application
/// element, they cover every element of the app.
const NOTIFICATIONS: [&str; 17] = [
    "AXValueChanged",
    "AXUIElementDestroyed",
    "AXCreated",
    "AXFocusedUIElementChanged",
    "AXFocusedWindowChanged",
    "AXSelectedChildrenChanged",
    "AXSelectedTextChanged",
    "AXSelectedRowsChanged",
    "AXRowCountChanged",
    "AXTitleChanged",
    "AXLayoutChanged",
    "AXWindowCreated",
    "AXMenuOpened",
    "AXMenuClosed",
    "AXResized",
    "AXLoadComplete",
    "AXLiveRegionChanged",
];

type Counts = Mutex<HashMap<i32, Activity>>;
type Created = Mutex<HashMap<i32, Vec<SentElement>>>;

/// An element handed from the event thread to the caller's thread.
struct SentElement(Element);

// SAFETY: an AXUIElement is an immutable reference to a remote UI object;
// the Accessibility API may be called on it from any thread.
unsafe impl Send for SentElement {}

/// What one observer's callback does with its notifications.
enum Counter {
    /// Counts every notification for the settle wait.
    Activity { pid: i32, counts: Arc<Counts> },
    /// Keeps the element of each window the app creates.
    Windows { pid: i32, created: Arc<Created> },
}

/// The run loop handle, which is safe to use from any thread for adding
/// sources and waking it.
struct Loop(CFRetained<CFRunLoop>);

// SAFETY: CFRunLoopAddSource and CFRunLoopWakeUp are documented thread-safe.
unsafe impl Send for Loop {}

pub(crate) struct Watcher {
    counts: Arc<Counts>,
    created: Arc<Created>,
    run_loop: Loop,
    thread: Thread,
    /// Observers stay alive (and registered) for as long as the watcher.
    observers: Mutex<HashMap<i32, CFRetained<AXObserver>>>,
    window_observers: Mutex<HashMap<i32, CFRetained<AXObserver>>>,
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watcher").finish_non_exhaustive()
    }
}

impl Watcher {
    pub(crate) fn start() -> Watcher {
        let (sender, receiver) = mpsc::channel();
        let handle = thread::Builder::new()
            .name("hush-ax-events".into())
            .spawn(move || {
                let current = CFRunLoop::current().expect("every thread has a run loop");
                let _ = sender.send(Loop(current));
                loop {
                    // SAFETY: the default mode constant is always set.
                    let mode = unsafe { kCFRunLoopDefaultMode };
                    let result = CFRunLoop::run_in_mode(mode, 3600.0, false);
                    // With no observer added yet the loop returns at once;
                    // wait for `watch` to unpark the thread.
                    if result == CFRunLoopRunResult::Finished {
                        thread::park_timeout(Duration::from_secs(1));
                    }
                }
            })
            .expect("spawn the accessibility event thread");
        let run_loop = receiver.recv().expect("event thread starts");
        Watcher {
            counts: Arc::default(),
            created: Arc::default(),
            run_loop,
            thread: handle.thread().clone(),
            observers: Mutex::default(),
            window_observers: Mutex::default(),
        }
    }

    /// Starts counting events for `pid` if it isn't counted yet.
    pub(crate) fn watch(&self, pid: i32) {
        let mut observers = self.observers.lock().expect("observer lock");
        if observers.contains_key(&pid) {
            return;
        }
        let counter = Counter::Activity {
            pid,
            counts: Arc::clone(&self.counts),
        };
        let Some(observer) = self.observe(pid, &NOTIFICATIONS, counter) else {
            return;
        };
        self.counts
            .lock()
            .expect("counts lock")
            .entry(pid)
            .or_default();
        observers.insert(pid, observer);
    }

    /// Starts keeping the windows `pid` creates, if it isn't yet.
    pub(crate) fn watch_windows(&self, pid: i32) {
        let mut observers = self.window_observers.lock().expect("observer lock");
        if observers.contains_key(&pid) {
            return;
        }
        let counter = Counter::Windows {
            pid,
            created: Arc::clone(&self.created),
        };
        if let Some(observer) = self.observe(pid, &["AXWindowCreated"], counter) {
            observers.insert(pid, observer);
        }
    }

    /// The windows `pid` created since the last call.
    pub(crate) fn take_created(&self, pid: i32) -> Vec<Element> {
        self.created
            .lock()
            .expect("created lock")
            .remove(&pid)
            .unwrap_or_default()
            .into_iter()
            .map(|sent| sent.0)
            .collect()
    }

    fn observe(
        &self,
        pid: i32,
        notifications: &[&str],
        counter: Counter,
    ) -> Option<CFRetained<AXObserver>> {
        let mut raw: *mut AXObserver = ptr::null_mut();
        // SAFETY: valid callback and out-pointer.
        let status = unsafe { AXObserver::create(pid, Some(callback), NonNull::from(&mut raw)) };
        let raw = NonNull::new(raw).filter(|_| status == AXError::Success)?;
        // SAFETY: AXObserverCreate follows the create rule.
        let observer = unsafe { CFRetained::from_raw(raw) };

        // The counter lives as long as the process: at most two per app.
        let counter = Box::into_raw(Box::new(counter));
        let app = Element::application(pid);
        for name in notifications {
            let notification: CFRetained<CFString> = cf::cfstr(name);
            // SAFETY: observer and element are live; `counter` outlives them.
            let _ = unsafe { observer.add_notification(app.raw(), &notification, counter.cast()) };
        }
        // SAFETY: getting the source of a live observer.
        let source = unsafe { observer.run_loop_source() };
        // SAFETY: the default mode constant is always set.
        self.run_loop
            .0
            .add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });
        self.run_loop.0.wake_up();
        self.thread.unpark();
        Some(observer)
    }

    pub(crate) fn activity(&self, pid: i32) -> Activity {
        self.counts
            .lock()
            .expect("counts lock")
            .get(&pid)
            .copied()
            .unwrap_or_default()
    }
}

unsafe extern "C-unwind" fn callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    _notification: NonNull<CFString>,
    refcon: *mut c_void,
) {
    // SAFETY: refcon is the leaked `Counter` registered with this observer.
    match unsafe { &*refcon.cast::<Counter>() } {
        Counter::Activity { pid, counts } => {
            if let Ok(mut counts) = counts.lock() {
                let activity = counts.entry(*pid).or_default();
                activity.events += 1;
                activity.last = Some(Instant::now());
            }
        }
        // Registered for window creation only, so every call is a new window.
        Counter::Windows { pid, created } => {
            // SAFETY: the element is live for the callback; retaining it
            // keeps it past the call.
            let element = Element::from_raw(unsafe { CFRetained::retain(element) });
            if let Ok(mut created) = created.lock() {
                let windows = created.entry(*pid).or_default();
                // Bounded for an app nobody asks about: the oldest go first.
                if windows.len() >= 64 {
                    windows.remove(0);
                }
                windows.push(SentElement(element));
            }
        }
    }
}

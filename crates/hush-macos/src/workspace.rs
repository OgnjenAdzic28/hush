//! App launches and switches, for every app on the machine.
//!
//! The per-app accessibility observers (see `watch`) only cover apps Hush
//! has listed. An app the user opens later, or quits and opens again, is a
//! new process none of them watch, so its window could reopen in the lane
//! (where the app last saved it) and stay there. NSWorkspace reports every
//! launch and every real switch, whichever app it is.
//!
//! The same reports say which apps are the user's: the ones running when
//! Hush started and the ones the user has switched to since. An app an
//! agent launched in the background is neither until the user goes to it.
//!
//! NSWorkspace posts these on the main run loop only, so they arrive in the
//! host, whose main thread runs it (`crate::serve_main_thread`). A process
//! that doesn't run it still has the accessibility observers.

use std::ptr::NonNull;
use std::sync::{Mutex, PoisonError};

use block2::RcBlock;
use objc2_app_kit::{
    NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidLaunchApplicationNotification,
};
use objc2_foundation::NSNotification;

/// Calls `launched` with the pid of each app that finishes launching, and
/// tells the lane about each app the user switches to, for as long as the
/// process runs.
pub(crate) fn observe(launched: impl Fn(i32) + Send + Sync + 'static) {
    // NSWorkspace's own list goes stale without the main run loop; the
    // process list is always current.
    let running = crate::apps::running();
    lock().extend(
        running
            .iter()
            .filter_map(|app| app.pid)
            .map(|pid| pid as i32),
    );
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let on_launch = RcBlock::new(move |note: NonNull<NSNotification>| {
        if let Some(pid) = pid(note) {
            launched(pid);
        }
    });
    let on_switch = RcBlock::new(|note: NonNull<NSNotification>| {
        if let Some(pid) = pid(note) {
            let mut users = lock();
            if !users.contains(&pid) {
                users.push(pid);
            }
            drop(users);
            crate::lane::user_switched_to(pid);
        }
    });
    // SAFETY: the names are AppKit's constants, no object filter is given,
    // and both blocks are `Send + Sync`, as a nil queue (the posting
    // thread) needs. The observer tokens are kept by the center for as long
    // as the process.
    unsafe {
        let _ = center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            None,
            &on_launch,
        );
        let _ = center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            None,
            &on_switch,
        );
    }
}

fn pid(note: NonNull<NSNotification>) -> Option<i32> {
    // SAFETY: the center passes a live notification for the block's call.
    let info = unsafe { note.as_ref() }.userInfo()?;
    // SAFETY: AppKit's key, whose value is the NSRunningApplication.
    let app = info.objectForKey(unsafe { NSWorkspaceApplicationKey })?;
    let app = app.downcast::<NSRunningApplication>().ok()?;
    Some(app.processIdentifier())
}

/// Whether `pid` is one of the user's apps (see the module comment). In a
/// process that doesn't run the main run loop, only the apps that were
/// running when it started are.
pub(crate) fn is_users(pid: i32) -> bool {
    lock().contains(&pid)
}

/// The user's apps. Pids are only added, at most one per app the user
/// opens, and a reused pid belongs to a process that started later, which
/// the user opened or switched to as well, or an agent did, rarely.
static USERS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

fn lock() -> std::sync::MutexGuard<'static, Vec<i32>> {
    USERS.lock().unwrap_or_else(PoisonError::into_inner)
}

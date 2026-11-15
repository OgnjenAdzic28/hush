//! A server killed by a signal (an agent closing its MCP session, `kill`)
//! still cleans up: apps it moved to the lane are hidden before the lane
//! display goes away with the process, so their windows don't land on the
//! user's screen.

#[cfg(unix)]
pub(crate) fn clean_up_on_exit(daemon: bool) {
    use std::mem::MaybeUninit;
    // SAFETY: plain signal-set calls on a local set; the mask applies to this
    // thread and every thread started after it, and one thread waits for
    // the signals with sigwait.
    unsafe {
        let mut set = MaybeUninit::<libc::sigset_t>::uninit();
        libc::sigemptyset(set.as_mut_ptr());
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::sigaddset(set.as_mut_ptr(), signal);
        }
        let set = set.assume_init();
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        std::thread::spawn(move || {
            let mut signal = 0;
            libc::sigwait(&set, &mut signal);
            #[cfg(target_os = "macos")]
            hush_macos::release_lane();
            if daemon {
                let _ = std::fs::remove_file(crate::daemon::socket());
            }
            std::process::exit(128 + signal);
        });
    }
}

#[cfg(not(unix))]
pub(crate) fn clean_up_on_exit(_daemon: bool) {}

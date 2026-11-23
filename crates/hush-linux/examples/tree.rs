//! Prints what Hush reads from a running app's window, node by node, before
//! the outline leaves anything out. For when an app's outline looks wrong:
//!
//!     cargo run -p hush-linux --example tree -- "Google Chrome"

#[cfg(target_os = "linux")]
fn main() {
    use hush_core::{Platform, States};
    let name = std::env::args().nth(1).expect("give an app name");
    let platform = hush_linux::LinuxPlatform::new();
    let apps = platform.apps(Some(&name)).expect("apps");
    let app = apps.into_iter().find(|a| a.pid.is_some()).expect("no running app by that name");
    for window in platform.windows(&app).expect("windows") {
        let snapshot = platform.read(&app, &window).expect("read");
        println!("window {:?} frame {:?}", window.title, snapshot.window.frame);
        for (i, node) in snapshot.nodes.iter().enumerate() {
            let depth = std::iter::successors(node.parent, |p| snapshot.nodes[*p].parent).count();
            println!(
                "{:indent$}{i} {:?} {:?} frame={:?} offscreen={} unread={}",
                "",
                node.role,
                node.name,
                node.frame,
                node.states.has(States::OFFSCREEN),
                node.unread,
                indent = depth * 2
            );
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}

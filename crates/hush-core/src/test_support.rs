//! Small builders for snapshots in tests. Handles are plain numbers.

use crate::model::{Actions, AppInfo, Node, Rect, Role, Snapshot, States, WindowInfo};

pub fn node(handle: u32, role: Role, name: &str, parent: Option<usize>) -> Node<u32> {
    Node {
        handle,
        role,
        name: name.to_owned(),
        value: None,
        frame: None,
        states: States::default(),
        actions: Actions::default(),
        parent,
        unread: 0,
    }
}

pub fn framed(mut node: Node<u32>, x: f64, y: f64) -> Node<u32> {
    node.frame = Some(Rect {
        x,
        y,
        w: 80.0,
        h: 24.0,
    });
    node
}

pub fn snapshot(nodes: Vec<Node<u32>>) -> Snapshot<u32> {
    Snapshot::new(
        AppInfo {
            name: "App".into(),
            pid: Some(1),
            id: None,
            path: None,
            frontmost: false,
        },
        WindowInfo {
            id: 1,
            title: nodes.first().map(|n| n.name.clone()).unwrap_or_default(),
            frame: Rect {
                x: 0.0,
                y: 0.0,
                w: 800.0,
                h: 600.0,
            },
            main: true,
            minimized: false,
        },
        nodes,
        Vec::new(),
    )
}

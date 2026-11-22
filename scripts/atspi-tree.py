#!/usr/bin/env python3
"""Prints the raw AT-SPI tree of running apps, to see what an app reports
when Hush's outline leaves something out. Needs python3-pyatspi.

    scripts/atspi-tree.py [APP_NAME_PART] [DEPTH]
"""
import sys

import pyatspi

want = sys.argv[1].lower() if len(sys.argv) > 1 else ""
depth_limit = int(sys.argv[2]) if len(sys.argv) > 2 else 12
seen = 0


def show(node, depth):
    global seen
    seen += 1
    if seen > 600 or depth > depth_limit:
        return
    try:
        states = [pyatspi.stateToString(s) for s in node.getState().getStates()]
        keep = [s for s in states if s in ("showing", "visible", "sensitive", "editable", "focused", "multi-line", "focusable")]
        try:
            ext = node.queryComponent().getExtents(pyatspi.WINDOW_COORDS)
            box = f"{ext.x},{ext.y} {ext.width}x{ext.height}"
        except NotImplementedError:
            box = "-"
        print(f"{'  ' * depth}{node.getRoleName()}({node.getRole()}) {node.name!r} [{' '.join(keep)}] {box} children={node.childCount}")
        if node.getRole() == pyatspi.ROLE_MENU_BAR:
            return
        for i in range(min(node.childCount, 40)):
            show(node.getChildAtIndex(i), depth + 1)
    except Exception as e:  # an element that went away mid-walk
        print(f"{'  ' * depth}! {e}")


desktop = pyatspi.Registry.getDesktop(0)
print(f"apps on the bus: {[a.name for a in desktop if a is not None]}")
for app in desktop:
    if app is not None and want in app.name.lower():
        show(app, 0)

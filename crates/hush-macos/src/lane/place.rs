//! Where windows are: reading and setting a window's position, and the
//! geometry of moving one between the lane and the main display.

use hush_core::{Error, Rect, Result};
use objc2_core_foundation::{CFRetained, CGPoint};

use crate::ax::Element;
use crate::{act, cf};

/// The same place on `main` as `at` has in the lane, kept clear of the menu
/// bar and far enough from the edges that the window can be grabbed.
pub(super) fn onto(at: CGPoint, lane: Rect, main: Rect) -> CGPoint {
    let keep = |offset: f64, low: f64, size: f64| offset.clamp(low, (size - 200.0).max(low));
    CGPoint {
        x: main.x + keep(at.x - lane.x, 0.0, main.w),
        y: main.y + keep(at.y - lane.y, 40.0, main.h),
    }
}

pub(super) fn within(at: CGPoint, bounds: Rect) -> bool {
    at.x >= bounds.x && at.y >= bounds.y && at.x < bounds.x + bounds.w && at.y < bounds.y + bounds.h
}

pub(super) fn inside(window: &Element, bounds: Rect) -> bool {
    position(window).is_some_and(|at| within(at, bounds))
}

pub(super) fn set_position(window: &Element, at: CGPoint) -> Result<()> {
    // SAFETY: AXValueCreate copies the point.
    let value = unsafe {
        objc2_application_services::AXValue::new(
            objc2_application_services::AXValueType::CGPoint,
            std::ptr::NonNull::from(&at).cast(),
        )
    }
    .ok_or_else(|| Error::Failed("couldn't build a position".into()))?;
    let value: CFRetained<objc2_core_foundation::CFType> = value.into();
    act::set(window, "AXPosition", &value)
}

pub(super) fn position(window: &Element) -> Option<CGPoint> {
    window.value("AXPosition").ok().and_then(|v| cf::point(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANE: Rect = Rect {
        x: 1512.0,
        y: 982.0,
        w: 1920.0,
        h: 1080.0,
    };
    const MAIN: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1512.0,
        h: 982.0,
    };

    #[test]
    fn a_window_with_no_place_of_its_own_lands_where_it_was_in_the_lane() {
        let at = onto(
            CGPoint {
                x: 1612.0,
                y: 1082.0,
            },
            LANE,
            MAIN,
        );
        assert_eq!((at.x, at.y), (100.0, 100.0));
    }

    #[test]
    fn it_lands_clear_of_the_menu_bar_and_inside_the_main_display() {
        let top = onto(
            CGPoint {
                x: 1512.0,
                y: 982.0,
            },
            LANE,
            MAIN,
        );
        assert_eq!((top.x, top.y), (0.0, 40.0));
        let far = onto(
            CGPoint {
                x: 3400.0,
                y: 2000.0,
            },
            LANE,
            MAIN,
        );
        assert_eq!((far.x, far.y), (1312.0, 782.0));
    }
}

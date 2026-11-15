//! Choosing how a step is carried out. Accessibility actions come first:
//! they work on background windows and never touch the user's input.
//! Pointer events at the element's center are the fallback, still sent to
//! the target app only.

use hush_core::{Actions, Button, Direction, Error, Node, Op, Ref, Result, Role, Step, Target};

/// One way of carrying out a step, then the way to try if the app refuses
/// the first (an app that doesn't support an action says so; it isn't a
/// failure of the step).
pub(crate) struct Route<'a, H> {
    pub(crate) first: Op<'a, H>,
    pub(crate) fallback: Option<Op<'a, H>>,
}

impl<'a, H> Route<'a, H> {
    fn only(op: Op<'a, H>) -> Route<'a, H> {
        Route {
            first: op,
            fallback: None,
        }
    }
}

/// Routes every step except `wait` and `pick`, which the batch runner
/// handles because they read the window in between.
pub(crate) fn route<'a, H: 'a>(
    step: &'a Step,
    node: impl Fn(Ref) -> Result<&'a Node<H>>,
) -> Result<Route<'a, H>> {
    Ok(match step {
        Step::Click {
            target,
            button,
            count,
        } => click(target, *button, *count, &node)?,
        Step::Set { r, value } => Route::only(Op::SetValue(&node(*r)?.handle, value)),
        Step::Type { into, text } => Route::only(Op::Type {
            text,
            into: into.map(|r| node(r).map(|n| &n.handle)).transpose()?,
        }),
        Step::Key { combo, repeat } => Route::only(Op::Key {
            combo: *combo,
            repeat: *repeat,
        }),
        Step::Menu { path } => Route::only(Op::Menu(path)),
        Step::Lane => Route::only(Op::Lane),
        Step::Select { r } => Route::only(Op::Select(&node(*r)?.handle)),
        Step::Focus { r } => Route::only(Op::Focus(&node(*r)?.handle)),
        Step::Show { r } => Route::only(Op::ScrollIntoView(&node(*r)?.handle)),
        Step::Scroll {
            target,
            direction,
            amount,
        } => {
            let (x, y) = point(target, &node)?;
            let lines = i32::from(*amount);
            let (dx, dy) = match direction {
                Direction::Up => (0, -lines),
                Direction::Down => (0, lines),
                Direction::Left => (-lines, 0),
                Direction::Right => (lines, 0),
            };
            Route::only(Op::Scroll { x, y, dx, dy })
        }
        Step::Pick { .. } | Step::Wait { .. } | Step::Drag { .. } => {
            return Err(Error::Failed(
                "pick, wait and drag are run by the batch, not routed".into(),
            ));
        }
    })
}

fn click<'a, H: 'a>(
    target: &Target,
    button: Button,
    count: u8,
    node: &impl Fn(Ref) -> Result<&'a Node<H>>,
) -> Result<Route<'a, H>> {
    let pixel = |(x, y): (f64, f64)| Op::Click {
        x,
        y,
        button,
        count,
    };
    let Target::Ref(r) = target else {
        return Ok(Route::only(pixel(point(target, node)?)));
    };
    let element = node(*r)?;
    let center = element.frame.map(|f| f.center());
    let first = match (button, count) {
        (Button::Left, 1) if element.actions.has(Actions::PRESS) => Op::Press(&element.handle),
        (Button::Left, 1) if element.role.is_text_input() => Op::Focus(&element.handle),
        (Button::Left, 1) if element.actions.has(Actions::SELECT) || element.role == Role::Row => {
            Op::Select(&element.handle)
        }
        (Button::Right, 1) => Op::ShowMenu(&element.handle),
        _ => {
            let center = center.ok_or_else(|| no_frame(*r))?;
            return Ok(Route::only(pixel(center)));
        }
    };
    Ok(Route {
        first,
        fallback: center.map(pixel),
    })
}

pub(crate) fn point<'a, H: 'a>(
    target: &Target,
    node: &impl Fn(Ref) -> Result<&'a Node<H>>,
) -> Result<(f64, f64)> {
    match target {
        Target::Point(x, y) => Ok((*x, *y)),
        Target::Ref(r) => node(*r)?
            .frame
            .map(|f| f.center())
            .ok_or_else(|| no_frame(*r)),
    }
}

fn no_frame(r: Ref) -> Error {
    Error::Unsupported(format!(
        "{r} has no position on screen, so it can't be clicked by pointer"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hush_core::{Rect, States};

    fn element(role: Role, actions: Actions) -> Node<u8> {
        Node {
            handle: 7,
            role,
            name: String::new(),
            value: None,
            frame: Some(Rect {
                x: 10.0,
                y: 20.0,
                w: 100.0,
                h: 40.0,
            }),
            states: States::default(),
            actions,
            parent: None,
            unread: 0,
        }
    }

    fn routed(step: &str, node: &Node<u8>) -> (String, Option<String>) {
        let step = Step::parse(step).unwrap();
        let route = route(&step, |_| Ok(node)).unwrap();
        (
            format!("{:?}", route.first),
            route.fallback.map(|op| format!("{op:?}")),
        )
    }

    #[test]
    fn a_press_comes_first_with_a_pointer_click_at_the_center_as_fallback() {
        let button = element(Role::Button, Actions::PRESS);
        let (first, fallback) = routed("click b1", &button);
        assert_eq!(first, "Press(7)");
        assert_eq!(
            fallback.unwrap(),
            "Click { x: 60.0, y: 40.0, button: Left, count: 1 }"
        );
    }

    #[test]
    fn fields_are_focused_rows_selected_and_double_clicks_use_the_pointer() {
        assert_eq!(
            routed("click t1", &element(Role::TextField, Actions::SET_VALUE)).0,
            "Focus(7)"
        );
        assert_eq!(
            routed("click i1", &element(Role::Row, Actions::default())).0,
            "Select(7)"
        );
        let (first, fallback) = routed("double i1", &element(Role::Row, Actions::SELECT));
        assert!(
            first.starts_with("Click {") && first.contains("count: 2"),
            "{first}"
        );
        assert!(fallback.is_none());
        assert_eq!(
            routed("right b1", &element(Role::Button, Actions::PRESS)).0,
            "ShowMenu(7)"
        );
    }
}

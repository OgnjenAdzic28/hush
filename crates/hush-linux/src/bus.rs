//! The accessibility bus: one D-Bus connection to it, and the few AT-SPI
//! calls Hush makes. Plain method calls rather than generated proxies, so
//! nothing is introspected or cached behind Hush's back.

use std::sync::Arc;
use std::time::Duration;

use hush_core::Rect;
use serde::Serialize;
use serde::de::DeserializeOwned;
use zbus::blocking::Connection;
use zbus::zvariant::{DynamicType, OwnedObjectPath, OwnedValue, Type, Value};

use crate::roles::StateSet;

const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
const COMPONENT: &str = "org.a11y.atspi.Component";
const TEXT: &str = "org.a11y.atspi.Text";
const VALUE: &str = "org.a11y.atspi.Value";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// How long one call may take. A healthy app answers in well under a
/// millisecond; anything near this is a hung app, and the call fails rather
/// than holding up the agent (D-Bus's own default is 25 s).
const TIMEOUT: Duration = Duration::from_secs(2);

/// `AtspiCoordType`: frames relative to the screen or to the element's window.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Coords {
    Screen = 0,
    Window = 1,
}

/// One accessible object: the app's bus name and the object's path on it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Element {
    pub(crate) bus: Arc<str>,
    pub(crate) path: Arc<str>,
}

impl Element {
    /// The registry's root, whose children are the running apps.
    pub(crate) fn desktop() -> Element {
        Element {
            bus: "org.a11y.atspi.Registry".into(),
            path: "/org/a11y/atspi/accessible/root".into(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Bus {
    connection: Connection,
}

pub(crate) type Reply<T> = std::result::Result<T, zbus::Error>;

impl Bus {
    /// Finds the session's accessibility bus and turns accessibility on, so
    /// toolkits that only expose their tree to assistive technology do.
    pub(crate) fn connect() -> std::result::Result<Bus, String> {
        let session = Connection::session().map_err(|e| {
            format!("no D-Bus session bus ({e}); Hush needs a desktop session on Linux")
        })?;
        // Best effort: on a desktop where it is already on, or where the
        // property is read-only, reading still works.
        let _ = session.call_method(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            Some(PROPERTIES),
            "Set",
            &("org.a11y.Status", "IsEnabled", Value::from(true)),
        );
        let address: String = session
            .call_method(
                Some("org.a11y.Bus"),
                "/org/a11y/bus",
                Some("org.a11y.Bus"),
                "GetAddress",
                &(),
            )
            .and_then(|reply| reply.body().deserialize())
            .map_err(|e| format!("no accessibility bus ({e}); install at-spi2-core"))?;
        let connection = zbus::blocking::connection::Builder::address(address.as_str())
            .and_then(|builder| builder.method_timeout(TIMEOUT).build())
            .map_err(|e| format!("can't connect to the accessibility bus: {e}"))?;
        Ok(Bus { connection })
    }

    fn call<B, R>(&self, element: &Element, interface: &str, method: &str, body: &B) -> Reply<R>
    where
        B: Serialize + DynamicType,
        R: DeserializeOwned + Type,
    {
        let reply = self.connection.call_method(
            Some(&*element.bus),
            &*element.path,
            Some(interface),
            method,
            body,
        )?;
        reply.body().deserialize()
    }

    fn property(&self, element: &Element, interface: &str, name: &str) -> Reply<OwnedValue> {
        self.call(element, PROPERTIES, "Get", &(interface, name))
    }

    pub(crate) fn children(&self, element: &Element) -> Reply<Vec<Element>> {
        let children: Vec<(String, OwnedObjectPath)> =
            self.call(element, ACCESSIBLE, "GetChildren", &())?;
        Ok(children
            .into_iter()
            .map(|(bus, path)| Element {
                bus: bus.into(),
                path: path.as_str().into(),
            })
            .collect())
    }

    /// The unlocalized role name, like "push button".
    pub(crate) fn role(&self, element: &Element) -> Reply<String> {
        self.call(element, ACCESSIBLE, "GetRoleName", &())
    }

    pub(crate) fn states(&self, element: &Element) -> Reply<StateSet> {
        let words: Vec<u32> = self.call(element, ACCESSIBLE, "GetState", &())?;
        Ok(StateSet::from_words(&words))
    }

    pub(crate) fn name(&self, element: &Element) -> Reply<String> {
        string(self.property(element, ACCESSIBLE, "Name")?)
    }

    pub(crate) fn description(&self, element: &Element) -> Reply<String> {
        string(self.property(element, ACCESSIBLE, "Description")?)
    }

    /// `None` when the element has no on-screen extent.
    pub(crate) fn frame(&self, element: &Element, coords: Coords) -> Reply<Option<Rect>> {
        let (x, y, w, h): (i32, i32, i32, i32) =
            self.call(element, COMPONENT, "GetExtents", &(coords as u32))?;
        Ok((w > 0 && h > 0).then_some(Rect {
            x: x.into(),
            y: y.into(),
            w: w.into(),
            h: h.into(),
        }))
    }

    pub(crate) fn text(&self, element: &Element) -> Reply<String> {
        self.call(element, TEXT, "GetText", &(0i32, -1i32))
    }

    pub(crate) fn number(&self, element: &Element) -> Reply<f64> {
        let value = self.property(element, VALUE, "CurrentValue")?;
        f64::try_from(value).map_err(zbus::Error::Variant)
    }

    /// The process behind a bus name.
    pub(crate) fn pid(&self, bus: &str) -> Reply<u32> {
        let reply = self.connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetConnectionUnixProcessID",
            &bus,
        )?;
        reply.body().deserialize()
    }
}

fn string(value: OwnedValue) -> Reply<String> {
    String::try_from(value).map_err(zbus::Error::Variant)
}

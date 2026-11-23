//! Which apps agents may read and act on, as the user wrote it:
//!
//! ```text
//! # Agents never see these.
//! never 1Password, Keychain Access, Messages
//! # Agents may read these but not act on them.
//! read Terminal, Mail
//! # With an only line, every app it doesn't name is off limits.
//! only Notes, TextEdit, Calculator, Terminal, Mail
//! ```
//!
//! Names match an app's name ignoring case, or its bundle id. `never` wins
//! over `read`, and `read` over `only`. The front end loads the file; this
//! module only decides.

use crate::model::AppInfo;
use crate::platform::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Reading the window: observe, menus, see, record.
    Read,
    /// Changing anything: act, timeline, script.
    Act,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    never: Vec<String>,
    read: Vec<String>,
    /// `None` when there is no `only` line: every app not named is open.
    only: Option<Vec<String>>,
    /// Why every app is refused: the user's file couldn't be used.
    broken: Option<String>,
}

impl Limits {
    /// Reads a limits file. A mistake is an error naming the line, so a
    /// typo never leaves apps open by accident.
    pub fn parse(text: &str) -> Result<Limits, String> {
        let mut limits = Limits::default();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (word, names) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let names: Vec<String> = names
                .split(',')
                .map(|name| name.trim().to_lowercase())
                .filter(|name| !name.is_empty())
                .collect();
            if names.is_empty() {
                return Err(format!("line {}: `{line}` names no apps", index + 1));
            }
            match word {
                "never" => limits.never.extend(names),
                "read" => limits.read.extend(names),
                "only" => limits.only.get_or_insert_with(Vec::new).extend(names),
                other => {
                    return Err(format!(
                        "line {}: `{other}` should be never, read or only",
                        index + 1
                    ));
                }
            }
        }
        Ok(limits)
    }

    /// Limits that refuse every app and say why, for when the user's file
    /// couldn't be read: refusing is safer than running without limits.
    pub fn nothing(reason: String) -> Limits {
        Limits {
            broken: Some(reason),
            ..Limits::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Limits::default()
    }

    /// The most an agent may do to `app`: `None` is nothing at all.
    pub fn allowed(&self, app: &AppInfo) -> Option<Access> {
        let named = |list: &[String]| {
            let name = app.name.to_lowercase();
            let id = app.id.as_deref().map(str::to_lowercase);
            list.iter()
                .any(|entry| *entry == name || Some(entry) == id.as_ref())
        };
        if self.broken.is_some() || named(&self.never) {
            None
        } else if named(&self.read) {
            Some(Access::Read)
        } else if self.only.as_deref().is_some_and(|only| !named(only)) {
            None
        } else {
            Some(Access::Act)
        }
    }

    /// Fails when the user's limits don't allow `access` to `app`. The
    /// message tells the agent not to look for a way around it.
    pub fn check(&self, app: &AppInfo, access: Access) -> Result<(), Error> {
        if let Some(reason) = &self.broken {
            return Err(Error::Permission(reason.clone()));
        }
        match (self.allowed(app), access) {
            (Some(Access::Act), _) | (Some(Access::Read), Access::Read) => Ok(()),
            (Some(Access::Read), Access::Act) => Err(Error::Permission(format!(
                "{} is read-only for agents: observe and see work, acting doesn't. The user set this in Hush's limits; tell them instead of working around it",
                app.name
            ))),
            (None, _) => Err(Error::Permission(format!(
                "{} is off limits to agents. The user set this in Hush's limits; tell them instead of working around it",
                app.name
            ))),
        }
    }

    /// Fails when any limits are set. A script can address any app, or run
    /// a shell command, whatever app the call names, so its target can't be
    /// checked; with limits in place, scripts are off.
    pub fn check_script(&self) -> Result<(), Error> {
        if let Some(reason) = &self.broken {
            return Err(Error::Permission(reason.clone()));
        }
        if self.is_empty() {
            return Ok(());
        }
        Err(Error::Permission(
            "scripts are off while the user has Hush limits, since a script can reach any app; use observe and act instead".into(),
        ))
    }

    /// A word for the app's line in `apps`, when it is limited.
    pub fn mark(&self, app: &AppInfo) -> Option<&'static str> {
        match self.allowed(app) {
            None => Some("off limits"),
            Some(Access::Read) => Some("read-only"),
            Some(Access::Act) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, id: &str) -> AppInfo {
        AppInfo {
            name: name.into(),
            pid: Some(1),
            id: Some(id.into()),
            path: None,
            frontmost: false,
        }
    }

    #[test]
    fn never_wins_over_read_and_read_over_only() {
        let limits = Limits::parse(
            "# mine\nnever 1Password, Messages\nread Terminal\nonly Notes, terminal, messages\n",
        )
        .unwrap();
        assert_eq!(
            limits.allowed(&app("Notes", "com.apple.Notes")),
            Some(Access::Act)
        );
        assert_eq!(
            limits.allowed(&app("Terminal", "com.apple.Terminal")),
            Some(Access::Read)
        );
        assert_eq!(
            limits.allowed(&app("Messages", "com.apple.MobileSMS")),
            None
        );
        assert_eq!(limits.allowed(&app("1Password", "com.1password")), None);
        assert_eq!(
            limits.allowed(&app("Safari", "com.apple.Safari")),
            None,
            "not in only"
        );
    }

    #[test]
    fn bundle_ids_match_and_no_file_limits_nothing() {
        let limits = Limits::parse("read com.apple.mail").unwrap();
        assert_eq!(
            limits.allowed(&app("Mail", "com.apple.mail")),
            Some(Access::Read)
        );
        assert_eq!(
            limits.allowed(&app("Notes", "com.apple.Notes")),
            Some(Access::Act)
        );
        assert!(Limits::parse("").unwrap().is_empty());
    }

    #[test]
    fn checks_say_what_is_allowed_and_mistakes_name_the_line() {
        let limits = Limits::parse("read Terminal\nnever Messages").unwrap();
        let terminal = app("Terminal", "com.apple.Terminal");
        assert!(limits.check(&terminal, Access::Read).is_ok());
        let refused = limits
            .check(&terminal, Access::Act)
            .unwrap_err()
            .to_string();
        assert!(
            refused.starts_with("permission needed: Terminal is read-only"),
            "{refused}"
        );
        assert_eq!(limits.mark(&app("Messages", "x")), Some("off limits"));
        assert_eq!(
            Limits::parse("\nallow Notes").unwrap_err(),
            "line 2: `allow` should be never, read or only"
        );
        assert!(
            Limits::parse("never")
                .unwrap_err()
                .contains("names no apps")
        );
        assert!(Limits::default().check_script().is_ok());
        assert!(
            limits
                .check_script()
                .unwrap_err()
                .to_string()
                .contains("scripts are off")
        );
    }
}

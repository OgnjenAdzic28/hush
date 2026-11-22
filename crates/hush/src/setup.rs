//! `hush setup`: checks the permissions Hush needs and installs the Hush
//! skill for the agent harnesses on this machine, so an agent can use Hush
//! right after install. Safe to run again: it updates what it installed.
//!
//! The skill goes to `~/.agents/skills`, the shared folder most agents read
//! (Codex, Cursor, Amp, Cline and others), and to Claude Code's own folder,
//! because Claude Code reads only that one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The skill ships inside the binary, so an installed `hush` can set up
/// agents without the repository.
const SKILL: &str = include_str!("../../../skills/hush/SKILL.md");

/// Codex runs shell commands in a sandbox that hides Accessibility, so hush
/// fails there even when it is allowed. This rule runs hush commands outside
/// the sandbox without asking.
const CODEX_RULE: &str = "prefix_rule(pattern=[\"hush\"], decision=\"allow\")\n";

pub(crate) const USAGE: &str = "hush setup [--skills-dir DIR]...";

#[derive(Debug, PartialEq, Eq)]
enum Installed {
    New,
    Updated,
    UpToDate,
    /// A symlink (a developer's checkout, say): left alone.
    Linked(PathBuf),
}

/// Runs setup and returns what to print, and whether Hush is ready to use.
pub(crate) fn run(args: &[String]) -> Result<(String, bool), String> {
    let mut extra = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--skills-dir" => match args.next() {
                Some(dir) => extra.push(PathBuf::from(dir)),
                None => return Err("--skills-dir needs a directory".into()),
            },
            other => return Err(format!("unknown option {other}; usage: {USAGE}")),
        }
    }

    let mut out = format!("Hush {}\n\n", env!("CARGO_PKG_VERSION"));
    let ready = permissions(&mut out);

    for (harness, dir) in skill_dirs(&extra) {
        let shown = tilde(&dir.join("hush"));
        report(
            &mut out,
            &format!("Skill for {harness}"),
            &shown,
            install_skill(&dir),
        );
    }
    if let Some(codex) = codex_home() {
        // Older versions put the skill here; Codex now reads it from
        // ~/.agents/skills, and two copies would show up twice.
        if remove_old_skill(&codex.join("skills")) {
            line(
                &mut out,
                '✓',
                "Old Codex skill",
                &format!("{} removed", tilde(&codex.join("skills").join("hush"))),
            );
        }
        let rule = codex.join("rules").join("hush.rules");
        let shown = tilde(&rule);
        report(
            &mut out,
            "Codex sandbox rule",
            &shown,
            write_file(&rule, CODEX_RULE),
        );
    }
    line(
        &mut out,
        '·',
        "MCP",
        "for harnesses without skills: run `hush mcp` (stdio)",
    );
    out.push_str(if ready {
        "\nReady. Ask your agent to do something in an app, or try: hush apps\n"
    } else {
        "\nTurn on what's marked ✗, then run hush setup again.\n"
    });
    Ok((out, ready))
}

fn line(out: &mut String, mark: char, label: &str, detail: &str) {
    out.push_str(&format!("  {mark}  {label:<24}{detail}\n"));
}

#[cfg(target_os = "macos")]
fn permissions(out: &mut String) -> bool {
    if hush_macos::sandboxed() {
        line(
            out,
            '✗',
            "Sandbox",
            "this is running inside a sandbox, which hides Accessibility; run hush setup from \
             your own terminal",
        );
        return false;
    }
    let granted = hush_macos::permissions(true);
    let app = hush_macos::host_app().unwrap_or_else(|| "the app you ran this from".into());
    let pane = "System Settings > Privacy & Security";
    if granted.accessibility {
        line(out, '✓', "Accessibility", &format!("on for {app}"));
    } else {
        line(
            out,
            '✗',
            "Accessibility",
            &format!("turn it on for {app} in {pane} > Accessibility"),
        );
    }
    if granted.screen_recording {
        line(out, '✓', "Screen Recording", &format!("on for {app}"));
    } else {
        line(
            out,
            '✗',
            "Screen Recording",
            &format!(
                "for see and record: turn it on for {app} in {pane} > Screen & System Audio \
                 Recording, then reopen {app}"
            ),
        );
    }
    granted.accessibility
}

#[cfg(target_os = "linux")]
fn permissions(out: &mut String) -> bool {
    let ready = hush_linux::readiness();
    let accessible = match &ready.accessibility {
        Ok(()) => {
            line(
                out,
                '✓',
                "Accessibility",
                "on; apps opened before now may need a restart to be readable",
            );
            true
        }
        Err(problem) => {
            line(out, '✗', "Accessibility", problem);
            false
        }
    };
    if ready.xvfb {
        line(out, '✓', "Hush's display", "Xvfb is installed");
    } else {
        line(
            out,
            '✗',
            "Hush's display",
            "to start apps, see them and use keys and the pointer: install Xvfb \
             (sudo apt install xvfb, or sudo dnf install xorg-x11-server-Xvfb)",
        );
    }
    if ready.ffmpeg {
        line(out, '✓', "Recording", "ffmpeg is installed");
    } else {
        line(
            out,
            '·',
            "Recording",
            "for record: install ffmpeg (sudo apt install ffmpeg)",
        );
    }
    accessible && ready.xvfb
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn permissions(out: &mut String) -> bool {
    line(
        out,
        '✗',
        "Platform",
        "not supported yet; Hush runs on macOS for now",
    );
    false
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// A harness's config folder, from its environment variable or the default
/// under the home folder, if it exists.
fn config_dir(var: &str, default: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(default)))
        .filter(|d| d.is_dir())
}

fn codex_home() -> Option<PathBuf> {
    config_dir("CODEX_HOME", ".codex")
}

/// Where the skill goes: Claude Code's folder if it is installed, the shared
/// agents folder always, then any folder the user named.
fn skill_dirs(extra: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut dirs = Vec::new();
    if let Some(claude) = config_dir("CLAUDE_CONFIG_DIR", ".claude") {
        dirs.push(("Claude Code".to_owned(), claude.join("skills")));
    }
    if let Some(home) = home() {
        dirs.push((
            "other agents".to_owned(),
            home.join(".agents").join("skills"),
        ));
    }
    dirs.extend(extra.iter().map(|d| ("your harness".to_owned(), d.clone())));
    dirs
}

/// Removes a Hush skill an older setup copied into `skills_dir`. Leaves
/// links and anything that isn't Hush's skill alone.
fn remove_old_skill(skills_dir: &Path) -> bool {
    let dir = skills_dir.join("hush");
    if fs::read_link(&dir).is_ok() {
        return false;
    }
    match fs::read_to_string(dir.join("SKILL.md")) {
        Ok(text) if text.starts_with("---\nname: hush\n") => fs::remove_dir_all(&dir).is_ok(),
        _ => false,
    }
}

fn install_skill(skills_dir: &Path) -> io::Result<Installed> {
    let dir = skills_dir.join("hush");
    if let Ok(target) = fs::read_link(&dir) {
        return Ok(Installed::Linked(target));
    }
    write_file(&dir.join("SKILL.md"), SKILL)
}

/// Writes `contents` to `file` unless it already holds them.
fn write_file(file: &Path, contents: &str) -> io::Result<Installed> {
    let state = match fs::read_to_string(file) {
        Ok(current) if current == contents => return Ok(Installed::UpToDate),
        Ok(_) => Installed::Updated,
        Err(_) => Installed::New,
    };
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(file, contents)?;
    Ok(state)
}

fn report(out: &mut String, label: &str, shown: &str, result: io::Result<Installed>) {
    match result {
        Ok(Installed::New) => line(out, '✓', label, shown),
        Ok(Installed::Updated) => line(out, '✓', label, &format!("{shown} (updated)")),
        Ok(Installed::UpToDate) => line(out, '✓', label, &format!("{shown} (up to date)")),
        Ok(Installed::Linked(to)) => line(
            out,
            '✓',
            label,
            &format!("{shown} (links to {}, left as is)", tilde(&to)),
        ),
        Err(error) => line(out, '✗', label, &format!("{shown}: {error}")),
    }
}

fn tilde(path: &Path) -> String {
    let text = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && text.starts_with(&home) => {
            format!("~{}", &text[home.len()..])
        }
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hush-setup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn installs_then_updates_then_leaves_alone() {
        let skills = scratch("install");
        assert_eq!(install_skill(&skills).unwrap(), Installed::New);
        assert_eq!(
            fs::read_to_string(skills.join("hush/SKILL.md")).unwrap(),
            SKILL
        );
        assert_eq!(install_skill(&skills).unwrap(), Installed::UpToDate);
        fs::write(skills.join("hush/SKILL.md"), "old").unwrap();
        assert_eq!(install_skill(&skills).unwrap(), Installed::Updated);
        fs::remove_dir_all(&skills).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_skill_is_left_alone() {
        let skills = scratch("linked");
        let checkout = skills.join("checkout");
        fs::create_dir_all(&checkout).unwrap();
        fs::write(checkout.join("SKILL.md"), "dev copy").unwrap();
        std::os::unix::fs::symlink(&checkout, skills.join("hush")).unwrap();
        assert_eq!(
            install_skill(&skills).unwrap(),
            Installed::Linked(checkout.clone())
        );
        assert_eq!(
            fs::read_to_string(checkout.join("SKILL.md")).unwrap(),
            "dev copy"
        );
        fs::remove_dir_all(&skills).unwrap();
    }

    #[test]
    fn an_old_copied_skill_is_removed_but_nothing_else() {
        let skills = scratch("old");
        install_skill(&skills).unwrap();
        assert!(remove_old_skill(&skills));
        assert!(!skills.join("hush").exists());

        fs::create_dir_all(skills.join("hush")).unwrap();
        fs::write(skills.join("hush/SKILL.md"), "someone else's").unwrap();
        assert!(!remove_old_skill(&skills));
        assert!(skills.join("hush/SKILL.md").exists());
        fs::remove_dir_all(&skills).unwrap();
    }

    #[test]
    fn the_embedded_skill_is_the_real_one() {
        assert!(SKILL.starts_with("---\nname: hush\n"));
    }
}

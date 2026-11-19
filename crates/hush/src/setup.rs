//! `hush setup`: checks the permissions Hush needs and installs the Hush
//! skill for the agent harnesses on this machine, so an agent can use Hush
//! right after install. Safe to run again: it updates what it installed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The skill ships inside the binary, so an installed `hush` can set up
/// agents without the repository.
const SKILL: &str = include_str!("../../../skills/hush/SKILL.md");

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

    let targets = harness_skill_dirs(&extra);
    if targets.is_empty() {
        line(
            &mut out,
            '-',
            "Skill",
            "no Claude Code or Codex found; for another harness: hush setup --skills-dir DIR",
        );
    }
    for (harness, dir) in &targets {
        let label = format!("Skill for {harness}");
        let shown = tilde(&dir.join("hush"));
        match install_skill(dir) {
            Ok(Installed::New) => line(&mut out, '✓', &label, &shown),
            Ok(Installed::Updated) => line(&mut out, '✓', &label, &format!("{shown} (updated)")),
            Ok(Installed::UpToDate) => {
                line(&mut out, '✓', &label, &format!("{shown} (up to date)"))
            }
            Ok(Installed::Linked(to)) => line(
                &mut out,
                '✓',
                &label,
                &format!("{shown} (links to {}, left as is)", tilde(&to)),
            ),
            Err(error) => line(&mut out, '✗', &label, &format!("{shown}: {error}")),
        }
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

#[cfg(not(target_os = "macos"))]
fn permissions(out: &mut String) -> bool {
    line(
        out,
        '✗',
        "Platform",
        "not supported yet; Hush runs on macOS for now",
    );
    false
}

/// The skills directory of each harness installed for this user, then any
/// the user named.
fn harness_skill_dirs(extra: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let config = |var: &str, default: &str| {
        std::env::var_os(var)
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(default)))
    };
    let mut dirs: Vec<(String, PathBuf)> = [
        ("Claude Code", config("CLAUDE_CONFIG_DIR", ".claude")),
        ("Codex", config("CODEX_HOME", ".codex")),
    ]
    .into_iter()
    .filter_map(|(name, dir)| Some((name.to_owned(), dir.filter(|d| d.is_dir())?.join("skills"))))
    .collect();
    dirs.extend(extra.iter().map(|d| ("your harness".to_owned(), d.clone())));
    dirs
}

fn install_skill(skills_dir: &Path) -> io::Result<Installed> {
    let dir = skills_dir.join("hush");
    if let Ok(target) = fs::read_link(&dir) {
        return Ok(Installed::Linked(target));
    }
    let file = dir.join("SKILL.md");
    let state = match fs::read_to_string(&file) {
        Ok(current) if current == SKILL => return Ok(Installed::UpToDate),
        Ok(_) => Installed::Updated,
        Err(_) => Installed::New,
    };
    fs::create_dir_all(&dir)?;
    fs::write(&file, SKILL)?;
    Ok(state)
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
    fn the_embedded_skill_is_the_real_one() {
        assert!(SKILL.starts_with("---\nname: hush\n"));
    }
}

//! Install the native sidebar files into `~/.config/cmux/sidebars/`.
//!
//! This is the product path (interpreted custom sidebar). The PTY plugin
//! manager install remains a keyboard-only fallback.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Files copied into the cmux sidebars directory. `.js` wins over `.swift`.
pub const SIDEBAR_FILES: &[&str] = &["beads.js", "beads.swift"];

/// Destination directory for interpreted custom sidebars.
#[must_use]
pub fn dest_dir() -> PathBuf {
    dest_dir_from(env::var_os("CMUX_SIDEBARS_DIR"), dirs_home())
}

fn dest_dir_from(override_dir: Option<std::ffi::OsString>, home: Option<PathBuf>) -> PathBuf {
    if let Some(custom) = override_dir.filter(|value| !value.is_empty()) {
        return PathBuf::from(custom);
    }
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config/cmux/sidebars")
}

/// Locate the packaged `sidebars/` directory next to the repo or install.
#[must_use]
pub fn source_dir() -> Option<PathBuf> {
    if let Ok(share) = env::var("CMUX_BEADS_SHARE") {
        let candidate = PathBuf::from(share).join("sidebars");
        if has_sidebar_files(&candidate) {
            return Some(candidate);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("sidebars"));
        candidates.push(dir.join("../sidebars"));
        candidates.push(dir.join("../../sidebars"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sidebars"));
    if let Some(home) = dirs_home() {
        candidates.push(home.join(".local/share/cmux/mux-plugins/cmux-beads/sidebars"));
        if let Ok(xdg) = env::var("XDG_DATA_HOME") {
            candidates.push(PathBuf::from(xdg).join("cmux/mux-plugins/cmux-beads/sidebars"));
        }
    }
    candidates.into_iter().find(|path| has_sidebar_files(path))
}

/// Copy `beads.js` and `beads.swift` into `dest`.
pub fn install_sidebars(source: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    if !has_sidebar_files(source) {
        bail!("sidebar sources missing under {}", source.display());
    }
    fs::create_dir_all(dest).with_context(|| format!("create {}", dest.display()))?;
    let mut written = Vec::new();
    for name in SIDEBAR_FILES {
        let from = source.join(name);
        let to = dest.join(name);
        fs::copy(&from, &to)
            .with_context(|| format!("copy {} → {}", from.display(), to.display()))?;
        written.push(to);
    }
    Ok(written)
}

fn has_sidebar_files(dir: &Path) -> bool {
    SIDEBAR_FILES.iter().all(|name| dir.join(name).is_file())
}

fn dirs_home() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dest_dir_honors_override() {
        assert_eq!(
            dest_dir_from(
                Some(std::ffi::OsString::from("/tmp/cmux-beads-sidebars-test")),
                Some(PathBuf::from("/home/lab"))
            ),
            PathBuf::from("/tmp/cmux-beads-sidebars-test")
        );
        assert_eq!(
            dest_dir_from(None, Some(PathBuf::from("/home/lab"))),
            PathBuf::from("/home/lab/.config/cmux/sidebars")
        );
    }

    #[test]
    fn install_copies_js_and_swift() {
        let tmp = tempfile_dir();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sidebars");
        let dest = tmp.join("sidebars");
        let written = install_sidebars(&source, &dest).expect("install");
        assert_eq!(written.len(), 2);
        assert!(dest.join("beads.js").is_file());
        assert!(dest.join("beads.swift").is_file());
        let js = fs::read_to_string(dest.join("beads.js")).unwrap();
        assert!(js.contains("Reorderable"));
        fs::remove_dir_all(tmp).ok();
    }

    fn tempfile_dir() -> PathBuf {
        let dir = env::temp_dir().join(format!("cmux-beads-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}

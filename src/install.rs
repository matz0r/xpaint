//! Installs xpaint for the current user: the binary in `~/.local/bin`, plus an icon
//! and desktop entry so it shows up in the application menu and in "Open with…".

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DESKTOP_ENTRY: &str = include_str!("../assets/xpaint.desktop");
const ICONS: [(&str, &[u8]); 3] = [
    (
        "256x256/apps/xpaint.png",
        include_bytes!("../assets/icon-256.png"),
    ),
    (
        "512x512/apps/xpaint.png",
        include_bytes!("../assets/icon-512.png"),
    ),
    (
        "scalable/apps/xpaint.svg",
        include_bytes!("../assets/icon.svg"),
    ),
];

/// Where an installation lives.
pub struct Location {
    pub binary: PathBuf,
    applications: PathBuf,
    icons: PathBuf,
}

impl Location {
    /// The standard per-user location: `~/.local/bin` and `$XDG_DATA_HOME`
    /// (`~/.local/share` by default).
    pub fn user() -> Result<Self, String> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is not set")?;
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        Ok(Self::new(&home.join(".local/bin"), &data))
    }

    fn new(bin: &Path, data: &Path) -> Self {
        Self {
            binary: bin.join("xpaint"),
            applications: data.join("applications"),
            icons: data.join("icons/hicolor"),
        }
    }

    fn desktop_entry(&self) -> PathBuf {
        self.applications.join("xpaint.desktop")
    }

    pub fn is_installed(&self) -> bool {
        self.binary.exists() && self.desktop_entry().exists()
    }

    /// Whether the running program is the installed binary.
    pub fn is_running_installed(&self) -> bool {
        let running = std::env::current_exe().and_then(fs::canonicalize);
        matches!((running, fs::canonicalize(&self.binary)), (Ok(a), Ok(b)) if a == b)
    }

    /// Installs `binary` (normally the running program) with icons and desktop entry.
    /// Replaces an existing installation.
    pub fn install(&self, binary: &Path) -> Result<(), String> {
        let same = matches!(
            (fs::canonicalize(binary), fs::canonicalize(&self.binary)),
            (Ok(a), Ok(b)) if a == b
        );
        if !same {
            // Copy next to the target and rename over it, which also works while the
            // old installed binary is running.
            let partial = self.binary.with_extension("partial");
            create_parent(&partial)?;
            fs::copy(binary, &partial).map_err(|e| error("copy", binary, e))?;
            fs::rename(&partial, &self.binary).map_err(|e| error("write", &self.binary, e))?;
        }
        for (path, bytes) in ICONS {
            write(&self.icons.join(path), bytes)?;
        }
        // Use the absolute binary path, since ~/.local/bin is not always on the
        // session's PATH.
        let exec = format!("Exec={}", self.binary.display());
        let entry = DESKTOP_ENTRY.replacen("Exec=xpaint", &exec, 1);
        write(&self.desktop_entry(), entry.as_bytes())?;
        self.refresh_caches();
        Ok(())
    }

    /// Removes what `install` added. Missing files are fine.
    pub fn uninstall(&self) -> Result<(), String> {
        let icons = ICONS.iter().map(|(path, _)| self.icons.join(path));
        for path in [self.binary.clone(), self.desktop_entry()]
            .into_iter()
            .chain(icons)
        {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error("remove", &path, e));
                }
                _ => {}
            }
        }
        self.refresh_caches();
        Ok(())
    }

    /// Lets the desktop pick up the changes right away. The tools are optional.
    fn refresh_caches(&self) {
        let _ = Command::new("gtk-update-icon-cache")
            .args(["-q", "-t"])
            .arg(&self.icons)
            .output();
        let _ = Command::new("update-desktop-database")
            .arg("-q")
            .arg(&self.applications)
            .output();
    }
}

fn create_parent(path: &Path) -> Result<(), String> {
    let dir = path.parent().expect("install paths have a parent");
    fs::create_dir_all(dir).map_err(|e| error("create", dir, e))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    create_parent(path)?;
    fs::write(path, bytes).map_err(|e| error("write", path, e))
}

fn error(action: &str, path: &Path, e: std::io::Error) -> String {
    format!("Could not {action} {}: {e}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_uninstall() {
        let dir = std::env::temp_dir().join(format!("xpaint-install-{}", std::process::id()));
        let location = Location::new(&dir.join("bin"), &dir.join("share"));
        let source = dir.join("build/xpaint");
        write(&source, b"binary").unwrap();
        assert!(!location.is_installed());

        location.install(&source).unwrap();
        assert!(location.is_installed());
        assert_eq!(fs::read(&location.binary).unwrap(), b"binary");
        let entry = fs::read_to_string(location.desktop_entry()).unwrap();
        let exec = format!("Exec={} %F", location.binary.display());
        assert!(entry.lines().any(|l| l == exec), "{entry}");
        assert!(location.icons.join("scalable/apps/xpaint.svg").exists());

        // Installing the installed binary again leaves it intact.
        location.install(&location.binary).unwrap();
        assert_eq!(fs::read(&location.binary).unwrap(), b"binary");

        location.uninstall().unwrap();
        assert!(!location.is_installed());
        assert!(!location.icons.join("256x256/apps/xpaint.png").exists());
        location.uninstall().unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }
}

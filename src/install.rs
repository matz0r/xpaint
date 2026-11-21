//! Installs xpaint for the current user so it shows up in the application menu
//! and in "Open with…".
//!
//! Linux: the binary in `~/.local/bin`, plus icons and a desktop entry.
//! Windows: the binary in `%LOCALAPPDATA%\Programs\xpaint`, a Start menu shortcut
//! and an "Open with" entry for images under `HKCU\Software\Classes`.

use std::fs;
use std::path::Path;
#[cfg(windows)]
use std::process::Command;

#[cfg(unix)]
pub use unix::Location;
#[cfg(windows)]
pub use windows::Location;

#[cfg(unix)]
mod unix {
    use super::{create_parent, error, write};
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
        pub(super) icons: PathBuf,
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

        pub(super) fn new(bin: &Path, data: &Path) -> Self {
            Self {
                binary: bin.join("xpaint"),
                applications: data.join("applications"),
                icons: data.join("icons/hicolor"),
            }
        }

        pub(super) fn desktop_entry(&self) -> PathBuf {
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
} // mod unix

#[cfg(windows)]
mod windows {
    use super::{create_parent, error, hidden};
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Image types offered in "Open with" (the formats xpaint opens).
    const EXTENSIONS: [&str; 12] = [
        ".png", ".jpg", ".jpeg", ".bmp", ".gif", ".tga", ".tif", ".tiff", ".webp", ".ico", ".qoi",
        ".jfif",
    ];
    const OPEN_WITH_KEY: &str = r"HKCU:\Software\Classes\Applications\xpaint.exe";

    /// Where an installation lives.
    pub struct Location {
        pub binary: PathBuf,
        shortcut: PathBuf,
        /// Whether to add the "Open with" registry entry (not in tests).
        register: bool,
    }

    impl Location {
        /// The standard per-user location: `%LOCALAPPDATA%\Programs\xpaint` and the
        /// user's Start menu.
        pub fn user() -> Result<Self, String> {
            let local = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .ok_or("LOCALAPPDATA is not set")?;
            let roaming = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .ok_or("APPDATA is not set")?;
            let start_menu = roaming.join(r"Microsoft\Windows\Start Menu\Programs");
            Ok(Self::new(
                &local.join(r"Programs\xpaint"),
                &start_menu,
                true,
            ))
        }

        fn new(dir: &Path, start_menu: &Path, register: bool) -> Self {
            Self {
                binary: dir.join("xpaint.exe"),
                shortcut: start_menu.join("xpaint.lnk"),
                register,
            }
        }

        /// Where a replaced binary goes: a running exe can't be deleted or
        /// overwritten on Windows, but it can be renamed.
        fn old_binary(&self) -> PathBuf {
            self.binary.with_extension("exe.old")
        }

        pub fn is_installed(&self) -> bool {
            self.binary.exists() && self.shortcut.exists()
        }

        /// Whether the running program is the installed binary.
        pub fn is_running_installed(&self) -> bool {
            let running = std::env::current_exe().and_then(fs::canonicalize);
            matches!((running, fs::canonicalize(&self.binary)), (Ok(a), Ok(b)) if a == b)
        }

        /// Installs `binary` (normally the running program) with a Start menu
        /// shortcut and the "Open with" entry. Replaces an existing installation.
        pub fn install(&self, binary: &Path) -> Result<(), String> {
            let same = matches!(
                (fs::canonicalize(binary), fs::canonicalize(&self.binary)),
                (Ok(a), Ok(b)) if a == b
            );
            if !same {
                let partial = self.binary.with_extension("partial");
                create_parent(&partial)?;
                fs::copy(binary, &partial).map_err(|e| error("copy", binary, e))?;
                // Left over from the last update; fails harmlessly while that
                // program still runs.
                let _ = fs::remove_file(self.old_binary());
                if self.binary.exists() {
                    fs::rename(&self.binary, self.old_binary())
                        .map_err(|e| error("replace", &self.binary, e))?;
                }
                fs::rename(&partial, &self.binary).map_err(|e| error("write", &self.binary, e))?;
            }
            create_parent(&self.shortcut)?;
            let mut script = String::from(
                "$ErrorActionPreference = 'Stop'\n\
                 $s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:XPAINT_LNK)\n\
                 $s.TargetPath = $env:XPAINT_EXE\n\
                 $s.WorkingDirectory = [IO.Path]::GetDirectoryName($env:XPAINT_EXE)\n\
                 $s.IconLocation = \"$env:XPAINT_EXE,0\"\n\
                 $s.Description = 'Image editor'\n\
                 $s.Save()\n",
            );
            if self.register {
                script += &format!(
                    "$k = '{OPEN_WITH_KEY}'\n\
                     New-Item -Path \"$k\\shell\\open\\command\" -Force | Out-Null\n\
                     Set-Item -Path \"$k\\shell\\open\\command\" -Value \"`\"$env:XPAINT_EXE`\" `\"%1`\"\"\n\
                     Set-ItemProperty -Path $k -Name FriendlyAppName -Value 'xpaint'\n\
                     New-Item -Path \"$k\\SupportedTypes\" -Force | Out-Null\n"
                );
                for ext in EXTENSIONS {
                    script += &format!(
                        "Set-ItemProperty -Path \"$k\\SupportedTypes\" -Name '{ext}' -Value ''\n\
                         New-Item -Path 'HKCU:\\Software\\Classes\\{ext}\\OpenWithList\\xpaint.exe' -Force | Out-Null\n"
                    );
                }
            }
            self.powershell(&script)
        }

        /// Removes what `install` added. Missing files are fine.
        pub fn uninstall(&self) -> Result<(), String> {
            if self.register {
                let mut script = format!(
                    "Remove-Item -Path '{OPEN_WITH_KEY}' -Recurse -Force -ErrorAction SilentlyContinue\n"
                );
                for ext in EXTENSIONS {
                    script += &format!(
                        "Remove-Item -Path 'HKCU:\\Software\\Classes\\{ext}\\OpenWithList\\xpaint.exe' \
                         -Recurse -Force -ErrorAction SilentlyContinue\n"
                    );
                }
                self.powershell(&script)?;
            }
            for path in [self.shortcut.clone(), self.old_binary()] {
                let _ = fs::remove_file(path);
            }
            match fs::remove_file(&self.binary) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                // Running: move it aside, to the temp folder if it is on the same
                // drive, so the program folder can go.
                Err(_) => {
                    let temp = std::env::temp_dir()
                        .join(format!("xpaint-uninstalled-{}.exe", std::process::id()));
                    if fs::rename(&self.binary, &temp).is_err() {
                        fs::rename(&self.binary, self.old_binary())
                            .map_err(|e| error("remove", &self.binary, e))?;
                    }
                }
            }
            if let Some(dir) = self.binary.parent() {
                // Only succeeds when empty
                let _ = fs::remove_dir(dir);
            }
            Ok(())
        }

        /// Runs `script` with the paths in environment variables, which avoids
        /// quoting them.
        fn powershell(&self, script: &str) -> Result<(), String> {
            let output = hidden("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                ])
                .args(["-Command", script])
                .env("XPAINT_EXE", &self.binary)
                .env("XPAINT_LNK", &self.shortcut)
                .output()
                .map_err(|e| format!("Could not start PowerShell: {e}"))?;
            if output.status.success() {
                return Ok(());
            }
            let message = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "Could not add xpaint to the Start menu: {}",
                message.trim()
            ))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn install_and_uninstall() {
            let dir = std::env::temp_dir().join(format!("xpaint-install-{}", std::process::id()));
            let location = Location::new(&dir.join("app"), &dir.join("menu"), false);
            let source = dir.join("build/xpaint.exe");
            super::super::write(&source, b"binary").unwrap();
            assert!(!location.is_installed());

            location.install(&source).unwrap();
            assert!(location.is_installed());
            assert_eq!(fs::read(&location.binary).unwrap(), b"binary");

            // Replacing it keeps working while the old binary exists.
            location.install(&source).unwrap();
            location.install(&location.binary).unwrap();
            assert_eq!(fs::read(&location.binary).unwrap(), b"binary");

            location.uninstall().unwrap();
            assert!(!location.is_installed());
            assert!(!dir.join("app").exists());
            location.uninstall().unwrap();
            fs::remove_dir_all(&dir).unwrap();
        }
    }
}

/// A command that opens no console window when started from the GUI.
#[cfg(windows)]
fn hidden(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn create_parent(path: &Path) -> Result<(), String> {
    let dir = path.parent().expect("install paths have a parent");
    fs::create_dir_all(dir).map_err(|e| error("create", dir, e))
}

#[cfg_attr(not(any(unix, test)), allow(dead_code))]
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    create_parent(path)?;
    fs::write(path, bytes).map_err(|e| error("write", path, e))
}

fn error(action: &str, path: &Path, e: std::io::Error) -> String {
    format!("Could not {action} {}: {e}", path.display())
}

#[cfg(all(test, unix))]
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

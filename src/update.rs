//! Update of the installed copy from GitHub releases, as in LeoView. Checking is
//! off until the user turns on **Check for updates once a day** in the Settings
//! menu; then the installed copy looks up the latest release tag at start, at
//! most once per `CHECK_INTERVAL`. The **Update** button downloads that release,
//! unpacks the binary to a temporary folder, checks its `--version` and runs it
//! with `--install`. The new version is used from the next start; the app offers
//! to restart right away.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const REPO: &str = "https://github.com/matz0r/xpaint";
const TEMP_PREFIX: &str = "xpaint-update-";
const BINARY: &str = if cfg!(windows) {
    "xpaint.exe"
} else {
    "xpaint"
};
/// Must match the archive names in `.github/workflows/release.yml`.
const ASSET_SUFFIX: &str = if cfg!(windows) {
    "x86_64-windows.zip"
} else {
    "x86_64-linux.tar.gz"
};
/// Release archives are a few MB; this only guards against a broken response.
const MAX_DOWNLOAD: u64 = 64 * 1024 * 1024;
/// Minimum time between two automatic checks
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub enum Msg {
    /// GitHub answered: the latest release, if it is newer than this program
    Checked(Option<semver::Version>),
    /// The update was installed
    Installed(semver::Version),
    Failed(String),
}

/// Saved update state, in `$XDG_CONFIG_HOME/xpaint/update` (Windows:
/// `%APPDATA%\xpaint\update`). Only the installed
/// copy reads and writes it.
#[derive(Default, PartialEq, Debug)]
pub struct Settings {
    /// "Check for updates" toggle, off by default
    pub check: bool,
    /// Unix seconds of the last successful check
    pub last_check: u64,
    /// Newest release found by the last check, if newer than that program
    pub latest: Option<semver::Version>,
}

impl Settings {
    #[cfg(windows)]
    fn path() -> Option<PathBuf> {
        let config = std::env::var_os("APPDATA").map(PathBuf::from)?;
        Some(config.join("xpaint").join("update"))
    }

    #[cfg(not(windows))]
    fn path() -> Option<PathBuf> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(config.join("xpaint").join("update"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| Self::parse(&s))
            .unwrap_or_default()
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
            match key.trim() {
                "check" => settings.check = value.trim() == "1",
                "last_check" => settings.last_check = value.trim().parse().unwrap_or(0),
                "latest" => settings.latest = semver::Version::parse(value.trim()).ok(),
                _ => {}
            }
        }
        settings
    }

    fn format(&self) -> String {
        format!(
            "check={}\nlast_check={}\nlatest={}\n",
            self.check as u8,
            self.last_check,
            self.latest
                .as_ref()
                .map_or(String::new(), ToString::to_string)
        )
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        let result = std::fs::create_dir_all(path.parent().expect("path has a parent"))
            .and_then(|()| std::fs::write(&path, self.format()));
        if let Err(e) = result {
            eprintln!("xpaint: can't save {}: {e}", path.display());
        }
    }

    /// The release the Update button offers
    pub fn offered(&self) -> Option<&semver::Version> {
        self.latest.as_ref().filter(|v| self.check && is_newer(v))
    }
}

/// Releases are only built for x86_64, and only the installed copy updates
/// itself: a download is updated by downloading again.
pub fn eligible() -> bool {
    cfg!(target_arch = "x86_64")
        && crate::install::Location::user().is_ok_and(|l| l.is_running_installed())
}

/// Seconds since the Unix epoch, for the stored time of the last check
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Whether the last successful check at `last` (Unix seconds) is old enough.
/// A last check in the future (clock changed) also counts as due.
pub fn due(last: u64, now: u64) -> bool {
    now < last || now - last >= CHECK_INTERVAL.as_secs()
}

/// Looks for a newer release in the background. Reports nothing when GitHub
/// can't be reached; offline use is normal and the next start tries again.
pub fn check(ctx: &egui::Context) -> Receiver<Msg> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        remove_leftovers();
        match latest() {
            Ok(latest) => {
                let _ = tx.send(Msg::Checked(is_newer(&latest).then_some(latest)));
                ctx.request_repaint();
            }
            Err(e) => eprintln!("xpaint: update check failed: {e}"),
        }
    });
    rx
}

/// Downloads and installs `version` in the background.
pub fn install(ctx: &egui::Context, version: semver::Version) -> Receiver<Msg> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(match download_and_install(&version) {
            Ok(()) => Msg::Installed(version),
            Err(e) => Msg::Failed(e),
        });
        ctx.request_repaint();
    });
    rx
}

/// `xpaint --update`: installs the latest release if it is newer than this
/// program. Returns the installed version, or None when up to date.
pub fn update_now() -> Result<Option<semver::Version>, String> {
    let version = latest()?;
    if !is_newer(&version) {
        return Ok(None);
    }
    download_and_install(&version)?;
    Ok(Some(version))
}

/// Starts the installed copy with `files` open, after an update.
pub fn restart(files: &[PathBuf]) -> Result<(), String> {
    let binary = crate::install::Location::user()?.binary;
    let mut child = Command::new(&binary)
        .args(files)
        .spawn()
        .map_err(|e| format!("Could not start {}: {e}", binary.display()))?;
    std::thread::sleep(Duration::from_millis(500));
    match child.try_wait() {
        Ok(Some(status)) => Err(format!("The updated xpaint exited right away ({status})")),
        _ => Ok(()),
    }
}

/// rustls with bundled roots: no OpenSSL needed at runtime.
fn agent(timeout: Duration, follow_redirects: bool) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(concat!("xpaint/", env!("CARGO_PKG_VERSION")));
    let config = if follow_redirects {
        config
    } else {
        config.max_redirects(0).max_redirects_will_error(false)
    };
    config.build().into()
}

/// GitHub redirects /releases/latest to /releases/tag/<tag>. Reading the
/// redirect avoids the rate-limited API and any JSON parsing.
fn latest() -> Result<semver::Version, String> {
    let response = agent(Duration::from_secs(10), false)
        .get(format!("{REPO}/releases/latest"))
        .call()
        .map_err(|e| e.to_string())?;
    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or("GitHub did not name the latest release")?;
    parse_tag_url(location).ok_or_else(|| format!("unexpected release address {location}"))
}

fn parse_tag_url(url: &str) -> Option<semver::Version> {
    let tag = url.rsplit_once("/releases/tag/")?.1;
    semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}

pub fn is_newer(version: &semver::Version) -> bool {
    *version > semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
}

fn asset_url(version: &semver::Version) -> String {
    format!("{REPO}/releases/download/v{version}/xpaint-v{version}-{ASSET_SUFFIX}")
}

fn download(version: &semver::Version) -> Result<Vec<u8>, String> {
    let mut response = agent(Duration::from_secs(120), true)
        .get(asset_url(version))
        .call()
        .map_err(|e| format!("Can't download version {version}: {e}"))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD)
        .read_to_vec()
        .map_err(|e| format!("Download interrupted: {e}"))
}

fn download_and_install(version: &semver::Version) -> Result<(), String> {
    let archive = download(version)?;
    // A fresh folder per attempt; create_dir fails instead of reusing a
    // folder someone else prepared.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let dir = std::env::temp_dir().join(format!("{TEMP_PREFIX}{}-{nonce}", std::process::id()));
    std::fs::create_dir(&dir).map_err(|e| format!("Can't create {}: {e}", dir.display()))?;
    let result = unpack_and_install(&archive, &dir, version);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn unpack_and_install(archive: &[u8], dir: &Path, version: &semver::Version) -> Result<(), String> {
    let exe = dir.join(BINARY);
    unpack(archive, &exe)?;

    // Make sure the download is the announced version before it replaces
    // anything.
    let output = hidden(&exe)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Can't start the downloaded program: {e}"))?;
    let reported = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || reported.trim() != version.to_string() {
        return Err(format!(
            "The download reports version {:?} instead of {version}",
            reported.trim()
        ));
    }

    let output = hidden(&exe)
        .arg("--install")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Can't start the installer: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let message = String::from_utf8_lossy(&output.stderr);
    Err(message
        .lines()
        .last()
        .unwrap_or("Installation failed")
        .to_owned())
}

/// A command that opens no console window when started from the GUI.
fn hidden(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(windows)]
fn unpack(archive: &[u8], exe: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))
        .map_err(|e| format!("Broken archive: {e}"))?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("Broken archive: {e}"))?;
        let name = entry
            .name()
            .map_err(|e| format!("Broken archive: {e}"))?
            .replace('\\', "/");
        if entry.is_file() && name.rsplit('/').next() == Some(BINARY) {
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(exe)
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("Broken archive: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("{BINARY} is missing from the download"))
}

#[cfg(not(windows))]
fn unpack(archive: &[u8], exe: &Path) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().map_err(|e| format!("Broken archive: {e}"))? {
        let mut entry = entry.map_err(|e| format!("Broken archive: {e}"))?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if entry.header().entry_type().is_file() && path.file_name() == Some(BINARY.as_ref()) {
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o755)
                .open(exe)
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("Broken archive: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("{BINARY} is missing from the download"))
}

/// Folders of earlier updates that could not be cleaned up.
fn remove_leftovers() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    let own = format!("{TEMP_PREFIX}{}-", std::process::id());
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(TEMP_PREFIX) && !name.starts_with(&own) {
            // Fails harmlessly for other users' folders
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_redirect() {
        assert_eq!(
            parse_tag_url("https://github.com/matz0r/xpaint/releases/tag/v0.3.0"),
            Some(semver::Version::new(0, 3, 0))
        );
        assert_eq!(
            parse_tag_url("https://github.com/matz0r/xpaint/releases"),
            None
        );
        assert_eq!(
            parse_tag_url("https://github.com/x/releases/tag/nightly"),
            None
        );
    }

    #[test]
    fn names_release_asset() {
        let url = asset_url(&semver::Version::new(1, 2, 3));
        let expected = if cfg!(windows) {
            "https://github.com/matz0r/xpaint/releases/download/v1.2.3/xpaint-v1.2.3-x86_64-windows.zip"
        } else {
            "https://github.com/matz0r/xpaint/releases/download/v1.2.3/xpaint-v1.2.3-x86_64-linux.tar.gz"
        };
        assert_eq!(url, expected);
    }

    #[test]
    fn checks_once_per_interval() {
        let day = CHECK_INTERVAL.as_secs();
        assert!(due(0, 1_000_000));
        assert!(!due(1_000_000, 1_000_000));
        assert!(!due(1_000_000, 1_000_000 + day - 1));
        assert!(due(1_000_000, 1_000_000 + day));
        assert!(due(1_000_000 + 60, 1_000_000));
    }

    #[test]
    fn only_newer_versions_are_updates() {
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        assert!(!is_newer(&current));
        assert!(is_newer(&semver::Version::new(current.major + 1, 0, 0)));
    }

    #[test]
    fn settings_round_trip() {
        let settings = Settings {
            check: true,
            last_check: 1_700_000_000,
            latest: Some(semver::Version::new(9, 1, 0)),
        };
        assert_eq!(Settings::parse(&settings.format()), settings);
        assert_eq!(Settings::parse(""), Settings::default());
        assert_eq!(Settings::parse("check=0\nlatest=\n"), Settings::default());
    }

    /// Talks to GitHub: `cargo test -- --ignored downloads_latest_release`.
    /// Installs nothing. Needs a release that knows `--version` (after 0.3.0).
    #[test]
    #[ignore]
    fn downloads_latest_release() {
        let version = latest().unwrap();
        let archive = download(&version).unwrap();
        let dir = std::env::temp_dir().join(format!("xpaint-download-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join(BINARY);
        unpack(&archive, &exe).unwrap();
        let output = Command::new(&exe).arg("--version").output().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            version.to_string()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

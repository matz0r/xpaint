//! Opening the current image in LeoView, the companion image viewer, when it is
//! installed for this user (its installer puts it in `~/.local/bin/leoview`).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The installed LeoView binary, if there is one.
pub fn installed() -> Option<PathBuf> {
    // LeoView is Linux only.
    if !cfg!(unix) {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    binary_in(Path::new(&home))
}

fn binary_in(home: &Path) -> Option<PathBuf> {
    let binary = home.join(".local/bin/leoview");
    binary.is_file().then_some(binary)
}

/// Starts LeoView showing `image` (and its folder) in a window of its own.
pub fn open(binary: &Path, image: &Path) -> Result<(), String> {
    let mut child = Command::new(binary)
        .arg(image)
        .spawn()
        .map_err(|e| format!("Could not start LeoView: {e}"))?;
    // Reap it when it exits, so it doesn't linger as a zombie while xpaint runs.
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_leoview_in_the_user_bin_folder() {
        let home = std::env::temp_dir().join(format!("xpaint-leoview-{}", std::process::id()));
        assert_eq!(binary_in(&home), None);
        let bin = home.join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("leoview"), b"").unwrap();
        assert_eq!(binary_in(&home), Some(bin.join("leoview")));
        std::fs::remove_dir_all(home).unwrap();
    }
}

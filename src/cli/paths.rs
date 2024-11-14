use std::{
    env::current_dir,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{anyhow, Result as Res};

#[allow(
    clippy::missing_panics_doc,
    reason = "This is copied straight from Zed's source, so it's not my problem to document it"
)]
/// Returns the path to the configuration directory used by Zed.
pub fn config_dir() -> &'static PathBuf {
    // tbh I could probably depend on Zed's source
    // directly instead of copy pasting, but I probably shouldn't
    static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();
    CONFIG_DIR.get_or_init(|| {
        if cfg!(target_os = "windows") {
            return dirs::config_dir()
                .expect("failed to determine RoamingAppData directory")
                .join("Zed");
        }

        if cfg!(target_os = "linux") {
            return if let Ok(flatpak_xdg_config) = std::env::var("FLATPAK_XDG_CONFIG_HOME") {
                flatpak_xdg_config.into()
            } else {
                dirs::config_dir().expect("failed to determine XDG_CONFIG_HOME directory")
            }
            .join("zed");
        }

        dirs::home_dir()
            .expect("failed to determine home directory")
            .join(".config")
            .join("zed")
    })
}

pub fn default_output_location(infile: &Path, ext: &str) -> Res<PathBuf> {
    let current_dir = current_dir()?;
    let rel = if infile.is_relative() {
        infile.to_path_buf()
    } else {
        pathdiff::diff_paths(infile, &current_dir).expect("Failed to diff infile and with the cwd. This should not be able to happen as both are absolute.")
    };

    let dir = current_dir.join("generated");
    Ok(dir.join(rel.with_extension(ext)))
}

pub fn default_install_location(outfile: &Path) -> Res<PathBuf> {
    let base_name = outfile
        .file_name()
        .ok_or_else(|| anyhow!("Output file does not have a file name"))?;
    Ok(config_dir().join("themes").join(base_name))
}

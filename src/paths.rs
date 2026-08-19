use std::env;
use std::path::{Path, PathBuf};

use directories::BaseDirs;

use crate::{APP_NAME, Error, Result};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_file: PathBuf,
    pub bin_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let base = BaseDirs::new().ok_or_else(|| {
            Error::Message("could not determine the current user's home directories".into())
        })?;
        let config_dir = env::var_os("TRANSFIGURE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| base.config_dir().join(APP_NAME));
        let bin_dir = env::var_os("TRANSFIGURE_BIN_DIR")
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(|| {
                let executable = env::current_exe().map_err(|source| {
                    Error::Message(format!("could not locate the executable: {source}"))
                })?;
                executable.parent().map(Path::to_owned).ok_or_else(|| {
                    Error::Message("the executable path has no parent directory".into())
                })
            })?;
        Ok(Self {
            config_file: config_dir.join("config.json"),
            bin_dir,
        })
    }

    pub fn ensure_directories(&self) -> Result<()> {
        for path in [self.config_file.parent(), Some(self.bin_dir.as_path())]
            .into_iter()
            .flatten()
        {
            std::fs::create_dir_all(path).map_err(|source| Error::Write {
                path: path.to_owned(),
                source,
            })?;
        }
        Ok(())
    }

    pub fn bin_is_on_path(&self) -> bool {
        env::var_os("PATH").is_some_and(|path| {
            env::split_paths(&path).any(|entry| paths_equal(&entry, &self.bin_dir))
        })
    }

    pub fn path_guidance(&self) -> String {
        #[cfg(windows)]
        {
            format!(
                "Add '{}' to your user PATH, then open a new terminal.",
                self.bin_dir.display()
            )
        }
        #[cfg(not(windows))]
        {
            format!(
                "Add this line to your shell profile, then open a new terminal:\nexport PATH=\"{}:$PATH\"",
                self.bin_dir.display()
            )
        }
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

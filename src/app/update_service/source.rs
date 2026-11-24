use std::{collections::HashMap, fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateSource {
    #[default]
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "crates.io")]
    CratesIo,
}

#[derive(Deserialize)]
struct CargoInstalls {
    installs: HashMap<String, CargoInstall>,
}

#[derive(Deserialize)]
struct CargoInstall {
    bins: Vec<String>,
}

impl UpdateSource {
    pub(super) fn detect() -> Self {
        std::env::current_exe()
            .ok()
            .map(|path| Self::for_executable(&path, env!("CARGO_PKG_VERSION")))
            .unwrap_or_default()
    }

    fn for_executable(executable: &Path, current_version: &str) -> Self {
        let Some(executable) = fs::canonicalize(executable).ok() else {
            return Self::GitHub;
        };
        let Some(bin) = executable.parent().filter(|path| path.ends_with("bin")) else {
            return Self::GitHub;
        };
        let Some(root) = bin.parent() else {
            return Self::GitHub;
        };
        let metadata = fs::read(root.join(".crates2.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CargoInstalls>(&bytes).ok());
        let tracked = metadata.is_some_and(|metadata| {
            metadata.installs.iter().any(|(package, install)| {
                let mut identity = package.split_whitespace();
                identity.next() == Some(env!("CARGO_PKG_NAME"))
                    && identity.next() == Some(current_version)
                    && install
                        .bins
                        .iter()
                        .any(|name| executable.file_name() == Some(name.as_ref()))
            })
        });
        if tracked {
            Self::CratesIo
        } else {
            Self::GitHub
        }
    }

    pub(super) fn endpoint(self) -> &'static str {
        match self {
            Self::GitHub => "https://api.github.com/repos/class-undefined/thufs/releases/latest",
            Self::CratesIo => "https://crates.io/api/v1/crates/thufs",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub Releases",
            Self::CratesIo => "crates.io (Cargo)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installation(root: &Path, package: &str, bins: &[&str]) -> std::path::PathBuf {
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let executable = bin.join(format!("thufs{}", std::env::consts::EXE_SUFFIX));
        fs::write(&executable, "fixture").unwrap();
        let metadata = serde_json::json!({"installs": {(package): {"bins": bins}}});
        fs::write(root.join(".crates2.json"), metadata.to_string()).unwrap();
        executable
    }

    #[test]
    fn detects_tracked_cargo_installations_under_any_install_root() {
        let temp = tempfile::tempdir().unwrap();
        let binary_name = format!("thufs{}", std::env::consts::EXE_SUFFIX);
        for origin in [
            "registry+https://github.com/rust-lang/crates.io-index",
            "path+file:///source",
            "git+https://example.com/thufs",
        ] {
            let executable = installation(
                temp.path(),
                &format!("thufs 0.2.0 ({origin})"),
                &[&binary_name],
            );
            assert_eq!(
                UpdateSource::for_executable(&executable, "0.2.0"),
                UpdateSource::CratesIo
            );
        }
    }

    #[test]
    fn unrelated_stale_missing_and_invalid_metadata_do_not_misidentify_manual_installs() {
        let temp = tempfile::tempdir().unwrap();
        let name = format!("thufs{}", std::env::consts::EXE_SUFFIX);
        for (package, bins) in [
            (
                "another 0.2.0 (registry+https://example.com/index)",
                vec![name.as_str()],
            ),
            (
                "thufs 0.1.9 (registry+https://example.com/index)",
                vec![name.as_str()],
            ),
            (
                "thufs 0.2.0 (registry+https://example.com/index)",
                vec!["another"],
            ),
        ] {
            let executable = installation(temp.path(), package, &bins);
            assert_eq!(
                UpdateSource::for_executable(&executable, "0.2.0"),
                UpdateSource::GitHub
            );
        }
        let executable = temp.path().join("bin").join(name);
        fs::write(temp.path().join(".crates2.json"), "invalid JSON").unwrap();
        assert_eq!(
            UpdateSource::for_executable(&executable, "0.2.0"),
            UpdateSource::GitHub
        );
        fs::remove_file(temp.path().join(".crates2.json")).unwrap();
        assert_eq!(
            UpdateSource::for_executable(&executable, "0.2.0"),
            UpdateSource::GitHub
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_to_cargo_installed_binaries_keep_their_update_source() {
        let temp = tempfile::tempdir().unwrap();
        let executable = installation(
            temp.path(),
            "thufs 0.2.0 (registry+https://example.com/index)",
            &["thufs"],
        );
        let link = temp.path().join("linked-thufs");
        std::os::unix::fs::symlink(executable, &link).unwrap();
        assert_eq!(
            UpdateSource::for_executable(&link, "0.2.0"),
            UpdateSource::CratesIo
        );
    }
}

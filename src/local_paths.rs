use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) fn default_download_directory() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()))
        .context("cannot determine the home directory; specify a local download directory")?;
    ensure_download_directory(Path::new(&home))
}

fn ensure_download_directory(home: &Path) -> Result<PathBuf> {
    let directory = home.join("Downloads");
    std::fs::create_dir_all(&directory).with_context(|| {
        format!(
            "failed to create default download directory {}",
            directory.display()
        )
    })?;
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::ensure_download_directory;

    #[test]
    fn creates_missing_downloads_directory_under_home() {
        let home = tempdir().expect("home directory");
        let downloads = ensure_download_directory(home.path()).expect("download directory");

        assert_eq!(downloads, home.path().join("Downloads"));
        assert!(downloads.is_dir());
    }

    #[test]
    fn reuses_existing_downloads_directory_without_changing_contents() {
        let home = tempdir().expect("home directory");
        let downloads = home.path().join("Downloads");
        std::fs::create_dir(&downloads).expect("create downloads");
        let existing_file = downloads.join("existing.txt");
        std::fs::write(&existing_file, "content").expect("write existing file");

        assert_eq!(
            ensure_download_directory(home.path()).expect("download directory"),
            downloads
        );
        assert_eq!(std::fs::read_to_string(existing_file).unwrap(), "content");
    }

    #[test]
    fn rejects_a_file_in_place_of_downloads_directory() {
        let home = tempdir().expect("home directory");
        let downloads = home.path().join("Downloads");
        std::fs::write(&downloads, "content").expect("write file");

        let error = ensure_download_directory(home.path()).expect_err("not a directory");

        assert!(error.to_string().contains("default download directory"));
        assert_eq!(std::fs::read_to_string(downloads).unwrap(), "content");
    }
}

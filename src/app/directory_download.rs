use std::{
    collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    seafile::{DirectoryEntry, DownloadAuth, EntryKind, SeafileClient},
    transfer::{DownloadMode, ProgressMode, create_progress_reporter},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum DirectorySource {
    Repository { repo_id: String, path: String },
    Shared { token: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectoryTarget {
    pub source: DirectorySource,
    pub entries: Vec<DirectoryEntry>,
}

impl DirectoryTarget {
    fn root(&self) -> &str {
        match &self.source {
            DirectorySource::Repository { path, .. } => path,
            DirectorySource::Shared { .. } => "/",
        }
    }
}

pub(super) fn validate_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    if name.is_empty()
        || name.contains(['/', '\\'])
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("remote entry has an unsafe file name: {name:?}");
    }
    Ok(())
}

fn relative_path(path: &str, root: &str) -> Result<PathBuf> {
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let relative = path
        .strip_prefix(&prefix)
        .with_context(|| format!("remote path `{path}` is outside directory `{root}`"))?;
    if relative.is_empty() {
        bail!("remote directory contains an empty entry path");
    }
    let mut result = PathBuf::new();
    for name in relative.split('/') {
        validate_name(name)?;
        result.push(name);
    }
    Ok(result)
}

fn manifest(client: &SeafileClient, target: &DirectoryTarget) -> Result<Vec<DirectoryEntry>> {
    let mut pending = VecDeque::from([(target.root().to_string(), target.entries.clone())]);
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    while let Some((parent, entries)) = pending.pop_front() {
        for entry in entries {
            validate_name(&entry.name)?;
            let expected = format!("{}/{}", parent.trim_end_matches('/'), entry.name);
            if entry.path != expected || !seen.insert(entry.path.clone()) {
                bail!(
                    "invalid or duplicate remote directory entry: {}",
                    entry.path
                );
            }
            relative_path(&entry.path, target.root())?;
            if entry.kind == EntryKind::Dir {
                let children = match &target.source {
                    DirectorySource::Repository { repo_id, .. } => {
                        client.list_directory_entries_with_time(repo_id, &entry.path, false)?
                    }
                    DirectorySource::Shared { token } => {
                        client.list_shared_directory(token, &entry.path)?.entries
                    }
                };
                pending.push_back((entry.path.clone(), children));
            }
            result.push(entry);
        }
    }
    Ok(result)
}

fn checked_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => bail!(
            "local directory `{}` is a file or symbolic link",
            path.display()
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => fs::create_dir(path)
            .with_context(|| format!("failed to create directory {}", path.display())),
        Err(error) => Err(error).with_context(|| format!("cannot inspect {}", path.display())),
    }
}

fn ensure_tree(root: &Path, relative: &Path) -> Result<()> {
    checked_directory(root)?;
    let mut path = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            bail!("invalid local relative path");
        }
        path.push(component);
        checked_directory(&path)?;
    }
    Ok(())
}

fn checked_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => bail!(
            "local file `{}` is a directory or symbolic link",
            path.display()
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("cannot inspect {}", path.display())),
    }
}

fn preflight_destination(destination: &Path, paths: &[(PathBuf, EntryKind)]) -> Result<()> {
    for (relative, kind) in paths {
        let mut path = destination.to_path_buf();
        for component in relative.components() {
            path.push(component);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => bail!(
                    "local destination contains a symbolic link: {}",
                    path.display()
                ),
                Ok(metadata) => {
                    let directory = path != destination.join(relative) || *kind == EntryKind::Dir;
                    if metadata.is_dir() != directory {
                        bail!(
                            "local destination has a file/directory conflict: {}",
                            path.display()
                        );
                    }
                }
                Err(error) if error.kind() == ErrorKind::NotFound => break,
                Err(error) => return Err(error).context("cannot inspect download destination"),
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FileIdentity(u64, u64);

#[cfg(not(unix))]
type FileIdentity = PathBuf;

fn file_identity(path: &Path) -> Result<FileIdentity> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path)?;
        Ok(FileIdentity(metadata.dev(), metadata.ino()))
    }
    #[cfg(not(unix))]
    {
        Ok(fs::canonicalize(path)?)
    }
}

fn claim_path(
    path: &Path,
    relative: &Path,
    claimed: &mut HashMap<FileIdentity, PathBuf>,
) -> Result<()> {
    let identity = file_identity(path)?;
    if let Some(previous) = claimed.get(&identity) {
        if previous != relative {
            bail!(
                "remote names collide on the local filesystem: `{}` and `{}`",
                previous.display(),
                relative.display()
            );
        }
    }
    claimed.insert(identity, relative.to_path_buf());
    Ok(())
}

fn prune_staging(directory: &Path, expected: &HashSet<FileIdentity>) -> Result<()> {
    for item in fs::read_dir(directory)? {
        let item = item?;
        let path = item.path();
        let kind = item.file_type()?;
        if kind.is_symlink() {
            bail!(
                "temporary download contains a symbolic link: {}",
                path.display()
            );
        }
        if !expected.contains(&file_identity(&path)?) {
            if kind.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        } else if kind.is_dir() {
            prune_staging(&path, expected)?;
        }
    }
    Ok(())
}

fn staging_path(destination: &Path, source: &DirectorySource) -> Result<PathBuf> {
    let mut hash = DefaultHasher::new();
    source.hash(&mut hash);
    destination.file_name().hash(&mut hash);
    let parent = destination
        .parent()
        .context("directory destination has no parent")?;
    Ok(parent.join(format!(".thufs-dir-{:016x}.thufs-part", hash.finish())))
}

pub(super) fn download(
    client: &SeafileClient,
    target: &DirectoryTarget,
    destination: &Path,
    mode: DownloadMode,
    workers: usize,
    progress_mode: ProgressMode,
    overwrite: bool,
) -> Result<u64> {
    let entries = manifest(client, target)?;
    let paths: Vec<_> = entries
        .iter()
        .map(|entry| {
            relative_path(&entry.path, target.root()).map(|path| (path, entry.kind.clone()))
        })
        .collect::<Result<_>>()?;
    if destination.exists() {
        if !overwrite {
            bail!("local directory already exists: {}", destination.display());
        }
        let metadata = fs::symlink_metadata(destination)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            bail!("directory destination is a file or symbolic link");
        }
        preflight_destination(destination, &paths)?;
    }
    let staging = staging_path(destination, &target.source)?;
    checked_directory(&staging)?;
    let tree = staging.join("tree");
    let parts = staging.join("parts");
    checked_directory(&tree)?;
    checked_directory(&parts)?;
    let total = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::File)
        .try_fold(0u64, |sum, entry| sum.checked_add(entry.size?));
    let progress = create_progress_reporter(
        progress_mode,
        "download",
        destination.display().to_string(),
        total,
    )?;
    let mut written = 0u64;
    let mut claimed = HashMap::new();
    for (entry, (relative, kind)) in entries.iter().zip(&paths) {
        if *kind == EntryKind::Dir {
            ensure_tree(&tree, relative)?;
            claim_path(&tree.join(relative), relative, &mut claimed)?;
            continue;
        }
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        ensure_tree(&tree, parent)?;
        ensure_tree(&parts, parent)?;
        let final_path = tree.join(relative);
        let part_path = parts.join(relative);
        checked_file(&final_path)?;
        checked_file(&part_path)?;
        if final_path.exists() {
            claim_path(&final_path, relative, &mut claimed)?;
        }
        let completed = fs::metadata(&final_path)
            .ok()
            .is_some_and(|metadata| Some(metadata.len()) == entry.size);
        let bytes = if completed {
            entry.size.context("missing completed file size")?
        } else {
            if final_path.exists() {
                fs::remove_file(&final_path)?;
            }
            let (link, auth) = match &target.source {
                DirectorySource::Repository { repo_id, .. } => (
                    client.get_download_link(repo_id, &entry.path)?,
                    DownloadAuth::Required,
                ),
                DirectorySource::Shared { token } => (
                    client.shared_directory_file_link(token, &entry.path)?,
                    DownloadAuth::Optional,
                ),
            };
            progress.set_message(format!("download {}", relative.display()));
            let bytes = client.download_file_reported(
                &link,
                &part_path,
                mode,
                workers,
                ProgressMode::None,
                auth,
                Some(progress.with_offset(written)),
            )?;
            if entry.size.is_some_and(|size| size != bytes) {
                bail!("download size differs from remote file: {}", entry.path);
            }
            fs::rename(&part_path, &final_path)?;
            claim_path(&final_path, relative, &mut claimed)?;
            bytes
        };
        written = written
            .checked_add(bytes)
            .context("directory size exceeds supported range")?;
        progress.set_position(written)?;
    }
    let expected = claimed.keys().cloned().collect();
    prune_staging(&tree, &expected)?;
    if overwrite && destination.exists() {
        preflight_destination(destination, &paths)?;
        for (relative, kind) in &paths {
            if *kind == EntryKind::Dir {
                ensure_tree(destination, relative)?;
                continue;
            }
            ensure_tree(
                destination,
                relative.parent().unwrap_or_else(|| Path::new("")),
            )?;
            let path = destination.join(relative);
            checked_file(&path)?;
            if path.exists() {
                fs::remove_file(&path)?;
            }
            fs::rename(tree.join(relative), path)?;
        }
    } else {
        if destination.exists() {
            bail!(
                "local destination appeared during download: {}",
                destination.display()
            );
        }
        fs::rename(&tree, destination)
            .with_context(|| format!("failed to save directory {}", destination.display()))?;
    }
    fs::remove_dir_all(&staging).context("failed to clean completed directory download")?;
    progress.finish_with_message(format!("download {} complete", destination.display()))?;
    Ok(written)
}

#[cfg(test)]
mod tests;

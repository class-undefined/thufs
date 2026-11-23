use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use glob::{MatchOptions, Pattern, glob_with};

use super::{
    backend::local_entries,
    model::{LocalEntry, expand_path, fuzzy_score},
};

pub struct PathMatches {
    pub directory: Option<PathBuf>,
    pub root: PathBuf,
    pub entries: Vec<LocalEntry>,
    pub pattern: bool,
    pub truncated: bool,
}

pub fn resolve(input: &str, cwd: &Path) -> PathBuf {
    let input = if input.len() >= 2
        && ((input.starts_with('"') && input.ends_with('"'))
            || (input.starts_with('\'') && input.ends_with('\'')))
    {
        &input[1..input.len() - 1]
    } else {
        input
    };
    let expanded = expand_path(input);
    if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    }
}

pub fn matches(input: &str, cwd: &Path, hidden: bool) -> Result<PathMatches> {
    matches_filtered(input, cwd, hidden, false)
}

pub fn directories(input: &str, cwd: &Path, hidden: bool) -> Result<PathMatches> {
    let target = resolve(input, cwd);
    if target.is_dir() {
        let input = format!(
            "{}{}",
            target
                .to_str()
                .context("The path is not valid UTF-8.")?
                .trim_end_matches(std::path::MAIN_SEPARATOR),
            std::path::MAIN_SEPARATOR
        );
        let mut result = matches_filtered(&input, cwd, hidden, true)?;
        result.directory = Some(target);
        Ok(result)
    } else if target.is_file() {
        bail!("Choose an existing local directory.")
    } else {
        matches_filtered(input, cwd, hidden, true)
    }
}

/// Browse the nearest existing directory instead of filtering by the file name.
pub fn browse(
    input: &str,
    cwd: &Path,
    hidden: bool,
    directories_only: bool,
) -> Result<PathMatches> {
    let target = resolve(input, cwd);
    let mut directory = target.as_path();
    while !directory.is_dir() {
        directory = directory
            .parent()
            .context("Cannot find a directory to browse.")?;
    }
    if directories_only {
        directories(
            directory.to_str().context("The path is not valid UTF-8.")?,
            cwd,
            hidden,
        )
    } else {
        let input = format!(
            "{}{}",
            directory
                .to_str()
                .context("The path is not valid UTF-8.")?
                .trim_end_matches(std::path::MAIN_SEPARATOR),
            std::path::MAIN_SEPARATOR
        );
        matches(&input, cwd, hidden)
    }
}

fn matches_filtered(
    input: &str,
    cwd: &Path,
    hidden: bool,
    directories_only: bool,
) -> Result<PathMatches> {
    let target = resolve(input, cwd);
    let pattern = input.contains(['*', '?', '[']) && !target.exists();
    if input.contains("**") {
        bail!("Recursive patterns are not supported. Choose a directory and a file-name pattern.");
    }
    if pattern {
        let options = MatchOptions {
            case_sensitive: !cfg!(windows),
            require_literal_separator: true,
            require_literal_leading_dot: !hidden,
        };
        let mut root = PathBuf::new();
        let mut tail = PathBuf::new();
        let mut started = false;
        for component in target.components() {
            if !started
                && component
                    .as_os_str()
                    .to_string_lossy()
                    .contains(['*', '?', '['])
                && !root.join(component).is_dir()
            {
                started = true;
            }
            if started {
                tail.push(component);
            } else {
                root.push(component);
            }
        }
        let expression = format!(
            "{}{}{}",
            Pattern::escape(root.to_str().context("The path is not valid UTF-8.")?)
                .trim_end_matches(std::path::MAIN_SEPARATOR),
            std::path::MAIN_SEPARATOR,
            tail.to_str().context("The path is not valid UTF-8.")?
        );
        let mut entries = Vec::new();
        let mut truncated = false;
        for entry in glob_with(&expression, options).context("Invalid path pattern")? {
            let path = entry.context("Cannot read a matching path")?;
            if let Some(entry) = entry_for(path) {
                if directories_only && !entry.is_dir {
                    continue;
                }
                if entries.len() == 2000 {
                    truncated = true;
                    break;
                }
                entries.push(entry);
            }
        }
        entries.sort_by(|a, b| (!a.is_dir).cmp(&(!b.is_dir)).then(a.path.cmp(&b.path)));
        return Ok(PathMatches {
            directory: None,
            root,
            entries,
            pattern,
            truncated,
        });
    }
    let directory_input = input.is_empty()
        || input == "."
        || input == ".."
        || input.ends_with(std::path::MAIN_SEPARATOR)
        || input.ends_with('/')
        || input == "~";
    let (root, needle) = if directory_input {
        (target, String::new())
    } else {
        (
            target
                .parent()
                .context("The path has no parent directory.")?
                .to_path_buf(),
            target
                .file_name()
                .context("The path has no file name.")?
                .to_string_lossy()
                .into_owned(),
        )
    };
    let mut entries = local_entries(&root, hidden || needle.starts_with('.'))?;
    if directories_only {
        entries.retain(|entry| entry.is_dir);
    }
    if !needle.is_empty() {
        entries.retain(|e| fuzzy_score(&e.name, &needle).is_some());
        entries.sort_by_key(|e| {
            (
                fuzzy_score(&e.name, &needle).unwrap_or(usize::MAX),
                !e.is_dir,
                e.name.to_lowercase(),
            )
        });
    }
    let truncated = entries.len() > 2000;
    entries.truncate(2000);
    Ok(PathMatches {
        directory: None,
        root,
        entries,
        pattern,
        truncated,
    })
}

fn entry_for(path: PathBuf) -> Option<LocalEntry> {
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() && !metadata.is_dir() {
        return None;
    }
    Some(LocalEntry {
        name: path.file_name()?.to_string_lossy().into_owned(),
        path,
        is_dir: metadata.is_dir(),
        size: if metadata.is_file() {
            Some(metadata.len())
        } else {
            None
        },
    })
}

pub fn completion(
    input: &str,
    cwd: &Path,
    entries: &[LocalEntry],
    selected: usize,
) -> Option<String> {
    let entry = entries.get(selected)?;
    let strings: Vec<_> = entries
        .iter()
        .map(|e| e.path.to_string_lossy().into_owned())
        .collect();
    if entries.len() > 1 && selected == 0 && !input.contains(['*', '?', '[']) {
        let first = &strings[0];
        let mut end = first.len();
        for other in &strings[1..] {
            end = end.min(
                first
                    .bytes()
                    .zip(other.bytes())
                    .take_while(|(a, b)| a == b)
                    .count(),
            );
        }
        while !first.is_char_boundary(end) {
            end -= 1;
        }
        if end > resolve(input, cwd).to_string_lossy().len() {
            return Some(first[..end].to_string());
        }
    }
    let mut value = entry.path.to_string_lossy().into_owned();
    if entry.is_dir {
        value.push(std::path::MAIN_SEPARATOR);
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        for name in [
            "report one.txt",
            "report two.txt",
            "results.csv",
            "论文草稿.pdf",
            "[draft].txt",
            ".hidden.txt",
        ] {
            std::fs::write(temp.path().join(name), "sample").unwrap();
        }
        std::fs::create_dir(temp.path().join("folder")).unwrap();
        temp
    }

    #[test]
    fn filename_prefixes_and_fuzzy_queries_find_literal_paths_with_spaces() {
        let temp = fixture();
        let result = matches("report", temp.path(), false).unwrap();
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].name, "report one.txt");
        let fuzzy = matches("rptw", temp.path(), false).unwrap();
        assert_eq!(fuzzy.entries.len(), 1);
        assert_eq!(fuzzy.entries[0].name, "report two.txt");
        assert_eq!(
            matches("REPORT", temp.path(), false).unwrap().entries.len(),
            2
        );
    }

    #[test]
    fn chinese_and_quoted_absolute_paths_are_supported() {
        let temp = fixture();
        assert_eq!(
            matches("论文", temp.path(), false).unwrap().entries[0].name,
            "论文草稿.pdf"
        );
        let absolute = format!("\"{}\"", temp.path().join("report one.txt").display());
        let result = matches(&absolute, temp.path(), false).unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, "report one.txt");
    }

    #[test]
    fn glob_patterns_respect_hidden_names() {
        let temp = fixture();
        let result = matches("report *.txt", temp.path(), false).unwrap();
        assert!(result.pattern);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(
            matches("*.txt", temp.path(), false).unwrap().entries.len(),
            3
        );
        assert_eq!(
            matches("*.txt", temp.path(), true).unwrap().entries.len(),
            4
        );
        assert_eq!(matches(".*", temp.path(), false).unwrap().entries.len(), 1);
        assert_eq!(
            matches(".hid", temp.path(), false).unwrap().entries.len(),
            1
        );
    }

    #[test]
    fn existing_literal_brackets_in_file_and_directory_names_are_not_patterns() {
        let temp = fixture();
        let literal = matches("[draft].txt", temp.path(), false).unwrap();
        assert!(!literal.pattern);
        assert_eq!(literal.entries.len(), 1);
        let directory = temp.path().join("[papers]");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("one.pdf"), "data").unwrap();
        assert_eq!(
            matches("*.pdf", &directory, false).unwrap().entries.len(),
            1
        );
        let absolute = format!("{}{}*.pdf", directory.display(), std::path::MAIN_SEPARATOR);
        assert_eq!(
            matches(&absolute, temp.path(), false)
                .unwrap()
                .entries
                .len(),
            1
        );
    }

    #[test]
    fn empty_queries_browse_directories_and_relative_parent_paths_work() {
        let temp = fixture();
        let browse = matches("", temp.path(), false).unwrap();
        assert!(browse.entries[0].is_dir);
        assert_eq!(browse.entries.len(), 6);
        let child = temp.path().join("folder");
        assert!(
            matches("..", &child, false)
                .unwrap()
                .entries
                .iter()
                .any(|entry| entry.name == "results.csv")
        );
        assert!(!matches(".", temp.path(), false).unwrap().entries.is_empty());
        assert_eq!(
            matches("../results.csv", &child, false).unwrap().entries[0].name,
            "results.csv"
        );
        assert!(
            matches(
                &format!("folder{}", std::path::MAIN_SEPARATOR),
                temp.path(),
                false
            )
            .unwrap()
            .entries
            .is_empty()
        );
    }

    #[test]
    fn invalid_paths_and_patterns_have_recoverable_results() {
        let temp = fixture();
        assert!(
            matches("missing-file", temp.path(), false)
                .unwrap()
                .entries
                .is_empty()
        );
        assert!(matches("missing-dir/file", temp.path(), false).is_err());
        assert!(matches("unclosed[", temp.path(), false).is_err());
        assert!(matches("**/*.txt", temp.path(), false).is_err());
    }

    #[test]
    fn completion_extends_common_prefixes_and_completes_the_selected_match() {
        let temp = fixture();
        let result = matches("rep", temp.path(), false).unwrap();
        assert!(
            completion("rep", temp.path(), &result.entries, 0)
                .unwrap()
                .ends_with("report ")
        );
        assert!(
            completion("rep", temp.path(), &result.entries, 1)
                .unwrap()
                .ends_with("report two.txt")
        );
        let directory = matches("fold", temp.path(), false).unwrap();
        assert!(
            completion("fold", temp.path(), &directory.entries, 0)
                .unwrap()
                .ends_with(std::path::MAIN_SEPARATOR)
        );
    }

    #[test]
    fn unicode_common_prefixes_never_split_utf8_or_panic() {
        let temp = fixture();
        for name in ["报告-a.txt", "报告-b.txt", "论点.txt", "讲义.txt"] {
            std::fs::write(temp.path().join(name), "data").unwrap();
        }
        let result = matches("报", temp.path(), false).unwrap();
        assert!(
            completion("报", temp.path(), &result.entries, 0)
                .unwrap()
                .ends_with("报告-")
        );
        let result = matches("*.txt", temp.path(), false).unwrap();
        assert!(completion("", temp.path(), &result.entries, 0).is_some());
        assert!(completion("", temp.path(), &[], 0).is_none());
    }

    #[test]
    fn home_shorthand_expands_without_changing_process_environment() {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            assert_eq!(
                resolve("~/Downloads", Path::new(".")),
                PathBuf::from(home).join("Downloads")
            );
        }
    }

    #[test]
    fn directory_matching_excludes_files_and_can_select_an_empty_directory() {
        let temp = fixture();
        let result = directories("", temp.path(), false).unwrap();
        assert!(result.directory.is_some());
        assert_eq!(result.entries.len(), 1);
        assert!(result.entries.iter().all(|entry| entry.is_dir));
        let empty = temp.path().join("folder");
        let result = directories(empty.to_str().unwrap(), temp.path(), false).unwrap();
        assert_eq!(result.directory, Some(empty));
        assert!(result.entries.is_empty());
    }

    #[test]
    fn directory_matching_supports_prefixes_spaces_hidden_names_and_rejects_files() {
        let temp = fixture();
        for name in ["下载 资料", ".hidden-folder"] {
            std::fs::create_dir(temp.path().join(name)).unwrap();
        }
        let prefix = directories("下载", temp.path(), false).unwrap();
        assert!(prefix.directory.is_none());
        assert_eq!(prefix.entries[0].name, "下载 资料");
        assert!(
            !directories("", temp.path(), false)
                .unwrap()
                .entries
                .iter()
                .any(|entry| entry.name.starts_with('.'))
        );
        assert!(
            directories("", temp.path(), true)
                .unwrap()
                .entries
                .iter()
                .any(|entry| entry.name == ".hidden-folder")
        );
        assert!(directories("results.csv", temp.path(), false).is_err());
    }

    #[test]
    fn browse_lists_the_whole_parent_of_a_file_and_recovers_from_an_unfinished_path() {
        let temp = fixture();
        let result = browse("report one.txt", temp.path(), false, false).unwrap();
        assert!(
            result
                .entries
                .iter()
                .any(|entry| entry.name == "report two.txt")
        );
        let result = browse("missing/subfolder", temp.path(), false, true).unwrap();
        assert!(result.entries.iter().all(|entry| entry.is_dir));
        assert_eq!(result.directory.as_deref(), Some(temp.path()));
    }
}

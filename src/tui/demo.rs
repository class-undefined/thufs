use crate::app::{
    account_service::{AccountInfoResult, RepoItem},
    list_service::ListItem,
    share_service::ShareItem,
};

use super::backend::{Message, Request};

/// An explicit offline fixture. No cloud requests or filesystem writes are allowed here.
pub fn respond(request: Request) -> Message {
    match request {
        Request::EntryDate { id, location, .. } => Message::EntryDate {
            id,
            location,
            result: Ok(Some("2026-11-20 16:30:00 +08:00".into())),
        },
        Request::SaveProxy { id, proxy } => Message::ProxySaved {
            id,
            result: proxy.normalized().map_err(|error| error.to_string()),
        },
        Request::UseSession { .. } | Request::SaveLanguage(_) => {
            unreachable!("settings are dispatched separately")
        }
        Request::SaveUpdateChecks(enabled) => Message::UpdateChecksSaved {
            enabled,
            result: Ok(()),
        },
        Request::CheckUpdates { id, .. } => Message::UpdateChecked {
            id,
            result: Ok(crate::app::update_service::UpdateCheck {
                current_version: env!("CARGO_PKG_VERSION").into(),
                latest_version: Some(env!("CARGO_PKG_VERSION").into()),
                update_available: false,
                release_url: crate::app::update_service::RELEASES_URL.into(),
                checked_at: 0,
                cached: false,
            }),
        },
        Request::SaveUserAgent { id, value } => Message::UserAgentSaved {
            id,
            result: crate::config::normalize_user_agent(&value).map_err(|error| error.to_string()),
        },
        Request::Authenticate { id, .. } => Message::Authenticated {
            id,
            result: Err("Demo is read-only".into()),
        },
        Request::SignOut { id } => Message::SignedOut {
            id,
            result: Err("Demo is read-only".into()),
        },
        Request::Account => Message::Account(Ok(AccountInfoResult {
            login_id: None,
            name: Some("Demo workspace".into()),
            email: "demo@example.invalid".into(),
            contact_email: None,
            is_staff: None,
            usage: Some(8_375_186_227),
            total: Some(107_374_182_400),
            space_usage: None,
            institution: Some("Tsinghua University".into()),
            department: None,
        })),
        Request::Repositories { id } => Message::Repositories {
            id,
            result: Ok([
                ("demo-research", "Research"),
                ("demo-courses", "Coursework"),
                ("demo-personal", "Personal"),
            ]
            .into_iter()
            .map(|(id, name)| RepoItem {
                id: id.into(),
                name: name.into(),
            })
            .collect()),
        },
        Request::Directory { id, location } => Message::Directory {
            id,
            result: Ok(entries(&location.path)),
        },
        Request::LibraryIndex { id, cancel, .. } => {
            let mut pending = vec!["/".to_string()];
            let mut all = Vec::new();
            while let Some(path) = pending.pop() {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Message::LibraryIndex {
                        id,
                        result: Err("Search cancelled".into()),
                    };
                }
                for entry in entries(&path) {
                    if entry.kind == "dir" {
                        pending.push(entry.path.clone());
                    }
                    all.push(entry);
                }
            }
            Message::LibraryIndex {
                id,
                result: Ok(all),
            }
        }
        Request::Shares { id } => Message::Shares {
            id,
            result: Ok(vec![ShareItem {
                repo: Some("Research".into()),
                repo_id: Some("demo-research".into()),
                path: Some("/paper-draft.pdf".into()),
                token: Some("demo-offline-link".into()),
                link: "https://example.invalid/demo-share/".into(),
                url: "https://example.invalid/demo-share/".into(),
                is_dir: Some(false),
                created_at: Some("2026-11-23".into()),
                expire_date: Some("2026-11-30".into()),
                view_count: Some(12),
            }]),
        },
        Request::CreateShare { id, .. }
        | Request::CreateRepository { id, .. }
        | Request::CreateDirectory { id, .. }
        | Request::Revoke { id, .. } => Message::Operation {
            id,
            result: Err("Demo is read-only".into()),
        },
        Request::Copy(text) => super::backend::execute(Request::Copy(text)),
        Request::MatchPaths {
            id,
            input,
            cwd,
            hidden,
            directories_only,
            browse,
        } => Message::PathMatches {
            id,
            result: if browse {
                super::paths::browse(&input, &cwd, hidden, directories_only)
            } else if directories_only {
                super::paths::directories(&input, &cwd, hidden)
            } else {
                super::paths::matches(&input, &cwd, hidden)
            }
            .map_err(|e| e.to_string()),
        },
        Request::Transfer { .. } | Request::CancelTransfer { .. } => {
            unreachable!("transfers are guarded by the backend")
        }
    }
}

pub fn entries(path: &str) -> Vec<ListItem> {
    let rows = if path == "/" {
        vec![
            ("experiments", "dir", None),
            ("datasets", "dir", None),
            ("读书笔记", "dir", None),
            ("paper-draft.pdf", "file", Some(2_560_000)),
            ("results.csv", "file", Some(148_224)),
            ("README.md", "file", Some(3_420)),
            ("模型参数.json", "file", Some(8_192)),
            ("training-curves.png", "file", Some(582_400)),
        ]
    } else if path.ends_with("experiments") {
        vec![
            ("run-001", "dir", None),
            ("run-002", "dir", None),
            ("ablation-study.csv", "file", Some(98_304)),
            ("experiment-notes.md", "file", Some(6_400)),
        ]
    } else if path.ends_with("datasets") {
        vec![
            ("benchmark.jsonl", "file", Some(18_874_368)),
            ("dataset-card.md", "file", Some(4_096)),
        ]
    } else if path.ends_with("读书笔记") {
        vec![("本周阅读.md", "file", Some(7_168))]
    } else {
        Vec::new()
    };
    rows.into_iter()
        .map(|(name, kind, size)| ListItem {
            name: name.into(),
            path: format!("{}/{name}", path.trim_end_matches('/')),
            kind: kind.into(),
            size,
            updated_at: Some("2026-11-20 16:30:00 +08:00".into()),
        })
        .collect()
}

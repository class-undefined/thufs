use std::{path::PathBuf, time::Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use tempfile::{TempDir, tempdir};

use super::{
    backend::{
        Message, Operation, Request, TransferOutcome, parse_transfer_outcome, transfer_arguments,
    },
    demo,
    editor::Editor,
    model::{
        Action, Focus, FormKind, Model, Overlay, TransferSpec, TransferStatus, Workspace,
        normalized_path, palette_matches,
    },
    ui,
};

fn key(model: &mut Model, code: KeyCode) {
    model.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn ctrl(model: &mut Model, ch: char) {
    model.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL));
}

fn flush_demo(model: &mut Model) {
    while !model.requests.is_empty() {
        for request in std::mem::take(&mut model.requests) {
            model.handle_message(demo::respond(request));
        }
    }
}

fn fixture() -> (TempDir, Model) {
    let temp = tempdir().unwrap();
    let mut model = Model::new(temp.path().to_path_buf(), None, true);
    flush_demo(&mut model);
    model.demo = false;
    (temp, model)
}

fn select_file(model: &mut Model, name: &str) {
    model.cursor = model
        .file_indices()
        .iter()
        .position(|&i| model.entries[i].name == name)
        .unwrap();
}

fn queue_downloads(model: &mut Model, count: usize) {
    model.selected = model
        .entries
        .iter()
        .filter(|f| f.kind == "file")
        .take(count)
        .map(|f| f.path.clone())
        .collect();
    model.action(Action::Download);
    assert!(matches!(model.overlay, Some(Overlay::Picker(_))));
    flush_demo(model);
    key(model, KeyCode::Enter);
    assert!(matches!(model.overlay, Some(Overlay::Form(_))));
    key(model, KeyCode::Enter);
    assert!(model.overlay.is_none());
}

fn render(model: &Model, width: u16, height: u16) -> (String, ui::HitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = ui::HitMap::default();
    terminal
        .draw(|frame| hits = ui::draw(frame, model))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .chunks(width as usize)
        .map(|row| {
            let mut text = String::new();
            let mut continuation = 0;
            for cell in row {
                if continuation > 0 {
                    continuation -= 1;
                    continue;
                }
                text.push_str(cell.symbol());
                continuation = ui::cell_width(cell.symbol()).saturating_sub(1);
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, hits)
}

#[test]
fn startup_loads_a_library_and_navigation_restores_parent_cursor() {
    let (_temp, mut model) = fixture();
    assert!(model.connected);
    assert_eq!(model.location.as_ref().unwrap().repo, "Research");
    select_file(&mut model, "experiments");
    let cursor = model.cursor;
    model.enter();
    flush_demo(&mut model);
    assert_eq!(model.location.as_ref().unwrap().path, "/experiments");
    model.cursor = 2;
    model.parent();
    flush_demo(&mut model);
    assert_eq!(model.location.as_ref().unwrap().path, "/");
    assert_eq!(model.cursor, cursor);
    model.parent();
    assert_eq!(model.focus, Focus::Libraries);
}

#[test]
fn switching_workspaces_preserves_the_current_file() {
    let (_temp, mut model) = fixture();
    select_file(&mut model, "results.csv");
    let position = model.cursor;
    model.switch(Workspace::Transfers);
    model.switch(Workspace::Files);
    assert_eq!(model.cursor, position);
    assert_eq!(model.current_file().unwrap().name, "results.csv");
    model.filter = Editor::new("csv");
    model.cursor = 0;
    model.switch(Workspace::Shares);
    model.switch(Workspace::Files);
    assert_eq!(model.current_file().unwrap().name, "results.csv");
}

#[test]
fn stale_directory_responses_cannot_replace_the_current_directory() {
    let (_temp, mut model) = fixture();
    let root = model.location.clone().unwrap();
    model.navigate(root.child("/experiments"));
    let old = model.requests.pop().unwrap();
    model.navigate(root.child("/datasets"));
    let latest = model.requests.pop().unwrap();
    model.handle_message(demo::respond(old));
    assert!(model.entries.is_empty());
    assert!(model.loading);
    model.handle_message(demo::respond(latest));
    assert!(!model.loading);
    assert_eq!(model.location.unwrap().path, "/datasets");
    assert!(model.entries.iter().any(|f| f.name == "benchmark.jsonl"));
}

#[test]
fn a_failed_directory_request_can_be_retried_without_losing_its_path() {
    let (_temp, mut model) = fixture();
    model.navigate(model.location.clone().unwrap().child("/datasets"));
    let Request::Directory { id, .. } = model.requests.pop().unwrap() else {
        panic!("directory request")
    };
    model.handle_message(Message::Directory {
        id,
        result: Err("offline".into()),
    });
    assert!(!model.loading);
    assert!(model.load_error.is_some());
    model.action(Action::Refresh);
    flush_demo(&mut model);
    assert!(model.load_error.is_none());
    assert_eq!(model.location.unwrap().path, "/datasets");
}

#[test]
fn account_failure_does_not_prevent_library_browsing() {
    let (_temp, mut model) = fixture();
    model.account = None;
    model.handle_message(Message::Account(Err("403".into())));
    assert!(model.connected);
    assert!(!model.entries.is_empty());
    assert!(model.notice.unwrap().error);
}

#[test]
fn filtering_supports_chinese_and_esc_clears_the_filter() {
    let (_temp, mut model) = fixture();
    key(&mut model, KeyCode::Char('f'));
    model.paste("模型");
    assert_eq!(model.file_indices().len(), 1);
    assert_eq!(model.current_file().unwrap().name, "模型参数.json");
    key(&mut model, KeyCode::Enter);
    assert!(!model.filtering);
    assert_eq!(model.file_indices().len(), 1);
    key(&mut model, KeyCode::Esc);
    assert_eq!(model.file_indices().len(), 8);
}

#[test]
fn select_all_only_selects_visible_files_and_preserves_hidden_selections() {
    let (_temp, mut model) = fixture();
    model.filter = Editor::new("pdf");
    key(&mut model, KeyCode::Char('a'));
    assert_eq!(model.selected.len(), 1);
    assert!(model.selected.contains("/paper-draft.pdf"));
    model.filter = Editor::new("csv");
    key(&mut model, KeyCode::Char('a'));
    assert_eq!(model.selected.len(), 2);
    key(&mut model, KeyCode::Esc);
    key(&mut model, KeyCode::Char('a'));
    assert_eq!(model.selected.len(), 5);
    assert!(
        model
            .entries
            .iter()
            .filter(|f| f.kind == "dir")
            .all(|f| !model.selected.contains(&f.path))
    );
}

#[test]
fn home_end_and_large_movements_are_bounded() {
    let (_temp, mut model) = fixture();
    key(&mut model, KeyCode::End);
    assert_eq!(model.cursor, 7);
    key(&mut model, KeyCode::Home);
    assert_eq!(model.cursor, 0);
    model.entries.clear();
    model.move_cursor(isize::MAX);
    model.move_cursor(isize::MIN);
    assert_eq!(model.cursor, 0);
}

#[test]
fn slash_palette_is_searchable_and_text_shortcuts_do_not_trigger_actions() {
    let (_temp, mut model) = fixture();
    ctrl(&mut model, 'u');
    assert!(model.overlay.is_none());
    ctrl(&mut model, 'p');
    model.paste("trns");
    key(&mut model, KeyCode::Enter);
    assert_eq!(model.workspace, Workspace::Transfers);
    assert!(model.overlay.is_none());
    assert_eq!(
        super::model::COMMANDS[palette_matches("/upload")[0]].action,
        Action::Upload
    );
    model.open_palette();
    key(&mut model, KeyCode::Char('q'));
    assert!(!model.should_quit);
    key(&mut model, KeyCode::Esc);
    assert!(model.overlay.is_none());
}

#[test]
fn downloads_queue_one_transfer_at_a_time_and_keep_final_saved_paths() {
    let (_temp, mut model) = fixture();
    queue_downloads(&mut model, 2);
    assert_eq!(model.transfers[0].status, TransferStatus::Running);
    assert_eq!(model.transfers[1].status, TransferStatus::Queued);
    assert_eq!(
        model
            .requests
            .iter()
            .filter(|r| matches!(r, Request::Transfer { .. }))
            .count(),
        1
    );
    model.requests.clear();
    let id = model.transfers[0].id;
    model.handle_message(Message::Progress {
        id,
        transferred: 3,
        total: Some(10),
        warning: None,
    });
    assert_eq!(model.transfers[0].ratio(), 0.3);
    model.handle_message(Message::TransferFinished {
        id,
        result: Ok(TransferOutcome {
            path: "/tmp/paper-(1).pdf".into(),
            bytes: 10,
            uniquified: true,
        }),
        cancelled: false,
    });
    assert_eq!(model.transfers[0].status, TransferStatus::Done);
    assert_eq!(
        model.transfers[0].final_path.as_deref(),
        Some("/tmp/paper-(1).pdf")
    );
    assert_eq!(model.transfers[1].status, TransferStatus::Running);
    assert_eq!(
        model
            .requests
            .iter()
            .filter(|r| matches!(r, Request::Transfer { .. }))
            .count(),
        1
    );
}

#[test]
fn cancelling_a_queued_transfer_never_starts_it() {
    let (_temp, mut model) = fixture();
    queue_downloads(&mut model, 2);
    model.requests.clear();
    model.transfer_cursor = 1;
    model.cancel_transfer();
    assert_eq!(model.transfers[1].status, TransferStatus::Cancelled);
    assert!(model.requests.is_empty());
    let id = model.transfers[0].id;
    model.handle_message(Message::TransferFinished {
        id,
        result: Err("failed".into()),
        cancelled: false,
    });
    assert_eq!(model.active_transfers(), 0);
}

#[test]
fn failed_and_cancelled_transfers_can_be_retried() {
    let (_temp, mut model) = fixture();
    queue_downloads(&mut model, 1);
    let id = model.transfers[0].id;
    model.handle_message(Message::TransferFinished {
        id,
        result: Err("network disconnected".into()),
        cancelled: false,
    });
    assert_eq!(model.transfers[0].status, TransferStatus::Failed);
    model.switch(Workspace::Transfers);
    model.action(Action::Refresh);
    assert_eq!(model.transfers[0].status, TransferStatus::Running);
    assert!(model.transfers[0].detail.is_empty());
    model.handle_message(Message::TransferFinished {
        id,
        result: Err("cancelled".into()),
        cancelled: true,
    });
    assert_eq!(model.transfers[0].status, TransferStatus::Cancelled);
    model.action(Action::Refresh);
    assert_eq!(model.transfers[0].status, TransferStatus::Running);
}

#[test]
fn quitting_with_transfers_requires_confirmation_and_waits_for_cancellation() {
    let (_temp, mut model) = fixture();
    queue_downloads(&mut model, 2);
    model.requests.clear();
    model.action(Action::Quit);
    assert!(matches!(model.overlay, Some(Overlay::Quit)));
    key(&mut model, KeyCode::Esc);
    assert!(!model.quitting);
    assert!(model.requests.is_empty());
    model.action(Action::Quit);
    key(&mut model, KeyCode::Enter);
    assert!(model.quitting);
    assert!(!model.should_quit);
    assert_eq!(model.transfers[1].status, TransferStatus::Cancelled);
    assert!(
        model
            .requests
            .iter()
            .any(|r| matches!(r, Request::CancelTransfer { .. }))
    );
    let id = model.transfers[0].id;
    model.handle_message(Message::TransferFinished {
        id,
        result: Err("cancelled".into()),
        cancelled: true,
    });
    assert!(model.should_quit);
}

#[test]
fn share_form_validates_expiration_and_keeps_password_out_of_the_rendered_ui() {
    let (_temp, mut model) = fixture();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Share);
    model.paste(" private password ");
    let (screen, _) = render(&model, 100, 32);
    assert!(!screen.contains("private password"));
    assert!(model.requests.is_empty());
    key(&mut model, KeyCode::Tab);
    ctrl(&mut model, 'u');
    model.paste("0");
    key(&mut model, KeyCode::Enter);
    assert!(matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some()));
    assert!(model.requests.is_empty());
    ctrl(&mut model, 'u');
    model.paste("3");
    key(&mut model, KeyCode::Enter);
    let Request::CreateShare {
        id,
        password,
        expire_days,
        remote,
    } = model.requests.pop().unwrap()
    else {
        panic!("create share")
    };
    assert_eq!(password.as_deref(), Some(" private password "));
    assert_eq!(expire_days, Some(3));
    assert_eq!(remote, "repo:Research/paper-draft.pdf");
    model.handle_message(Message::Operation {
        id,
        result: Err("server unavailable".into()),
    });
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.as_deref() == Some("server unavailable") && form.fields[0].editor.text == " private password ")
    );
}

#[test]
fn revoking_a_share_only_sends_a_request_after_confirmation() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Shares);
    flush_demo(&mut model);
    key(&mut model, KeyCode::Char('x'));
    assert!(model.requests.is_empty());
    key(&mut model, KeyCode::Esc);
    assert!(model.requests.is_empty());
    key(&mut model, KeyCode::Char('x'));
    key(&mut model, KeyCode::Enter);
    let Request::Revoke { id, token } = model.requests.pop().unwrap() else {
        panic!("revoke request")
    };
    assert_eq!(token, "demo-offline-link");
    model.handle_message(Message::Operation {
        id,
        result: Ok(Operation::Revoked),
    });
    assert!(model.overlay.is_none());
    assert!(
        model
            .requests
            .iter()
            .any(|r| matches!(r, Request::Shares { .. }))
    );
}

#[test]
fn demo_mode_never_enqueues_cloud_mutations() {
    let (_temp, mut model) = fixture();
    model.demo = true;
    for action in [Action::Share, Action::Mkdir, Action::NewLibrary] {
        model.action(action);
        assert!(model.requests.is_empty());
    }
    model.switch(Workspace::Shares);
    flush_demo(&mut model);
    key(&mut model, KeyCode::Char('x'));
    key(&mut model, KeyCode::Enter);
    assert!(model.requests.is_empty());
    assert!(model.transfers.is_empty());
}

#[test]
fn unsafe_folder_names_are_rejected_before_any_server_request() {
    let (_temp, mut model) = fixture();
    for name in ["../escape", "/absolute", "one//two", "one/./two"] {
        model.action(Action::Mkdir);
        model.paste(name);
        key(&mut model, KeyCode::Enter);
        assert!(matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some()));
        assert!(model.requests.is_empty());
        key(&mut model, KeyCode::Esc);
    }
}

#[test]
fn clipboard_requests_copy_remote_paths_not_authentication_data() {
    let (_temp, mut model) = fixture();
    select_file(&mut model, "模型参数.json");
    key(&mut model, KeyCode::Char('y'));
    assert!(
        matches!(model.requests.pop(), Some(Request::Copy(text)) if text == "repo:Research/模型参数.json")
    );
}

#[test]
fn editor_deletes_whole_graphemes_and_uses_cell_width_for_the_cursor() {
    let mut editor = Editor::new("论文👩‍💻e\u{301}");
    editor.handle(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(editor.text, "论文👩‍💻");
    editor.handle(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(editor.text, "论文");
    let (visible, cursor) = editor.viewport(10, false);
    assert_eq!(visible, "论文");
    assert_eq!(cursor, 4);
    editor.handle(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    editor.insert("A");
    assert_eq!(editor.text, "A论文");
    editor.handle(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    assert_eq!(editor.text, "A文");
}

#[test]
fn editor_handles_multibyte_scrolling_and_sanitizes_pasted_control_characters() {
    let mut editor = Editor::new("文件名很长很长");
    let (visible, cursor) = editor.viewport(7, false);
    assert!(ui::cell_width(&visible) <= 7);
    assert!(cursor < 7);
    editor.insert("\n\r\t\x1btest");
    assert!(!editor.text.chars().any(char::is_control));
    let (secret, _) = editor.viewport(50, true);
    assert!(!secret.contains("test"));
    assert!(secret.contains('•'));
}

#[test]
fn all_workspaces_and_overlays_render_at_supported_terminal_sizes() {
    let (_temp, mut model) = fixture();
    for (width, height) in [(128, 36), (100, 30), (80, 22), (60, 18), (40, 12), (1, 1)] {
        for workspace in [
            Workspace::Files,
            Workspace::Transfers,
            Workspace::Shares,
            Workspace::Settings,
        ] {
            model.switch(workspace);
            let (text, _) = render(&model, width, height);
            if width >= 60 {
                assert!(text.contains("thufs"));
            }
        }
        model.switch(Workspace::Files);
        for action in [
            Action::Help,
            Action::Jump,
            Action::Mkdir,
            Action::NewLibrary,
            Action::Share,
            Action::Upload,
        ] {
            model.action(action);
            render(&model, width, height);
            key(&mut model, KeyCode::Esc);
        }
        model.open_palette();
        model.paste("no-such-command");
        render(&model, width, height);
        key(&mut model, KeyCode::Esc);
    }
}

#[test]
fn narrow_layout_keeps_files_visible_and_tab_exposes_libraries() {
    let (_temp, mut model) = fixture();
    let (wide, _) = render(&model, 128, 36);
    assert!(wide.contains("INSPECTOR"));
    let (narrow, _) = render(&model, 80, 22);
    assert!(!narrow.contains("INSPECTOR"));
    assert!(narrow.contains("paper-draft.pdf"));
    key(&mut model, KeyCode::Tab);
    let (libraries, _) = render(&model, 80, 22);
    assert!(libraries.contains("LIBRARIES"));
    key(&mut model, KeyCode::Enter);
    assert_eq!(model.focus, Focus::Files);
    flush_demo(&mut model);
    let (minimum, _) = render(&model, 60, 18);
    assert!(minimum.contains("datasets/"));
}

#[test]
fn mouse_selection_respects_table_headers_in_both_layouts() {
    let (_temp, mut model) = fixture();
    for (width, height) in [(128, 36), (80, 22)] {
        let (_, hits) = render(&model, width, height);
        let mut last = None;
        super::handle_mouse(
            &mut model,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: hits.files.x + 5,
                row: hits.files.y + hits.file_header + 1,
                modifiers: KeyModifiers::NONE,
            },
            &hits,
            &mut last,
        );
        assert_eq!(model.cursor, 1);
        assert!(last.is_some_and(|(time, _, _)| time <= Instant::now()));
        model.cursor = 0;
    }
}

#[test]
fn terminal_control_characters_in_remote_names_are_never_rendered() {
    let (_temp, mut model) = fixture();
    model.entries[0].name = "evil\x1b[2J\nname".into();
    let (text, _) = render(&model, 128, 36);
    assert!(!text.contains('\x1b'));
    assert_eq!(ui::safe("a\x1b\r\n\tb"), "ab");
}

#[test]
fn transfer_process_arguments_are_literal_and_cannot_be_parsed_as_options() {
    let spec = TransferSpec::Upload {
        local: PathBuf::from("-file $(touch nope).txt"),
        remote: "repo:Research/a b.txt".into(),
        policy: "uniquify".into(),
    };
    let args = transfer_arguments(&spec);
    assert_eq!(args[6], "--");
    assert_eq!(args[7], "-file $(touch nope).txt");
    assert_eq!(args[8], "repo:Research/a b.txt");
}

#[test]
fn transfer_results_keep_unique_local_and_remote_paths() {
    let download = parse_transfer_outcome(
        r#"{"final_local_path":"/tmp/论文-(1).pdf","bytes_written":12,"uniquified":true}"#,
    )
    .unwrap();
    assert_eq!(download.path, "/tmp/论文-(1).pdf");
    assert!(download.uniquified);
    let upload =
        parse_transfer_outcome(r#"{"repo":"Research","final_remote_path":"/a.txt","size":42}"#)
            .unwrap();
    assert_eq!(upload.bytes, 42);
    assert_eq!(upload.path, "repo:Research/a.txt");
    assert!(parse_transfer_outcome(r#"{"final_remote_path":"/a.txt"}"#).is_err());
    assert!(parse_transfer_outcome("{}").is_err());
    assert!(parse_transfer_outcome("broken").is_err());
}

#[test]
fn jump_paths_cannot_escape_a_library_root() {
    assert_eq!(normalized_path("/a/../../b/./"), "/b");
    assert_eq!(normalized_path("/../../../"), "/");
}

#[test]
fn upload_picker_ignores_results_for_a_previously_opened_directory() {
    let (_temp, mut model) = fixture();
    model.action(Action::Upload);
    let Request::MatchPaths { id: old, .. } = model.requests.pop().unwrap() else {
        panic!("local request")
    };
    model.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::ALT));
    let Request::MatchPaths { id: new, .. } = model.requests.pop().unwrap() else {
        panic!("local request")
    };
    model.handle_message(Message::PathMatches {
        id: old,
        result: Err("stale error".into()),
    });
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.loading && p.error.is_none()));
    model.handle_message(Message::PathMatches {
        id: new,
        result: Ok(super::paths::PathMatches {
            directory: None,
            root: model.local.clone(),
            entries: vec![],
            pattern: false,
            truncated: false,
        }),
    });
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if !p.loading && p.error.is_none()));
}

#[test]
fn transfer_policy_is_explicit_and_defaults_to_avoiding_overwrites() {
    let (_temp, mut model) = fixture();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(f)) if f.policy() == "uniquify" && matches!(f.kind, FormKind::Download(_)))
    );
    ctrl(&mut model, 'o');
    assert!(matches!(&model.overlay, Some(Overlay::Form(f)) if f.policy() == "overwrite"));
    ctrl(&mut model, 'o');
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.transfers[0].spec, TransferSpec::Download { policy, .. } if policy == "fail")
    );
}

fn auth_result(model: &Model, email: &str) -> super::settings::AuthorizedAccount {
    let mut account = model.account.clone().unwrap();
    account.email = email.into();
    account.name = Some("Next user".into());
    super::settings::AuthorizedAccount {
        token: super::settings::SecretToken::new("next-account-token".into()),
        account,
        settings: super::settings::SettingsView {
            language: model.language(),
            active_account: Some(format!("account:{email}")),
            accounts: vec![super::settings::AccountChoice {
                key: format!("account:{email}"),
                label: "Next user".into(),
                detail: email.into(),
            }],
        },
    }
}

#[test]
fn missing_credentials_open_a_masked_token_login_inside_the_tui() {
    let temp = tempdir().unwrap();
    let mut model = Model::new(temp.path().to_path_buf(), None, false);
    model.configure_settings(
        super::settings::SettingsView {
            language: super::i18n::Language::English,
            active_account: None,
            accounts: vec![],
        },
        false,
    );
    assert!(!model.signed_in);
    assert!(model.requests.is_empty());
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(form.kind, FormKind::Login) && form.fields[0].secret)
    );
    model.paste("private-login-token");
    assert!(!render(&model, 100, 32).0.contains("private-login-token"));
    assert!(!format!("{:?}", model.overlay).contains("private-login-token"));
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.requests[0], Request::Authenticate { credential: super::settings::Credential::Token { token, .. }, .. } if token.expose() == "private-login-token")
    );
}

#[test]
fn failed_login_preserves_the_current_account_and_allows_correcting_the_token() {
    let (_temp, mut model) = fixture();
    let email = model.account.as_ref().unwrap().email.clone();
    model.action(Action::Login);
    model.paste("incorrect-token");
    key(&mut model, KeyCode::Enter);
    let Request::Authenticate { id, .. } = model.requests.pop().unwrap() else {
        panic!("authentication")
    };
    model.handle_message(Message::Authenticated {
        id,
        result: Err("Token rejected".into()),
    });
    assert_eq!(model.account.as_ref().unwrap().email, email);
    assert!(model.signed_in);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some() && form.fields[0].editor.text == "incorrect-token")
    );
    ctrl(&mut model, 'u');
    model.paste("corrected-token");
    key(&mut model, KeyCode::Enter);
    assert!(matches!(
        model.requests.pop(),
        Some(Request::Authenticate { .. })
    ));
}

#[test]
fn successful_account_switch_clears_old_caches_and_ignores_late_account_responses() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Shares);
    flush_demo(&mut model);
    let old_account = model.account.clone().unwrap();
    let old_epoch = model.session_epoch;
    let authorized = auth_result(&model, "next@example.com");
    model.action(Action::Login);
    model.paste("next-account-token");
    key(&mut model, KeyCode::Enter);
    let Request::Authenticate { id, .. } = model.requests.pop().unwrap() else {
        panic!("authentication")
    };
    model.handle_message(Message::Authenticated {
        id,
        result: Ok(authorized),
    });
    assert_eq!(model.account.as_ref().unwrap().email, "next@example.com");
    assert!(model.entries.is_empty());
    assert!(model.shares.is_empty());
    assert!(model.selected.is_empty());
    assert!(model.transfers.is_empty());
    assert!(model.session_epoch > old_epoch);
    assert!(
        matches!(&model.requests[0], Request::UseSession { token: Some(token), .. } if token.expose() == "next-account-token")
    );
    model.handle_message(Message::Session {
        epoch: old_epoch,
        message: Box::new(Message::Account(Ok(old_account))),
    });
    assert_eq!(model.account.as_ref().unwrap().email, "next@example.com");
}

#[test]
fn account_switch_and_logout_are_blocked_while_transfers_are_active() {
    let (_temp, mut model) = fixture();
    queue_downloads(&mut model, 1);
    model.requests.clear();
    for action in [Action::Accounts, Action::Login, Action::SignOut] {
        model.action(action);
        assert!(model.overlay.is_none());
        assert!(model.requests.is_empty());
    }
    model.switch(Workspace::Settings);
    key(&mut model, KeyCode::Enter);
    assert_eq!(model.language(), super::i18n::Language::Chinese);
    assert!(matches!(
        model.requests.pop(),
        Some(Request::SaveLanguage(_))
    ));
}

#[test]
fn signed_out_sessions_clear_cached_data_and_remain_available_for_login() {
    let (_temp, mut model) = fixture();
    model.action(Action::SignOut);
    let Request::SignOut { id } = model.requests.pop().unwrap() else {
        panic!("sign out")
    };
    let settings = super::settings::SettingsView {
        language: model.language(),
        active_account: None,
        accounts: vec![],
    };
    model.handle_message(Message::SignedOut {
        id,
        result: Ok(settings),
    });
    assert!(!model.signed_in);
    assert!(model.account.is_none());
    assert!(model.entries.is_empty());
    assert_eq!(model.workspace, Workspace::Settings);
    assert!(matches!(
        model.requests.pop(),
        Some(Request::UseSession { token: None, .. })
    ));
    model.action(Action::Upload);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(form.kind, FormKind::Login))
    );
}

#[test]
fn language_switch_updates_settings_files_and_commands_without_translating_filenames() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Settings);
    key(&mut model, KeyCode::Enter);
    let (settings, _) = render(&model, 100, 32);
    assert!(settings.contains("界面语言"));
    assert!(settings.contains("切换账号"));
    assert!(!settings.contains("Language"));
    model.switch(Workspace::Files);
    let (files, _) = render(&model, 128, 36);
    assert!(files.contains("资料库"));
    assert!(files.contains("名称"));
    assert!(files.contains("大小"));
    assert!(files.contains("README.md"));
    ctrl(&mut model, 'p');
    model.paste("上传");
    assert!(render(&model, 100, 32).0.contains("选择要上传的本地文件"));
    key(&mut model, KeyCode::Enter);
    assert!(matches!(model.overlay, Some(Overlay::Picker(_))));
}

#[test]
fn command_palette_uses_ctrl_or_command_p_and_has_no_permanent_command_line() {
    let (_temp, mut model) = fixture();
    key(&mut model, KeyCode::Char('/'));
    key(&mut model, KeyCode::Char(':'));
    assert!(model.overlay.is_none());
    assert!(!render(&model, 128, 36).0.contains("/ for commands"));
    model.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::SUPER));
    assert!(matches!(model.overlay, Some(Overlay::Palette { .. })));
    ctrl(&mut model, 'p');
    assert!(model.overlay.is_none());
}

#[test]
fn path_picker_completes_paths_with_spaces_and_reviews_the_selected_file() {
    let (temp, mut model) = fixture();
    std::fs::write(temp.path().join("report one.txt"), "one").unwrap();
    std::fs::write(temp.path().join("report two.txt"), "two").unwrap();
    model.action(Action::Upload);
    flush_demo(&mut model);
    model.paste("rep");
    flush_demo(&mut model);
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.input.text.ends_with("report "))
    );
    key(&mut model, KeyCode::Down);
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(form.kind, FormKind::Upload) && form.fields[0].editor.text.ends_with("report two.txt"))
    );
    assert!(model.transfers.is_empty());
}

#[test]
fn matched_files_are_reviewed_before_upload_and_then_queued_with_explicit_paths() {
    let (temp, mut model) = fixture();
    for name in ["论文 a.pdf", "论文 b.pdf", "other.txt"] {
        std::fs::write(temp.path().join(name), "test").unwrap();
    }
    model.action(Action::Upload);
    model.paste("*.pdf");
    flush_demo(&mut model);
    ctrl(&mut model, 'a');
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(&form.kind, FormKind::UploadMany(files) if files.len() == 2))
    );
    assert!(model.transfers.is_empty());
    key(&mut model, KeyCode::Enter);
    assert_eq!(model.transfers.len(), 2);
    assert!(model.transfers.iter().all(|t| matches!(&t.spec, TransferSpec::Upload { remote, policy, .. } if remote.starts_with("repo:Research/论文 ") && policy == "uniquify")));
    assert_eq!(model.transfers[0].status, TransferStatus::Running);
    assert_eq!(model.transfers[1].status, TransferStatus::Queued);
}

#[test]
fn batch_upload_validation_does_not_partially_queue_a_batch_if_a_file_disappears() {
    let (temp, mut model) = fixture();
    for name in ["a.txt", "b.txt"] {
        std::fs::write(temp.path().join(name), "test").unwrap();
    }
    model.action(Action::Upload);
    model.paste("*.txt");
    flush_demo(&mut model);
    ctrl(&mut model, 'a');
    key(&mut model, KeyCode::Enter);
    std::fs::remove_file(temp.path().join("b.txt")).unwrap();
    key(&mut model, KeyCode::Enter);
    assert!(model.transfers.is_empty());
    assert!(matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some()));
}

#[test]
fn invalid_and_empty_path_matches_can_be_corrected_without_closing_the_picker() {
    let (temp, mut model) = fixture();
    std::fs::write(temp.path().join("available.txt"), "test").unwrap();
    model.action(Action::Upload);
    model.paste("missing-folder/file");
    flush_demo(&mut model);
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.error.is_some()));
    ctrl(&mut model, 'l');
    model.paste("no-match");
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.error.is_none() && p.entries.is_empty())
    );
    ctrl(&mut model, 'l');
    model.paste("available");
    flush_demo(&mut model);
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.entries.len() == 1));
}

#[test]
fn download_picker_browses_directories_then_queues_all_selected_files_at_the_chosen_destination() {
    let (temp, mut model) = fixture();
    let destination = temp.path().join("下载 资料");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(temp.path().join("not-a-folder.txt"), "data").unwrap();
    model.selected = ["/paper-draft.pdf", "/results.csv"]
        .into_iter()
        .map(str::to_string)
        .collect();
    model.action(Action::Download);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.directories_only() && p.entries.iter().all(|entry| entry.is_dir))
    );
    key(&mut model, KeyCode::Down);
    key(&mut model, KeyCode::Enter);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.cwd == destination && p.entries.len() == 1)
    );
    key(&mut model, KeyCode::Enter);
    assert!(model.transfers.is_empty());
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(&form.kind, FormKind::Download(remotes) if remotes.len() == 2) && std::path::Path::new(&form.fields[0].editor.text) == destination.as_path())
    );
    key(&mut model, KeyCode::Enter);
    assert_eq!(model.transfers.len(), 2);
    assert!(model.transfers.iter().all(|transfer| matches!(&transfer.spec, TransferSpec::Download { local, .. } if *local == destination)));
}

#[test]
fn download_directory_picker_completes_paths_and_can_choose_a_highlighted_folder() {
    let (temp, mut model) = fixture();
    let destination = temp.path().join("results folder");
    std::fs::create_dir(&destination).unwrap();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    model.paste("res");
    flush_demo(&mut model);
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.input.text.ends_with(std::path::MAIN_SEPARATOR) && p.entries.len() == 1)
    );
    ctrl(&mut model, 'd');
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if form.fields[0].editor.text.trim_end_matches(std::path::MAIN_SEPARATOR) == destination.display().to_string())
    );
    assert!(model.transfers.is_empty());
}

#[test]
fn cancelling_download_directory_selection_preserves_local_path_and_remote_selection() {
    let (_temp, mut model) = fixture();
    select_file(&mut model, "paper-draft.pdf");
    model.selected.insert("/paper-draft.pdf".into());
    let local = model.local.clone();
    let selected = model.selected.clone();
    model.action(Action::Download);
    flush_demo(&mut model);
    key(&mut model, KeyCode::Esc);
    assert!(model.overlay.is_none());
    assert!(model.transfers.is_empty());
    assert_eq!(model.local, local);
    assert_eq!(model.selected, selected);
    assert!(
        !model
            .requests
            .iter()
            .any(|request| matches!(request, Request::Transfer { .. }))
    );
}

#[test]
fn download_picker_rejects_files_and_recovers_to_the_current_directory() {
    let (temp, mut model) = fixture();
    std::fs::write(temp.path().join("file.txt"), "test").unwrap();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    model.paste("file.txt");
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.error.is_some() && p.entries.is_empty())
    );
    ctrl(&mut model, 'd');
    assert!(matches!(model.overlay, Some(Overlay::Picker(_))));
    ctrl(&mut model, 'l');
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    assert!(matches!(model.overlay, Some(Overlay::Form(_))));
    assert!(model.transfers.is_empty());
}

#[test]
fn download_picker_supports_chinese_narrow_layouts_and_demo_preview_without_starting_downloads() {
    let (_temp, mut model) = fixture();
    model.demo = true;
    model.settings.language = super::i18n::Language::Chinese;
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    flush_demo(&mut model);
    let (text, _) = render(&model, 60, 18);
    assert!(text.contains("选择保存目录"));
    assert!(text.contains("使用此目录"));
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Enter);
    assert!(matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some()));
    assert!(model.transfers.is_empty());
}

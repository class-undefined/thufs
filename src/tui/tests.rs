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
        Action, Focus, FormKind, Model, Overlay, Setting, SettingsCategory, TransferSpec,
        TransferStatus, Workspace, normalized_path, palette_matches,
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
    let mut model = Model::new(
        temp.path().to_path_buf(),
        temp.path().to_path_buf(),
        None,
        true,
    );
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
fn library_search_finds_nested_paths_and_downloads_the_selected_result() {
    let (_temp, mut model) = fixture();
    ctrl(&mut model, 'f');
    model.paste("DAtasets BNCH JSON");
    flush_demo(&mut model);
    assert_eq!(model.file_indices().len(), 1);
    assert_eq!(
        model.current_file().unwrap().path,
        "/datasets/benchmark.jsonl"
    );
    assert_eq!(model.entries.len(), 8);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Char('d'));
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.transfers[0].spec, TransferSpec::Download { remote, .. } if remote == "repo:Research/datasets/benchmark.jsonl")
    );
}

#[test]
fn library_search_reuses_one_index_while_queries_change_and_clears_to_the_current_directory() {
    let (_temp, mut model) = fixture();
    ctrl(&mut model, 'f');
    model.paste("bench");
    model.paste("mark");
    assert_eq!(
        model
            .requests
            .iter()
            .filter(|r| matches!(r, Request::LibraryIndex { .. }))
            .count(),
        1
    );
    flush_demo(&mut model);
    assert_eq!(
        model.current_file().unwrap().path,
        "/datasets/benchmark.jsonl"
    );
    ctrl(&mut model, 'u');
    model.paste("中文");
    assert!(model.requests.is_empty());
    key(&mut model, KeyCode::Esc);
    assert!(!model.searching_library());
    assert_eq!(model.file_entries(), model.entries.as_slice());
    assert_eq!(model.file_indices().len(), 8);
}

#[test]
fn switching_libraries_cancels_search_and_ignores_late_results() {
    use std::sync::atomic::Ordering;
    let (_temp, mut model) = fixture();
    ctrl(&mut model, 'f');
    model.paste("benchmark");
    let (id, cancel) = model
        .requests
        .iter()
        .find_map(|r| match r {
            Request::LibraryIndex { id, cancel, .. } => Some((*id, cancel.clone())),
            _ => None,
        })
        .unwrap();
    model.library_cursor = 1;
    model.open_library();
    assert!(cancel.load(Ordering::Relaxed));
    flush_demo(&mut model);
    model.handle_message(Message::LibraryIndex {
        id,
        result: Ok(demo::entries("/datasets")),
    });
    assert_eq!(model.location.as_ref().unwrap().repo, "Coursework");
    assert!(!model.searching_library());
    assert_eq!(model.file_indices().len(), 8);
}

#[test]
fn library_search_failures_are_visible_and_refresh_retries_the_full_library() {
    let (_temp, mut model) = fixture();
    ctrl(&mut model, 'f');
    model.paste("benchmark");
    let Request::LibraryIndex { id, .. } = model.requests.pop().unwrap() else {
        panic!("index request");
    };
    model.handle_message(Message::LibraryIndex {
        id,
        result: Err("Cannot read nested folder".into()),
    });
    assert_eq!(model.files_error(), Some("Cannot read nested folder"));
    assert!(
        render(&model, 100, 32)
            .0
            .contains("Cannot read nested folder")
    );
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Char('r'));
    assert!(
        model
            .requests
            .iter()
            .any(|r| matches!(r, Request::LibraryIndex { id: retry, .. } if *retry != id))
    );
    flush_demo(&mut model);
    assert!(model.files_error().is_none());
    assert_eq!(
        model.current_file().unwrap().path,
        "/datasets/benchmark.jsonl"
    );
}

#[test]
fn search_inputs_are_visible_above_file_and_share_lists_and_can_be_clicked() {
    for language in [
        super::i18n::Language::English,
        super::i18n::Language::Chinese,
    ] {
        for workspace in [Workspace::Files, Workspace::Shares] {
            for (width, height) in [(60, 18), (128, 36)] {
                let (_temp, mut model) = fixture();
                model.settings.language = language;
                model.switch(workspace);
                flush_demo(&mut model);
                let (text, hits) = render(&model, width, height);
                assert!(hits.search.width > 10);
                assert!(hits.search.y < hits.files.y);
                assert!(hits.search.right() <= width);
                assert!(
                    text.contains(
                        language
                            .tr(if workspace == Workspace::Files {
                                "Search library"
                            } else {
                                "Search shares"
                            })
                            .as_ref()
                    )
                );
                click(&mut model, &hits, hits.search, &mut None);
                assert!(model.filtering);
                assert!(model.overlay.is_none());
            }
        }
    }
}

#[test]
fn share_search_matches_names_libraries_urls_and_codes_with_fuzzy_multiword_queries() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Shares);
    flush_demo(&mut model);
    model.shares[0].path = Some("/论文/report-final.pdf".into());
    model.shares[0].repo = Some("Research".into());
    model.shares[0].token = Some("AbC123_XYZ".into());
    model.shares[0].url = "https://cloud.tsinghua.edu.cn/f/AbC123_XYZ/".into();
    let mut other = model.shares[0].clone();
    other.path = Some("/other.txt".into());
    other.repo = Some("Personal".into());
    other.token = Some("different".into());
    other.url = "https://example.invalid/other/".into();
    model.shares.push(other);
    for query in ["论文", "RPTFNL", "research pdf", "ABC123", "tsinghua XYZ"] {
        model.filter = Editor::new(query);
        assert_eq!(model.share_indices(), vec![0], "query {query}");
    }
}

#[test]
fn shared_file_downloads_work_from_the_list_and_details_without_an_open_library() {
    for from_details in [false, true] {
        let (_temp, mut model) = fixture();
        model.switch(Workspace::Shares);
        flush_demo(&mut model);
        model.location = None;
        model.shares[0].url = "https://cloud.tsinghua.edu.cn/f/AbC123_XYZ/".into();
        if from_details {
            key(&mut model, KeyCode::Enter);
        }
        key(&mut model, KeyCode::Char('d'));
        assert!(matches!(model.overlay, Some(Overlay::Picker(_))));
        flush_demo(&mut model);
        key(&mut model, KeyCode::Enter);
        key(&mut model, KeyCode::Enter);
        assert!(
            matches!(&model.transfers[0].spec, TransferSpec::Download { remote, .. } if remote == "https://cloud.tsinghua.edu.cn/f/AbC123_XYZ/")
        );
    }
}

#[test]
fn a_selected_library_directory_can_be_downloaded_and_keeps_its_name() {
    let (_temp, mut model) = fixture();
    model.cursor = model
        .file_indices()
        .iter()
        .position(|&i| model.entries[i].name == "datasets")
        .unwrap();
    key(&mut model, KeyCode::Char('d'));
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.transfers[0].spec, TransferSpec::Download { remote, .. } if remote == "repo:Research/datasets/")
    );
    assert_eq!(model.transfers[0].spec.name(), "datasets");
}

#[test]
fn selecting_a_directory_and_its_search_results_does_not_download_children_twice() {
    let (_temp, mut model) = fixture();
    ctrl(&mut model, 'f');
    model.paste("datasets");
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Char('a'));
    assert_eq!(model.selected.len(), 3);
    key(&mut model, KeyCode::Char('d'));
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Enter);
    assert_eq!(model.transfers.len(), 1);
    assert!(
        matches!(&model.transfers[0].spec, TransferSpec::Download { remote, .. } if remote == "repo:Research/datasets/")
    );
}

#[test]
fn shared_directory_links_can_be_queued_for_recursive_download() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Shares);
    flush_demo(&mut model);
    model.shares[0].is_dir = None;
    model.shares[0].url = "https://cloud.tsinghua.edu.cn/d/folder-code/".into();
    key(&mut model, KeyCode::Char('d'));
    assert!(matches!(model.overlay, Some(Overlay::Picker(_))));
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.transfers[0].spec, TransferSpec::Download { remote, .. } if remote == "https://cloud.tsinghua.edu.cn/d/folder-code/")
    );
}

#[test]
fn refreshing_a_directory_invalidates_the_cached_library_index() {
    let (_temp, mut model) = fixture();
    ctrl(&mut model, 'f');
    model.paste("benchmark");
    flush_demo(&mut model);
    key(&mut model, KeyCode::Esc);
    key(&mut model, KeyCode::Char('r'));
    flush_demo(&mut model);
    ctrl(&mut model, 'f');
    model.paste("benchmark");
    assert!(
        model
            .requests
            .iter()
            .any(|r| matches!(r, Request::LibraryIndex { .. }))
    );
}

#[test]
fn clicking_a_filtered_share_result_keeps_the_query_and_downloads_the_correct_link() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Shares);
    flush_demo(&mut model);
    let mut target = model.shares[0].clone();
    target.path = Some("/report-final.pdf".into());
    target.url = "https://cloud.tsinghua.edu.cn/f/target123/".into();
    model.shares.push(target);
    ctrl(&mut model, 'f');
    model.paste("rptfnl");
    assert_eq!(model.share_indices(), vec![1]);
    let (_, hits) = render(&model, 100, 32);
    let row = ratatui::layout::Rect::new(
        hits.files.x,
        hits.files.y + hits.file_header,
        hits.files.width,
        1,
    );
    click(&mut model, &hits, row, &mut None);
    assert!(!model.filtering);
    assert_eq!(model.filter.text, "rptfnl");
    key(&mut model, KeyCode::Char('d'));
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.transfers[0].spec, TransferSpec::Download { remote, .. } if remote == "https://cloud.tsinghua.edu.cn/f/target123/")
    );
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
fn select_all_selects_visible_files_and_folders_and_preserves_hidden_selections() {
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
    assert_eq!(model.selected.len(), 8);
    assert!(
        model
            .entries
            .iter()
            .filter(|f| f.kind == "dir")
            .all(|f| model.selected.contains(&f.path))
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
            Workspace::About,
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
fn a_single_library_click_immediately_loads_that_library_at_wide_and_narrow_widths() {
    for (width, height) in [(128, 36), (60, 18)] {
        let (_temp, mut model) = fixture();
        model.selected.insert("/paper-draft.pdf".into());
        if width < 90 {
            key(&mut model, KeyCode::Tab);
        }
        let (_, hits) = render(&model, width, height);
        let row = ratatui::layout::Rect::new(
            hits.libraries.x,
            hits.libraries.y + 1,
            hits.libraries.width,
            1,
        );
        click(&mut model, &hits, row, &mut None);
        assert_eq!(model.location.as_ref().unwrap().repo, "Coursework");
        assert_eq!(model.focus, Focus::Files);
        assert!(model.selected.is_empty());
        flush_demo(&mut model);
        assert!(!model.loading);
        assert!(!model.entries.is_empty());
    }
}

#[test]
fn picker_paths_are_prefilled_and_stay_in_the_input_when_browsing_directories() {
    for action in [Action::Upload, Action::Download] {
        let (temp, mut model) = fixture();
        let child = temp.path().join("子目录 with spaces");
        std::fs::create_dir(&child).unwrap();
        select_file(&mut model, "paper-draft.pdf");
        model.action(action);
        flush_demo(&mut model);
        let Some(Overlay::Picker(picker)) = &mut model.overlay else {
            panic!("path picker")
        };
        assert_eq!(std::path::Path::new(&picker.input.text), temp.path());
        assert!(picker.input.text.ends_with(std::path::MAIN_SEPARATOR));
        picker.selected = picker
            .entries
            .iter()
            .position(|entry| entry.path == child)
            .unwrap();

        key(&mut model, KeyCode::Enter);
        flush_demo(&mut model);
        assert!(
            matches!(&model.overlay, Some(Overlay::Picker(p)) if p.cwd == child && std::path::Path::new(&p.input.text) == child)
        );

        model.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        flush_demo(&mut model);
        assert!(
            matches!(&model.overlay, Some(Overlay::Picker(p)) if p.cwd == temp.path() && std::path::Path::new(&p.input.text) == temp.path())
        );
        let (_, hits) = render(&model, 100, 32);
        let input = hits.dialog.picker.unwrap().input;
        let (text, _) = render(&model, 100, 32);
        for line in text.lines().take(input.y as usize) {
            assert!(!line.contains(&temp.path().display().to_string()));
        }
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
            user_agent: model.settings.user_agent.clone(),
            check_updates: model.settings.check_updates,
            proxy: model.settings.proxy.clone(),
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
    let mut model = Model::new(
        temp.path().to_path_buf(),
        temp.path().to_path_buf(),
        None,
        false,
    );
    model.configure_settings(
        super::settings::SettingsView {
            language: super::i18n::Language::English,
            user_agent: None,
            check_updates: true,
            proxy: crate::network::ProxyConfig::Auto,
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
        user_agent: None,
        check_updates: true,
        proxy: crate::network::ProxyConfig::Auto,
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
fn about_is_searchable_in_both_languages_without_login_and_only_displays_project_information() {
    for (language, query, author_label, repository_label) in [
        (
            super::i18n::Language::English,
            "about",
            "Author",
            "Repository",
        ),
        (super::i18n::Language::Chinese, "关于", "作者", "仓库"),
    ] {
        let (_temp, mut model) = fixture();
        model.settings.language = language;
        model.signed_in = false;
        model.requests.clear();
        ctrl(&mut model, 'p');
        model.paste(query);
        key(&mut model, KeyCode::Enter);
        assert_eq!(model.workspace, Workspace::About);
        assert!(model.overlay.is_none());

        for (width, height) in [(60, 18), (80, 24), (100, 32)] {
            let (text, _) = render(&model, width, height);
            assert!(text.contains(author_label));
            assert!(text.contains(repository_label));
            assert!(text.contains("野漫横江 (@class-undefined)"));
            assert!(text.contains(env!("CARGO_PKG_VERSION")));
            assert!(text.contains("https://github.com/class-undefined/thufs"));
            assert!(!text.contains("Copy repository"));
            assert!(!text.contains("复制仓库"));
        }
        assert!(model.requests.is_empty());

        key(&mut model, KeyCode::Char('y'));
        assert!(model.requests.is_empty());
        key(&mut model, KeyCode::Enter);
        assert!(model.overlay.is_none());
        assert_eq!(model.workspace, Workspace::About);
    }
}

#[test]
fn about_and_settings_are_peer_tabs_accessible_by_keyboard_and_single_click() {
    for (width, height) in [(60, 18), (128, 36)] {
        let (_temp, mut model) = fixture();
        key(&mut model, KeyCode::Char('4'));
        let (text, hits) = render(&model, width, height);
        assert!(text.contains("About"));
        let about = hits
            .tabs
            .iter()
            .find(|(_, page)| *page == Workspace::About)
            .unwrap()
            .0;
        let settings = hits
            .tabs
            .iter()
            .find(|(_, page)| *page == Workspace::Settings)
            .unwrap()
            .0;
        assert_eq!(about.y, settings.y);
        assert!(about.right() <= width);
        let mut last_click = None;
        click(&mut model, &hits, about, &mut last_click);
        assert_eq!(model.workspace, Workspace::About);
        assert!(model.overlay.is_none());
        click(&mut model, &hits, settings, &mut last_click);
        assert_eq!(model.workspace, Workspace::Settings);
        assert!(model.overlay.is_none());

        key(&mut model, KeyCode::Char('5'));
        assert_eq!(model.workspace, Workspace::About);
        key(&mut model, KeyCode::Esc);
        assert!(model.overlay.is_none());
        assert!(!model.should_quit);
        assert_eq!(model.workspace, Workspace::Files);
    }
}

fn update_result(available: bool) -> crate::app::update_service::UpdateCheck {
    crate::app::update_service::UpdateCheck {
        current_version: env!("CARGO_PKG_VERSION").into(),
        latest_version: Some(
            if available {
                "999.0.0"
            } else {
                env!("CARGO_PKG_VERSION")
            }
            .into(),
        ),
        update_available: available,
        release_url: "https://github.com/class-undefined/thufs/releases/tag/v999.0.0".into(),
        checked_at: 100,
        cached: false,
    }
}

#[test]
fn automatic_update_checks_are_nonblocking_deduplicated_and_can_be_disabled() {
    use crate::app::update_service::CheckMode;
    let (_temp, mut model) = fixture();
    model.check_for_updates(CheckMode::Cached);
    model.check_for_updates(CheckMode::Cached);
    assert_eq!(model.requests.len(), 1);
    assert!(model.overlay.is_none());
    let Request::CheckUpdates { id, .. } = model.requests.pop().unwrap() else {
        panic!("update check")
    };
    model.handle_message(Message::UpdateChecked {
        id,
        result: Err("offline".into()),
    });
    assert!(model.notice.is_none());
    assert!(matches!(model.update, super::model::UpdateState::Failed(_)));
    model.check_for_updates(CheckMode::Cached);
    let Request::CheckUpdates { id, .. } = model.requests.pop().unwrap() else {
        panic!("update check")
    };
    model.handle_message(Message::UpdateChecksSaved {
        enabled: false,
        result: Ok(()),
    });
    model.handle_message(Message::UpdateChecked {
        id,
        result: Ok(update_result(true)),
    });
    assert!(matches!(
        model.update,
        super::model::UpdateState::NotChecked
    ));
    model.check_for_updates(CheckMode::Cached);
    assert!(model.requests.is_empty());
    model.switch(Workspace::About);
    key(&mut model, KeyCode::Char('u'));
    assert!(matches!(
        &model.requests[0],
        Request::CheckUpdates {
            mode: CheckMode::Refresh,
            ..
        }
    ));
}

#[test]
fn about_update_status_and_clickable_check_are_visible_in_both_languages_at_minimum_size() {
    for (language, check_label, status) in [
        (
            super::i18n::Language::English,
            "Check for updates",
            "available",
        ),
        (super::i18n::Language::Chinese, "检查更新", "发现"),
    ] {
        for (width, height) in [(60, 18), (80, 24), (128, 36)] {
            let (_temp, mut model) = fixture();
            model.settings.language = language;
            model.signed_in = false;
            model.settings.check_updates = false;
            model.switch(Workspace::About);
            let (text, hits) = render(&model, width, height);
            assert!(text.contains(check_label));
            assert!(hits.check_updates.right() <= width);
            assert!(hits.check_updates.bottom() <= height - 3);
            let mut last = None;
            click(&mut model, &hits, hits.check_updates, &mut last);
            let Request::CheckUpdates { id, .. } = model.requests.pop().unwrap() else {
                panic!("update check")
            };
            model.handle_message(Message::UpdateChecked {
                id,
                result: Ok(update_result(true)),
            });
            let (text, _) = render(&model, width, height);
            assert!(text.contains("999.0.0"));
            assert!(text.contains(status));
            assert!(text.contains(env!("CARGO_PKG_REPOSITORY")));
            key(&mut model, KeyCode::Enter);
            assert!(
                matches!(&model.overlay, Some(Overlay::Details { lines, .. }) if lines.iter().any(|line| line == "cargo install thufs --locked --force"))
            );
            assert!(model.requests.is_empty());
        }
    }
}

#[test]
fn update_failures_are_not_reported_as_up_to_date_and_can_be_retried_from_about() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::About);
    key(&mut model, KeyCode::Char('u'));
    let Request::CheckUpdates { id, .. } = model.requests.pop().unwrap() else {
        panic!("update check")
    };
    model.handle_message(Message::UpdateChecked {
        id,
        result: Err("rate limited".into()),
    });
    let text = render(&model, 60, 18).0;
    assert!(text.contains("Check unavailable"));
    assert!(!text.contains("No newer release"));
    key(&mut model, KeyCode::Char('i'));
    assert!(
        matches!(&model.overlay, Some(Overlay::Details { lines, .. }) if lines == &["rate limited"])
    );
    key(&mut model, KeyCode::Esc);
    key(&mut model, KeyCode::Char('r'));
    assert!(matches!(
        model.requests.pop(),
        Some(Request::CheckUpdates { .. })
    ));
}

#[test]
fn public_update_results_remain_valid_across_an_account_switch() {
    let (_temp, mut model) = fixture();
    model.action(Action::CheckUpdates);
    let Request::CheckUpdates { id, .. } = model.requests.pop().unwrap() else {
        panic!("update check")
    };
    let authorized = auth_result(&model, "next@example.com");
    model.action(Action::Login);
    model.paste("next-account-token");
    key(&mut model, KeyCode::Enter);
    let Request::Authenticate { id: auth_id, .. } = model.requests.pop().unwrap() else {
        panic!("authentication")
    };
    model.handle_message(Message::Authenticated {
        id: auth_id,
        result: Ok(authorized),
    });
    model.handle_message(Message::UpdateChecked {
        id,
        result: Ok(update_result(true)),
    });
    assert!(
        matches!(&model.update, super::model::UpdateState::Checked(result) if result.update_available)
    );
    assert_eq!(model.account.as_ref().unwrap().email, "next@example.com");
}

#[test]
fn automatic_update_setting_is_accessible_and_saves_before_being_applied() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Settings);
    key(&mut model, KeyCode::Tab);
    key(&mut model, KeyCode::Tab);
    assert!(render(&model, 60, 18).0.contains("Automatic update checks"));
    key(&mut model, KeyCode::Enter);
    assert!(model.settings.check_updates);
    assert!(matches!(
        model.requests.pop(),
        Some(Request::SaveUpdateChecks(false))
    ));
    model.handle_message(Message::UpdateChecksSaved {
        enabled: false,
        result: Err("save failed".into()),
    });
    assert!(model.settings.check_updates);
    key(&mut model, KeyCode::Enter);
    flush_demo(&mut model);
    assert!(!model.settings.check_updates);
}

#[test]
fn categorized_settings_navigation_keeps_selection_within_each_group_and_routes_account_actions() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Settings);
    assert_eq!(model.settings_category, SettingsCategory::Interface);
    key(&mut model, KeyCode::End);
    assert_eq!(model.settings_cursor, 0);
    key(&mut model, KeyCode::Tab);
    assert_eq!(model.settings_category, SettingsCategory::Network);
    key(&mut model, KeyCode::End);
    assert_eq!(
        model.settings_category.items()[model.settings_cursor],
        Setting::UserAgent
    );
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(form.kind, FormKind::UserAgent))
    );
    key(&mut model, KeyCode::Esc);
    model.switch(Workspace::Files);
    model.switch(Workspace::Settings);
    assert_eq!(model.settings_category, SettingsCategory::Network);
    assert_eq!(model.settings_cursor, 1);
    key(&mut model, KeyCode::Tab);
    assert_eq!(model.settings_category, SettingsCategory::Updates);
    assert_eq!(model.settings_cursor, 0);
    key(&mut model, KeyCode::Down);
    key(&mut model, KeyCode::Enter);
    assert!(matches!(
        model.requests.pop(),
        Some(Request::CheckUpdates { .. })
    ));
    key(&mut model, KeyCode::Tab);
    assert_eq!(model.settings_category, SettingsCategory::Accounts);
    key(&mut model, KeyCode::Enter);
    assert!(matches!(model.overlay, Some(Overlay::Accounts { .. })));
    key(&mut model, KeyCode::Esc);
    key(&mut model, KeyCode::Down);
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(form.kind, FormKind::Login))
    );
    key(&mut model, KeyCode::Esc);
    key(&mut model, KeyCode::Tab);
    assert_eq!(model.settings_category, SettingsCategory::Interface);
    key(&mut model, KeyCode::BackTab);
    assert_eq!(model.settings_category, SettingsCategory::Accounts);
    key(&mut model, KeyCode::End);
    assert_eq!(model.settings_cursor, 2);
    key(&mut model, KeyCode::Enter);
    assert!(matches!(
        model.requests.pop(),
        Some(Request::SignOut { .. })
    ));
}

#[test]
fn settings_categories_and_options_are_mouse_accessible_in_both_languages_and_narrow_layouts() {
    for language in [
        super::i18n::Language::English,
        super::i18n::Language::Chinese,
    ] {
        for (width, height) in [(60, 18), (80, 24), (128, 36)] {
            let (_temp, mut model) = fixture();
            model.settings.language = language;
            model.switch(Workspace::Settings);
            let (_, hits) = render(&model, width, height);
            assert_eq!(hits.settings_categories.len(), 4);
            for (rect, _) in &hits.settings_categories {
                assert!(rect.right() < width);
            }
            let mut last = None;
            let network = hits
                .settings_categories
                .iter()
                .find(|(_, category)| *category == SettingsCategory::Network)
                .unwrap()
                .0;
            click(&mut model, &hits, network, &mut last);
            assert_eq!(model.settings_category, SettingsCategory::Network);
            let (text, hits) = render(&model, width, height);
            assert!(text.contains("User-Agent"));
            assert!(!text.contains("demo@example.invalid"));
            let row = ratatui::layout::Rect::new(
                hits.files.x,
                hits.files.y + hits.row_height,
                hits.files.width,
                1,
            );
            click(&mut model, &hits, row, &mut last);
            click(&mut model, &hits, row, &mut last);
            assert!(
                matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(form.kind, FormKind::UserAgent))
            );
            key(&mut model, KeyCode::Esc);
            let (_, hits) = render(&model, width, height);
            let accounts = hits
                .settings_categories
                .iter()
                .find(|(_, category)| *category == SettingsCategory::Accounts)
                .unwrap()
                .0;
            click(&mut model, &hits, accounts, &mut last);
            assert_eq!(model.settings_category, SettingsCategory::Accounts);
            let (_, hits) = render(&model, width, height);
            assert_eq!(hits.files.height, 3 * hits.row_height);
            // Empty space below the account options must never activate Sign out.
            if height > 18 {
                let blank = ratatui::layout::Rect::new(
                    hits.files.x,
                    hits.files.bottom(),
                    hits.files.width,
                    1,
                );
                click(&mut model, &hits, blank, &mut last);
                click(&mut model, &hits, blank, &mut last);
                assert!(model.overlay.is_none());
                assert!(model.requests.is_empty());
                assert!(model.signed_in);
            }
        }
    }
}

#[test]
fn custom_user_agent_can_be_saved_corrected_and_reset_from_settings_without_login() {
    let (_temp, mut model) = fixture();
    model.signed_in = false;
    model.switch(Workspace::Settings);
    key(&mut model, KeyCode::Tab);
    for (width, height) in [(60, 18), (80, 24), (100, 32)] {
        assert!(
            render(&model, width, height)
                .0
                .contains("User-Agent    thufs/")
        );
    }
    key(&mut model, KeyCode::Down);
    assert!(render(&model, 60, 18).0.contains("User-Agent"));
    key(&mut model, KeyCode::Enter);
    assert!(
        render(&model, 100, 32)
            .0
            .contains(crate::config::DEFAULT_USER_AGENT)
    );
    model.paste("中文 UA");
    key(&mut model, KeyCode::Enter);
    assert!(matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some()));
    assert!(model.requests.is_empty());
    ctrl(&mut model, 'u');
    model.paste("ExampleClient/2.0");
    key(&mut model, KeyCode::Enter);
    let Request::SaveUserAgent { id, value } = model.requests.pop().unwrap() else {
        panic!("save user agent");
    };
    assert_eq!(value, "ExampleClient/2.0");
    model.handle_message(Message::UserAgentSaved {
        id,
        result: Err("Cannot save settings".into()),
    });
    assert!(model.settings.user_agent.is_none());
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some() && form.fields[0].editor.text == "ExampleClient/2.0")
    );
    key(&mut model, KeyCode::Enter);
    flush_demo(&mut model);
    assert_eq!(
        model.settings.user_agent.as_deref(),
        Some("ExampleClient/2.0")
    );
    assert!(model.overlay.is_none());
    assert!(render(&model, 100, 32).0.contains("ExampleClient/2.0"));
    key(&mut model, KeyCode::Enter);
    ctrl(&mut model, 'u');
    key(&mut model, KeyCode::Enter);
    flush_demo(&mut model);
    assert!(model.settings.user_agent.is_none());
    assert!(model.overlay.is_none());
    assert!(
        render(&model, 100, 32)
            .0
            .contains(crate::config::DEFAULT_USER_AGENT)
    );
    assert!(model.transfers.is_empty());
}

#[test]
fn proxy_settings_support_auto_manual_direct_and_hide_proxy_credentials() {
    use crate::network::ProxyConfig;
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Settings);
    key(&mut model, KeyCode::Tab);
    key(&mut model, KeyCode::Enter);
    assert!(render(&model, 60, 18).0.contains("Proxy mode"));
    key(&mut model, KeyCode::Right);
    key(&mut model, KeyCode::Tab);
    model.paste("file:///invalid");
    key(&mut model, KeyCode::Enter);
    assert!(matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some()));
    assert_eq!(model.settings.proxy, ProxyConfig::Auto);
    ctrl(&mut model, 'u');
    model.paste("http://private-user:private-password@127.0.0.1:7890");
    assert!(!render(&model, 100, 32).0.contains("private-password"));
    assert!(!format!("{:?}", model.overlay).contains("private-password"));
    key(&mut model, KeyCode::Enter);
    let Request::SaveProxy { id, proxy } = model.requests.pop().unwrap() else {
        panic!("save proxy")
    };
    model.handle_message(Message::ProxySaved {
        id,
        result: Err("Cannot save settings".into()),
    });
    assert_eq!(model.settings.proxy, ProxyConfig::Auto);
    assert!(matches!(&model.overlay, Some(Overlay::Form(form)) if form.error.is_some()));
    key(&mut model, KeyCode::Enter);
    flush_demo(&mut model);
    assert_eq!(model.settings.proxy, proxy);
    assert!(!render(&model, 100, 32).0.contains("private-password"));
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Right);
    key(&mut model, KeyCode::Enter);
    flush_demo(&mut model);
    assert_eq!(model.settings.proxy, ProxyConfig::Direct);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Right);
    key(&mut model, KeyCode::Enter);
    flush_demo(&mut model);
    assert_eq!(model.settings.proxy, ProxyConfig::Auto);
}

#[test]
fn file_and_folder_inspectors_and_file_details_show_modification_dates() {
    for language in [
        super::i18n::Language::English,
        super::i18n::Language::Chinese,
    ] {
        let (_temp, mut model) = fixture();
        model.settings.language = language;
        for name in ["paper-draft.pdf", "datasets"] {
            select_file(&mut model, name);
            let (text, _) = render(&model, 128, 22);
            assert!(text.contains("2026-11-20 16:30:00"));
            assert!(text.contains("+08:00"));
            assert!(
                text.contains(if language == super::i18n::Language::Chinese {
                    "修改时间"
                } else {
                    "MODIFIED"
                })
            );
        }
        select_file(&mut model, "paper-draft.pdf");
        key(&mut model, KeyCode::Enter);
        assert!(
            render(&model, 60, 18)
                .0
                .contains("2026-11-20 16:30:00 +08:00")
        );
        assert!(
            !model
                .requests
                .iter()
                .any(|request| matches!(request, Request::EntryDate { .. }))
        );
    }
}

#[test]
fn missing_dates_are_fetched_on_demand_and_late_responses_do_not_replace_other_file_details() {
    let (_temp, mut model) = fixture();
    model
        .entries
        .iter_mut()
        .find(|file| file.name == "paper-draft.pdf")
        .unwrap()
        .updated_at = None;
    select_file(&mut model, "paper-draft.pdf");
    key(&mut model, KeyCode::Enter);
    let Request::EntryDate { id, location, .. } = model.requests.pop().unwrap() else {
        panic!("entry date")
    };
    let date = "2024-01-01 08:00:00 +08:00";
    model.handle_message(Message::EntryDate {
        id,
        location,
        result: Ok(Some(date.into())),
    });
    assert!(render(&model, 100, 32).0.contains(date));
    key(&mut model, KeyCode::Esc);
    key(&mut model, KeyCode::Enter);
    assert!(model.requests.is_empty());
    key(&mut model, KeyCode::Esc);
    model
        .entries
        .iter_mut()
        .find(|file| file.name == "paper-draft.pdf")
        .unwrap()
        .updated_at = None;
    key(&mut model, KeyCode::Enter);
    let Request::EntryDate { id, location, .. } = model.requests.pop().unwrap() else {
        panic!("entry date")
    };
    key(&mut model, KeyCode::Esc);
    select_file(&mut model, "README.md");
    key(&mut model, KeyCode::Enter);
    model.handle_message(Message::EntryDate {
        id,
        location,
        result: Ok(Some(date.into())),
    });
    let text = render(&model, 100, 32).0;
    assert!(text.contains("2026-11-20 16:30:00 +08:00"));
    assert!(!text.contains(date));
}

#[test]
fn language_switch_updates_settings_files_and_commands_without_translating_filenames() {
    let (_temp, mut model) = fixture();
    model.switch(Workspace::Settings);
    key(&mut model, KeyCode::Enter);
    let (settings, _) = render(&model, 100, 32);
    assert!(settings.contains("界面语言"));
    assert!(settings.contains("账号"));
    assert!(!settings.contains("Language"));
    key(&mut model, KeyCode::BackTab);
    assert!(render(&model, 100, 32).0.contains("切换账号"));
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
fn upload_and_download_pickers_remember_separate_directories() {
    let (temp, mut model) = fixture();
    let uploads = temp.path().join("Uploads");
    let downloads = temp.path().join("Downloads");
    let chosen = downloads.join("saved files");
    std::fs::create_dir(&uploads).unwrap();
    std::fs::create_dir_all(&chosen).unwrap();
    model.local = uploads.clone();
    model.download_directory = downloads.clone();

    model.action(Action::Upload);
    flush_demo(&mut model);
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.cwd == uploads));
    key(&mut model, KeyCode::Esc);

    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    flush_demo(&mut model);
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.cwd == downloads));
    model.paste("saved files");
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    key(&mut model, KeyCode::Enter);

    assert_eq!(model.local, uploads);
    assert_eq!(model.download_directory, chosen);
    assert!(
        matches!(&model.transfers[0].spec, TransferSpec::Download { local, .. } if *local == chosen)
    );
    model.requests.clear();

    model.action(Action::Download);
    flush_demo(&mut model);
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.cwd == chosen));
    key(&mut model, KeyCode::Esc);
    model.action(Action::Upload);
    flush_demo(&mut model);
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.cwd == uploads));
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
fn download_directory_picker_tab_completes_children_instead_of_the_current_directory_action() {
    let (temp, mut model) = fixture();
    let first = temp.path().join("报告 资料 a");
    let child = first.join("子目录");
    std::fs::create_dir_all(&child).unwrap();
    std::fs::create_dir(temp.path().join("报告 资料 b")).unwrap();
    std::fs::write(temp.path().join("unrelated.txt"), "data").unwrap();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    flush_demo(&mut model);

    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.input.text == temp.path().join("报告 资料 ").display().to_string())
    );
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.input.text.ends_with(std::path::MAIN_SEPARATOR) && std::path::Path::new(&p.input.text) == first)
    );
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if std::path::Path::new(&p.input.text) == child)
    );
    assert!(model.transfers.is_empty());
}

#[test]
fn download_directory_picker_applies_tab_after_pending_matches_arrive() {
    let (temp, mut model) = fixture();
    let target = temp.path().join("下载 资料");
    std::fs::create_dir(&target).unwrap();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    model.paste("下载");
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if !p.loading && p.input.text.ends_with(std::path::MAIN_SEPARATOR) && std::path::Path::new(&p.input.text) == target)
    );

    // Editing again cancels a queued completion for the previous input.
    ctrl(&mut model, 'l');
    model.paste(&temp.path().join("下").display().to_string());
    key(&mut model, KeyCode::Tab);
    model.paste("missing");
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if p.input.text.ends_with("下missing") && p.entries.is_empty())
    );
}

#[test]
fn download_confirmation_tab_completes_directories_and_preserves_download_options() {
    let (temp, mut model) = fixture();
    let first = temp.path().join("保存 资料 a");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(temp.path().join("保存 资料 b")).unwrap();
    std::fs::write(temp.path().join("保存 other.txt"), "data").unwrap();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    ctrl(&mut model, 'o');
    ctrl(&mut model, 'u');
    model.paste(&temp.path().join("保存").display().to_string());
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(f)) if f.active == 0 && f.fields[0].editor.text == temp.path().join("保存 资料 ").display().to_string())
    );
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(f)) if f.active == 0 && f.fields[0].editor.text.ends_with(std::path::MAIN_SEPARATOR) && std::path::Path::new(&f.fields[0].editor.text) == first && f.policy() == "overwrite" && matches!(&f.kind, FormKind::Download(remotes) if remotes == &["repo:Research/paper-draft.pdf"]))
    );
    assert!(model.transfers.is_empty());
    assert!(render(&model, 100, 32).0.contains("tab complete"));
}

#[test]
fn local_path_completion_ignores_stale_results_after_edits_navigation_and_cancellation() {
    let (temp, mut model) = upload_form();
    ctrl(&mut model, 'u');
    model.paste(&temp.path().join("bet").display().to_string());
    key(&mut model, KeyCode::Tab);
    let stale = demo::respond(model.requests.pop().unwrap());
    model.paste("ter");
    model.handle_message(stale);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(f)) if f.fields[0].editor.text.ends_with("better"))
    );

    ctrl(&mut model, 'u');
    model.paste(&temp.path().join("bet").display().to_string());
    key(&mut model, KeyCode::Tab);
    key(&mut model, KeyCode::Down);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(f)) if f.active == 1 && f.fields[0].editor.text.ends_with("bet"))
    );
    key(&mut model, KeyCode::BackTab);
    key(&mut model, KeyCode::Tab);
    key(&mut model, KeyCode::Esc);
    flush_demo(&mut model);
    assert!(model.overlay.is_none());
    assert!(model.transfers.is_empty());
}

#[test]
fn upload_confirmation_tab_completes_the_local_file_and_keeps_field_navigation() {
    let (temp, mut model) = upload_form();
    ctrl(&mut model, 'u');
    model.paste(&temp.path().join("bet").display().to_string());
    key(&mut model, KeyCode::Tab);
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(f)) if f.active == 0 && std::path::Path::new(&f.fields[0].editor.text) == temp.path().join("beta.txt"))
    );
    key(&mut model, KeyCode::Down);
    assert!(matches!(&model.overlay, Some(Overlay::Form(f)) if f.active == 1));
    key(&mut model, KeyCode::Tab);
    assert!(matches!(&model.overlay, Some(Overlay::Form(f)) if f.active == 0));
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

fn click(
    model: &mut Model,
    hits: &ui::HitMap,
    area: ratatui::layout::Rect,
    last: &mut Option<(Instant, u16, u16)>,
) {
    super::handle_mouse(
        model,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + area.width / 2,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        },
        hits,
        last,
    );
}

fn upload_form() -> (TempDir, Model) {
    let (temp, mut model) = fixture();
    std::fs::write(temp.path().join("alpha.txt"), "a").unwrap();
    std::fs::write(temp.path().join("beta.txt"), "b").unwrap();
    model.action(Action::Upload);
    model.paste("alpha.txt");
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    (temp, model)
}

#[test]
fn clicking_upload_browse_and_double_clicking_a_file_only_updates_the_local_path() {
    let (temp, mut model) = upload_form();
    let Some(Overlay::Form(form)) = &mut model.overlay else {
        panic!("upload form")
    };
    form.fields[1].editor = Editor::new("repo:Research/custom/name.bin");
    form.conflict = 1;
    let (_, hits) = render(&model, 100, 32);
    let mut last = None;
    assert_eq!(hits.dialog.browse.len(), 1);
    click(&mut model, &hits, hits.dialog.browse[0].0, &mut last);
    assert_eq!(model.requests.len(), 1);
    flush_demo(&mut model);
    let Some(Overlay::Picker(picker)) = &model.overlay else {
        panic!("file dialog")
    };
    assert!(!picker.allows_multiple());
    let index = picker
        .entries
        .iter()
        .position(|entry| entry.name == "beta.txt")
        .unwrap();
    let (_, hits) = render(&model, 100, 32);
    let dialog = hits.dialog.picker.as_ref().unwrap();
    let row = ratatui::layout::Rect::new(
        dialog.list.x,
        dialog.list.y + (index - dialog.start) as u16,
        dialog.list.width,
        1,
    );
    click(&mut model, &hits, row, &mut last);
    assert!(matches!(model.overlay, Some(Overlay::Picker(_))));
    click(&mut model, &hits, row, &mut last);
    let Some(Overlay::Form(form)) = &model.overlay else {
        panic!("returned upload form")
    };
    assert_eq!(
        std::path::Path::new(&form.fields[0].editor.text),
        temp.path().join("beta.txt")
    );
    assert_eq!(form.fields[1].editor.text, "repo:Research/custom/name.bin");
    assert_eq!(form.policy(), "overwrite");
    assert!(model.transfers.is_empty());
}

#[test]
fn directory_dialog_cancel_and_select_preserve_download_options_and_remote_files() {
    let (temp, mut model) = fixture();
    let picked = temp.path().join("选中 目录");
    std::fs::create_dir(&picked).unwrap();
    select_file(&mut model, "paper-draft.pdf");
    model.action(Action::Download);
    flush_demo(&mut model);
    key(&mut model, KeyCode::Enter);
    let original = temp
        .path()
        .join("missing/typed-directory")
        .display()
        .to_string();
    let Some(Overlay::Form(form)) = &mut model.overlay else {
        panic!("download form")
    };
    form.fields[0].editor = Editor::new(&original);
    form.conflict = 2;
    ctrl(&mut model, 'b');
    flush_demo(&mut model);
    let (_, hits) = render(&model, 100, 32);
    let mut last = None;
    click(
        &mut model,
        &hits,
        hits.dialog.picker.as_ref().unwrap().cancel,
        &mut last,
    );
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if form.fields[0].editor.text == original && form.policy() == "fail")
    );
    key(&mut model, KeyCode::F(4));
    flush_demo(&mut model);
    let Some(Overlay::Picker(picker)) = &model.overlay else {
        panic!("directory dialog")
    };
    let index = picker
        .entries
        .iter()
        .position(|entry| entry.path == picked)
        .unwrap();
    let (_, hits) = render(&model, 100, 32);
    let dialog = hits.dialog.picker.as_ref().unwrap();
    let row = ratatui::layout::Rect::new(
        dialog.list.x,
        dialog.list.y + (index - dialog.start) as u16,
        dialog.list.width,
        1,
    );
    click(&mut model, &hits, row, &mut last);
    // Choosing a highlighted folder does not require entering that folder first.
    last = None;
    click(
        &mut model,
        &hits,
        hits.dialog.picker.as_ref().unwrap().select,
        &mut last,
    );
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if std::path::Path::new(&form.fields[0].editor.text) == picked && form.policy() == "fail" && matches!(&form.kind, FormKind::Download(remotes) if remotes == &["repo:Research/paper-draft.pdf"]))
    );
    assert!(model.transfers.is_empty());
}

#[test]
fn picker_browse_button_expands_matches_and_parent_and_wheel_are_mouse_operable() {
    let (temp, mut model) = fixture();
    let child = temp.path().join("child");
    std::fs::create_dir(&child).unwrap();
    for index in 0..12 {
        std::fs::write(temp.path().join(format!("file-{index:02}.txt")), "data").unwrap();
    }
    model.local = child;
    model.action(Action::Upload);
    flush_demo(&mut model);
    let (_, hits) = render(&model, 80, 22);
    let mut last = None;
    click(
        &mut model,
        &hits,
        hits.dialog.picker.as_ref().unwrap().parent,
        &mut last,
    );
    flush_demo(&mut model);
    model.paste("file-00.txt");
    flush_demo(&mut model);
    let (_, hits) = render(&model, 80, 22);
    click(
        &mut model,
        &hits,
        hits.dialog.picker.as_ref().unwrap().browse,
        &mut last,
    );
    flush_demo(&mut model);
    assert!(
        matches!(&model.overlay, Some(Overlay::Picker(p)) if std::path::Path::new(&p.input.text) == temp.path() && p.entries.len() == 13)
    );
    let (_, hits) = render(&model, 80, 22);
    let list = hits.dialog.picker.as_ref().unwrap().list;
    super::handle_mouse(
        &mut model,
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: list.x,
            row: list.y,
            modifiers: KeyModifiers::NONE,
        },
        &hits,
        &mut last,
    );
    assert!(matches!(&model.overlay, Some(Overlay::Picker(p)) if p.selected == 4));
    click(&mut model, &hits, hits.tabs[3].0, &mut last);
    assert_eq!(model.workspace, Workspace::Files);
}

#[test]
fn browse_controls_fit_narrow_dialogs_in_both_languages_and_escape_restores_the_form() {
    let (_temp, mut model) = upload_form();
    for language in [
        super::i18n::Language::English,
        super::i18n::Language::Chinese,
    ] {
        model.settings.language = language;
        let (_, hits) = render(&model, 60, 18);
        let browse = hits.dialog.browse[0].0;
        assert!(browse.right() <= 60);
        assert!(hits.dialog.fields[0].0.right() < browse.x);
        model.browse_path(0);
        flush_demo(&mut model);
        let (_, hits) = render(&model, 60, 18);
        let dialog = hits.dialog.picker.unwrap();
        assert!(
            dialog.browse.right() <= 60
                && dialog.select.right() < dialog.cancel.x
                && dialog.cancel.right() <= 60
        );
        key(&mut model, KeyCode::Esc);
        assert!(matches!(model.overlay, Some(Overlay::Form(_))));
    }
}

#[test]
fn authentication_fields_do_not_expose_path_browsing_controls() {
    let temp = tempdir().unwrap();
    let mut model = Model::new(
        temp.path().to_path_buf(),
        temp.path().to_path_buf(),
        None,
        false,
    );
    model.configure_settings(
        super::settings::SettingsView {
            language: super::i18n::Language::English,
            user_agent: None,
            check_updates: true,
            proxy: crate::network::ProxyConfig::Auto,
            active_account: None,
            accounts: vec![],
        },
        false,
    );
    let (_, hits) = render(&model, 100, 32);
    assert!(hits.dialog.browse.is_empty());
    ctrl(&mut model, 'b');
    key(&mut model, KeyCode::F(4));
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if matches!(form.kind, FormKind::Login))
    );
    assert!(model.requests.is_empty());
}

#[test]
fn reselecting_an_upload_file_updates_the_untouched_suggested_remote_name() {
    let (_temp, mut model) = upload_form();
    model.browse_path(0);
    flush_demo(&mut model);
    let Some(Overlay::Picker(picker)) = &mut model.overlay else {
        panic!("file dialog")
    };
    picker.selected = picker
        .entries
        .iter()
        .position(|entry| entry.name == "beta.txt")
        .unwrap();
    key(&mut model, KeyCode::Enter);
    assert!(
        matches!(&model.overlay, Some(Overlay::Form(form)) if form.fields[1].editor.text == "repo:Research/beta.txt")
    );
}

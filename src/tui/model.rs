use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    app::{
        account_service::{AccountInfoResult, RepoItem},
        list_service::ListItem,
        share_service::{ShareItem, ShareResult},
    },
    contract::RemoteRef,
};

use super::{
    backend::{Message, Operation, Request},
    editor::Editor,
    i18n::Language,
    settings::{AuthorizedAccount, Credential, SecretToken, SettingsView, validate_token},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    Files,
    Transfers,
    Shares,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Libraries,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    pub repo: String,
    pub repo_id: String,
    pub path: String,
}

impl Location {
    pub fn remote(&self) -> String {
        format!("repo:{}{}", self.repo, self.path)
    }
    pub fn child(&self, path: &str) -> Self {
        Self {
            path: path.to_string(),
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Name,
    NameDescending,
    Size,
}

impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "name ↑",
            Self::NameDescending => "name ↓",
            Self::Size => "size ↓",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Name => Self::NameDescending,
            Self::NameDescending => Self::Size,
            Self::Size => Self::Name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Files,
    Transfers,
    Shares,
    Upload,
    Download,
    Share,
    Mkdir,
    NewLibrary,
    Jump,
    Refresh,
    Sort,
    Details,
    Help,
    Quit,
    Settings,
    Accounts,
    Login,
    SignOut,
}

pub struct PaletteCommand {
    pub action: Action,
    pub name: &'static str,
    pub description: &'static str,
    pub key: &'static str,
}

pub const COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        action: Action::Settings,
        name: "Settings",
        description: "Language and account settings",
        key: "4",
    },
    PaletteCommand {
        action: Action::Accounts,
        name: "Accounts",
        description: "Switch between saved accounts",
        key: "",
    },
    PaletteCommand {
        action: Action::Login,
        name: "Sign in",
        description: "Add an account using a token",
        key: "",
    },
    PaletteCommand {
        action: Action::SignOut,
        name: "Sign out",
        description: "Sign out of this workspace",
        key: "",
    },
    PaletteCommand {
        action: Action::Files,
        name: "Files",
        description: "Browse your cloud libraries",
        key: "1",
    },
    PaletteCommand {
        action: Action::Upload,
        name: "Upload",
        description: "Choose a local file to upload",
        key: "u",
    },
    PaletteCommand {
        action: Action::Download,
        name: "Download",
        description: "Download the selected files",
        key: "d",
    },
    PaletteCommand {
        action: Action::Share,
        name: "Share",
        description: "Create a link with optional password and expiry",
        key: "s",
    },
    PaletteCommand {
        action: Action::Transfers,
        name: "Transfers",
        description: "Track, cancel, or retry your transfers",
        key: "2",
    },
    PaletteCommand {
        action: Action::Shares,
        name: "Shares",
        description: "Browse and revoke existing share links",
        key: "3",
    },
    PaletteCommand {
        action: Action::Jump,
        name: "Goto",
        description: "Jump to a library or remote directory",
        key: "g",
    },
    PaletteCommand {
        action: Action::Mkdir,
        name: "Mkdir",
        description: "Create a folder in the current library",
        key: "n",
    },
    PaletteCommand {
        action: Action::NewLibrary,
        name: "Mkrepo",
        description: "Create a new library",
        key: "N",
    },
    PaletteCommand {
        action: Action::Refresh,
        name: "Refresh",
        description: "Reload the current workspace",
        key: "r",
    },
    PaletteCommand {
        action: Action::Sort,
        name: "Sort",
        description: "Cycle file sorting: name, reverse name, size",
        key: "o",
    },
    PaletteCommand {
        action: Action::Details,
        name: "Details",
        description: "Toggle the file inspector",
        key: "i",
    },
    PaletteCommand {
        action: Action::Help,
        name: "Help",
        description: "Keyboard shortcuts and workflow guide",
        key: "?",
    },
    PaletteCommand {
        action: Action::Quit,
        name: "Quit",
        description: "Leave the workspace",
        key: "q",
    },
];

#[derive(Clone)]
pub struct Field {
    pub label: &'static str,
    pub editor: Editor,
    pub secret: bool,
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("label", &self.label)
            .field(
                "value",
                &if self.secret {
                    "[redacted]"
                } else {
                    &self.editor.text
                },
            )
            .finish()
    }
}

impl Field {
    fn new(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            editor: Editor::new(value),
            secret: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FormKind {
    Login,
    Upload,
    UploadMany(Vec<PathBuf>),
    Download(Vec<String>),
    Share(String),
    Mkdir,
    NewLibrary,
    Jump,
}

#[derive(Debug, Clone)]
pub struct Form {
    pub kind: FormKind,
    pub title: &'static str,
    pub fields: Vec<Field>,
    pub active: usize,
    pub conflict: usize,
    pub error: Option<String>,
}

impl Form {
    fn new(kind: FormKind, title: &'static str, fields: Vec<Field>) -> Self {
        Self {
            kind,
            title,
            fields,
            active: 0,
            conflict: 0,
            error: None,
        }
    }
    pub fn policy(&self) -> &'static str {
        ["uniquify", "overwrite", "fail"][self.conflict % 3]
    }
    pub fn is_transfer(&self) -> bool {
        matches!(
            self.kind,
            FormKind::Upload | FormKind::UploadMany(_) | FormKind::Download(_)
        )
    }
    pub fn can_browse(&self, field: usize) -> bool {
        field == 0 && matches!(self.kind, FormKind::Upload | FormKind::Download(_))
    }
}

#[derive(Debug, Clone)]
pub struct LocalEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum PickerPurpose {
    Upload,
    Download { remotes: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct Picker {
    pub return_to: Option<Box<Form>>,
    pub browse_target: Option<PathBuf>,
    pub purpose: PickerPurpose,
    pub cwd: PathBuf,
    pub entries: Vec<LocalEntry>,
    pub selected: usize,
    pub hidden: bool,
    pub loading: bool,
    pub generation: u64,
    pub error: Option<String>,
    pub input: Editor,
    pub chosen: HashSet<PathBuf>,
    pub match_root: PathBuf,
    pub pattern: bool,
    pub truncated: bool,
}

impl Picker {
    fn new(purpose: PickerPurpose, cwd: PathBuf) -> Self {
        Self {
            return_to: None,
            browse_target: None,
            purpose,
            cwd: cwd.clone(),
            entries: Vec::new(),
            selected: 0,
            hidden: false,
            loading: true,
            generation: 0,
            error: None,
            input: Editor::default(),
            chosen: HashSet::new(),
            match_root: cwd.clone(),
            pattern: false,
            truncated: false,
        }
    }

    pub fn allows_multiple(&self) -> bool {
        !self.directories_only() && self.return_to.is_none()
    }
    pub fn directories_only(&self) -> bool {
        matches!(self.purpose, PickerPurpose::Download { .. })
    }
    pub fn is_current_directory(&self, entry: &LocalEntry) -> bool {
        self.directories_only() && entry.name == "."
    }
}

#[derive(Debug, Clone)]
pub enum Overlay {
    Accounts { selected: usize },
    Palette { input: Editor, selected: usize },
    Form(Form),
    Picker(Picker),
    Busy { id: u64, form: Box<Form> },
    ShareReady(ShareResult),
    Revoke { token: String, path: String },
    Details { title: String, lines: Vec<String> },
    Help { scroll: u16 },
    Quit,
}

#[derive(Debug, Clone)]
pub enum TransferSpec {
    Upload {
        local: PathBuf,
        remote: String,
        policy: String,
    },
    Download {
        remote: String,
        local: PathBuf,
        policy: String,
    },
}

impl TransferSpec {
    pub fn name(&self) -> String {
        match self {
            Self::Upload { local, .. } => local
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            Self::Download { remote, .. } => {
                remote.rsplit('/').next().unwrap_or(remote).to_string()
            }
        }
    }
    pub fn direction(&self) -> &'static str {
        match self {
            Self::Upload { .. } => "↑ upload",
            Self::Download { .. } => "↓ download",
        }
    }
    pub fn source(&self) -> String {
        match self {
            Self::Upload { local, .. } => local.display().to_string(),
            Self::Download { remote, .. } => remote.clone(),
        }
    }
    pub fn destination(&self) -> String {
        match self {
            Self::Upload { remote, .. } => remote.clone(),
            Self::Download { local, .. } => local.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl TransferStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "transferring",
            Self::Done => "complete",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

pub struct Transfer {
    pub id: u64,
    pub spec: TransferSpec,
    pub status: TransferStatus,
    pub transferred: u64,
    pub total: Option<u64>,
    pub started: Option<Instant>,
    pub finished: Option<Instant>,
    pub detail: String,
    pub final_path: Option<String>,
}

impl Transfer {
    pub fn ratio(&self) -> f64 {
        if self.status == TransferStatus::Done {
            return 1.0;
        }
        self.total.filter(|t| *t > 0).map_or(0.0, |t| {
            (self.transferred as f64 / t as f64).clamp(0.0, 1.0)
        })
    }
    pub fn elapsed(&self) -> Duration {
        self.started.map_or(Duration::ZERO, |s| {
            self.finished
                .unwrap_or_else(Instant::now)
                .saturating_duration_since(s)
        })
    }
}

pub struct Notice {
    pub text: String,
    pub error: bool,
    pub since: Instant,
}

pub struct Model {
    pub settings: SettingsView,
    pub settings_cursor: usize,
    pub signed_in: bool,
    pub session_epoch: u64,
    auth_request: u64,
    pub workspace: Workspace,
    pub focus: Focus,
    pub demo: bool,
    pub repos: Vec<RepoItem>,
    pub library_cursor: usize,
    pub repos_loading: bool,
    pub account: Option<AccountInfoResult>,
    pub connected: bool,
    pub location: Option<Location>,
    pub entries: Vec<ListItem>,
    pub cursor: usize,
    pub loading: bool,
    pub load_error: Option<String>,
    pub selected: HashSet<String>,
    pub filter: Editor,
    pub filtering: bool,
    pub sort: Sort,
    pub inspector: bool,
    pub overlay: Option<Overlay>,
    pub notice: Option<Notice>,
    pub transfers: Vec<Transfer>,
    pub transfer_cursor: usize,
    pub shares: Vec<ShareItem>,
    pub share_cursor: usize,
    pub shares_loading: bool,
    pub shares_loaded: bool,
    pub shares_error: Option<String>,
    pub local: PathBuf,
    pub should_quit: bool,
    pub quitting: bool,
    pub tick: u64,
    pub requests: Vec<Request>,
    initial_remote: Option<String>,
    generation: u64,
    repo_generation: u64,
    share_generation: u64,
    sequence: u64,
    cursors: HashMap<Location, usize>,
}

impl Model {
    pub fn new(local: PathBuf, initial_remote: Option<String>, demo: bool) -> Self {
        let mut model = Self {
            settings: SettingsView {
                language: Language::English,
                active_account: None,
                accounts: Vec::new(),
            },
            settings_cursor: 0,
            signed_in: true,
            session_epoch: 0,
            auth_request: 0,
            workspace: Workspace::Files,
            focus: Focus::Files,
            demo,
            repos: Vec::new(),
            library_cursor: 0,
            repos_loading: false,
            account: None,
            connected: false,
            location: None,
            entries: Vec::new(),
            cursor: 0,
            loading: false,
            load_error: None,
            selected: HashSet::new(),
            filter: Editor::default(),
            filtering: false,
            sort: Sort::Name,
            inspector: true,
            overlay: None,
            notice: None,
            transfers: Vec::new(),
            transfer_cursor: 0,
            shares: Vec::new(),
            share_cursor: 0,
            shares_loading: false,
            shares_loaded: false,
            shares_error: None,
            local,
            should_quit: false,
            quitting: false,
            tick: 0,
            requests: Vec::new(),
            initial_remote,
            generation: 0,
            repo_generation: 0,
            share_generation: 0,
            sequence: 0,
            cursors: HashMap::new(),
        };
        model.refresh_repos();
        model.requests.push(Request::Account);
        model
    }

    fn id(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    pub fn configure_settings(&mut self, settings: SettingsView, has_token: bool) {
        self.settings = settings;
        self.signed_in = has_token || self.demo;
        if !self.signed_in {
            self.requests.clear();
            self.repos_loading = false;
            self.open_login();
        }
    }

    pub fn language(&self) -> Language {
        self.settings.language
    }

    fn can_switch_account(&mut self) -> bool {
        if self.active_transfers() > 0 {
            self.toast(
                "Finish or cancel your transfers before switching accounts.",
                true,
            );
            false
        } else if self.demo {
            self.toast(
                "This demo is read-only. Run thufs tui to work with your cloud drive.",
                false,
            );
            false
        } else {
            true
        }
    }

    fn open_login(&mut self) {
        let mut token = Field::new("Token", "");
        token.secret = true;
        self.overlay = Some(Overlay::Form(Form::new(
            FormKind::Login,
            "Sign in",
            vec![token, Field::new("Account name (optional)", "")],
        )));
    }

    fn reset_account(&mut self) {
        self.requests.clear();
        self.repos.clear();
        self.entries.clear();
        self.shares.clear();
        self.transfers.clear();
        self.account = None;
        self.location = None;
        self.cursors.clear();
        self.selected.clear();
        self.filter = Editor::default();
        self.filtering = false;
        self.connected = false;
        self.cursor = 0;
        self.library_cursor = 0;
        self.share_cursor = 0;
        self.transfer_cursor = 0;
        self.loading = false;
        self.repos_loading = false;
        self.shares_loading = false;
        self.shares_loaded = false;
        self.load_error = None;
        self.shares_error = None;
        self.initial_remote = None;
        self.session_epoch += 1;
        self.overlay = None;
    }

    fn accept_account(&mut self, authorized: AuthorizedAccount) {
        let language = self.language();
        self.reset_account();
        self.settings = authorized.settings;
        self.settings.language = language;
        self.signed_in = true;
        self.account = Some(authorized.account);
        self.requests.push(Request::UseSession {
            epoch: self.session_epoch,
            token: Some(authorized.token),
        });
        self.workspace = Workspace::Files;
        self.focus = Focus::Files;
        self.refresh_repos();
        self.toast("Account switched", false);
    }

    fn settings_enter(&mut self) {
        match self.settings_cursor {
            0 => {
                self.settings.language = self.settings.language.next();
                self.requests
                    .push(Request::SaveLanguage(self.settings.language));
            }
            1 => self.action(Action::Accounts),
            2 => self.action(Action::Login),
            3 => self.action(Action::SignOut),
            _ => {}
        }
    }
    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.notice = Some(Notice {
            text: text.into(),
            error,
            since: Instant::now(),
        });
    }

    pub fn file_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.entries.len())
            .filter(|&i| fuzzy_score(&self.entries[i].name, &self.filter.text).is_some())
            .collect();
        indices.sort_by(|&a, &b| {
            let a = &self.entries[a];
            let b = &self.entries[b];
            (a.kind != "dir")
                .cmp(&(b.kind != "dir"))
                .then_with(|| {
                    if !self.filter.text.is_empty() {
                        return fuzzy_score(&a.name, &self.filter.text)
                            .cmp(&fuzzy_score(&b.name, &self.filter.text));
                    }
                    match self.sort {
                        Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        Sort::NameDescending => b.name.to_lowercase().cmp(&a.name.to_lowercase()),
                        Sort::Size => b.size.cmp(&a.size),
                    }
                })
                .then(a.name.cmp(&b.name))
        });
        indices
    }

    pub fn current_file(&self) -> Option<&ListItem> {
        self.file_indices()
            .get(self.cursor)
            .map(|&i| &self.entries[i])
    }
    pub fn share_indices(&self) -> Vec<usize> {
        (0..self.shares.len())
            .filter(|&i| {
                fuzzy_score(
                    &format!(
                        "{} {}",
                        self.shares[i].path.as_deref().unwrap_or_default(),
                        self.shares[i].repo.as_deref().unwrap_or_default()
                    ),
                    &self.filter.text,
                )
                .is_some()
            })
            .collect()
    }
    pub fn current_share(&self) -> Option<&ShareItem> {
        self.share_indices()
            .get(self.share_cursor)
            .map(|&i| &self.shares[i])
    }
    pub fn active_transfers(&self) -> usize {
        self.transfers.iter().filter(|t| t.status.active()).count()
    }

    pub fn navigate(&mut self, location: Location) {
        if self.loading && self.location.as_ref() == Some(&location) {
            return;
        }
        if let Some(current) = self.location.as_ref() {
            self.cursors.insert(current.clone(), self.cursor);
        }
        self.cursor = self.cursors.get(&location).copied().unwrap_or(0);
        self.library_cursor = self
            .repos
            .iter()
            .position(|r| r.id == location.repo_id)
            .unwrap_or(0);
        self.location = Some(location.clone());
        self.entries.clear();
        self.selected.clear();
        self.filter = Editor::default();
        self.filtering = false;
        self.load_error = None;
        self.loading = true;
        self.focus = Focus::Files;
        self.generation = self.id();
        self.requests.push(Request::Directory {
            id: self.generation,
            location,
        });
    }

    pub fn open_library(&mut self) {
        if let Some(repo) = self.repos.get(self.library_cursor) {
            self.navigate(Location {
                repo: repo.name.clone(),
                repo_id: repo.id.clone(),
                path: "/".to_string(),
            });
        }
    }

    pub fn parent(&mut self) {
        if let Some(location) = &self.location {
            if location.path != "/" {
                let parent = location
                    .path
                    .trim_end_matches('/')
                    .rsplit_once('/')
                    .map_or("/", |(p, _)| if p.is_empty() { "/" } else { p });
                self.navigate(location.child(parent));
            } else {
                self.focus = Focus::Libraries;
            }
        } else {
            self.focus = Focus::Libraries;
        }
    }

    pub fn enter(&mut self) {
        match self.workspace {
            Workspace::Settings => self.settings_enter(),
            Workspace::Files if self.focus == Focus::Libraries => self.open_library(),
            Workspace::Files => {
                if let (Some(file), Some(location)) = (self.current_file(), self.location.as_ref())
                {
                    if file.kind == "dir" {
                        self.navigate(location.child(&file.path));
                    } else {
                        self.overlay = Some(Overlay::Details {
                            title: file.name.clone(),
                            lines: vec![
                                format!("Path: repo:{}{}", location.repo, file.path),
                                format!(
                                    "Size: {}",
                                    file.size
                                        .map(crate::transfer::format_size)
                                        .unwrap_or_else(|| "unknown".into())
                                ),
                                "".into(),
                                "d  Download     s  Share     y  Copy path".into(),
                            ],
                        });
                    }
                }
            }
            Workspace::Shares => {
                if let Some(share) = self.current_share() {
                    self.overlay = Some(Overlay::Details {
                        title: "Share link".into(),
                        lines: vec![
                            share.path.clone().unwrap_or_default(),
                            share.url.clone(),
                            "".into(),
                            "y  Copy link     x  Revoke (with confirmation)".into(),
                        ],
                    });
                }
            }
            Workspace::Transfers => {
                if let Some(t) = self.transfers.get(self.transfer_cursor) {
                    self.overlay = Some(Overlay::Details {
                        title: t.spec.name(),
                        lines: vec![
                            format!("Status: {}", t.status.label()),
                            format!("From: {}", t.spec.source()),
                            format!(
                                "To: {}",
                                t.final_path.clone().unwrap_or_else(|| t.spec.destination())
                            ),
                            "".into(),
                            t.detail.clone(),
                        ],
                    });
                }
            }
        }
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let (cursor, len) = match self.workspace {
            Workspace::Settings => (&mut self.settings_cursor, 4),
            Workspace::Files if self.focus == Focus::Libraries => {
                (&mut self.library_cursor, self.repos.len())
            }
            Workspace::Files => {
                let len = self.file_indices().len();
                (&mut self.cursor, len)
            }
            Workspace::Transfers => (&mut self.transfer_cursor, self.transfers.len()),
            Workspace::Shares => {
                let len = self.share_indices().len();
                (&mut self.share_cursor, len)
            }
        };
        *cursor = cursor
            .saturating_add_signed(delta)
            .min(len.saturating_sub(1));
    }

    pub fn switch(&mut self, workspace: Workspace) {
        let file_path = self.current_file().map(|f| f.path.clone());
        let share_url = self.current_share().map(|s| s.url.clone());
        self.workspace = workspace;
        self.filter = Editor::default();
        self.filtering = false;
        self.cursor = self
            .file_indices()
            .iter()
            .position(|&i| Some(&self.entries[i].path) == file_path.as_ref())
            .unwrap_or(0);
        self.share_cursor = self
            .share_indices()
            .iter()
            .position(|&i| Some(&self.shares[i].url) == share_url.as_ref())
            .unwrap_or(0);
        if workspace == Workspace::Shares
            && self.signed_in
            && !self.shares_loaded
            && !self.shares_loading
        {
            self.refresh_shares();
        }
    }

    fn refresh_repos(&mut self) {
        self.repo_generation = self.id();
        self.repos_loading = true;
        self.requests.push(Request::Repositories {
            id: self.repo_generation,
        });
    }
    fn refresh_shares(&mut self) {
        self.share_generation = self.id();
        self.shares_loading = true;
        self.shares_error = None;
        self.requests.push(Request::Shares {
            id: self.share_generation,
        });
    }

    pub fn action(&mut self, action: Action) {
        if !self.signed_in
            && matches!(
                action,
                Action::Upload
                    | Action::Refresh
                    | Action::Download
                    | Action::Share
                    | Action::Mkdir
                    | Action::NewLibrary
                    | Action::Jump
            )
        {
            self.open_login();
            return;
        }
        if self.demo && matches!(action, Action::Share | Action::Mkdir | Action::NewLibrary) {
            self.toast(
                "This demo is read-only. Run thufs tui to work with your cloud drive.",
                false,
            );
            return;
        }
        match action {
            Action::Settings => self.switch(Workspace::Settings),
            Action::Login => {
                if self.can_switch_account() {
                    self.open_login();
                }
            }
            Action::Accounts => {
                if self.can_switch_account() {
                    self.overlay = Some(Overlay::Accounts { selected: 0 });
                }
            }
            Action::SignOut => {
                if self.signed_in && self.can_switch_account() {
                    let id = self.id();
                    self.auth_request = id;
                    self.requests.push(Request::SignOut { id });
                    self.overlay = Some(Overlay::Busy {
                        id,
                        form: Box::new(Form::new(FormKind::Login, "Sign out", vec![])),
                    });
                }
            }
            Action::Files => self.switch(Workspace::Files),
            Action::Transfers => self.switch(Workspace::Transfers),
            Action::Shares => self.switch(Workspace::Shares),
            Action::Help => self.overlay = Some(Overlay::Help { scroll: 0 }),
            Action::Quit => {
                if self.active_transfers() > 0 {
                    self.overlay = Some(Overlay::Quit);
                } else {
                    self.should_quit = true;
                }
            }
            Action::Details => self.inspector = !self.inspector,
            Action::Sort => {
                self.sort = self.sort.next();
                self.cursor = 0;
            }
            Action::Refresh => match self.workspace {
                Workspace::Settings => {}
                Workspace::Files if self.focus == Focus::Libraries || self.location.is_none() => {
                    self.refresh_repos()
                }
                Workspace::Files => {
                    if let Some(location) = self.location.clone() {
                        self.navigate(location);
                    }
                }
                Workspace::Shares => self.refresh_shares(),
                Workspace::Transfers => self.retry_transfer(),
            },
            Action::Jump => {
                self.overlay = Some(Overlay::Form(Form::new(
                    FormKind::Jump,
                    "Go to a directory",
                    vec![Field::new(
                        "Library or remote path",
                        self.location
                            .as_ref()
                            .map(Location::remote)
                            .unwrap_or_default(),
                    )],
                )))
            }
            Action::NewLibrary => {
                self.overlay = Some(Overlay::Form(Form::new(
                    FormKind::NewLibrary,
                    "New library",
                    vec![Field::new("Library name", "")],
                )))
            }
            Action::Upload | Action::Download | Action::Share | Action::Mkdir => {
                let Some(location) = self.location.clone() else {
                    self.toast(
                        "Open a library first. Tab switches to the library list.",
                        true,
                    );
                    return;
                };
                match action {
                    Action::Upload => {
                        self.open_picker(PickerPurpose::Upload);
                    }
                    Action::Download => {
                        let files: Vec<_> = self
                            .entries
                            .iter()
                            .filter(|f| f.kind == "file" && self.selected.contains(&f.path))
                            .collect();
                        let files = if files.is_empty() {
                            self.current_file()
                                .filter(|f| f.kind == "file")
                                .into_iter()
                                .collect()
                        } else {
                            files
                        };
                        if files.is_empty() {
                            self.toast(
                                "Select a file to download. Space selects multiple files.",
                                true,
                            );
                            return;
                        }
                        let remotes = files
                            .iter()
                            .map(|f| format!("repo:{}{}", location.repo, f.path))
                            .collect();
                        self.open_picker(PickerPurpose::Download { remotes });
                    }
                    Action::Share => {
                        let Some(file) = self.current_file() else {
                            self.toast("Select a file or folder to share.", true);
                            return;
                        };
                        let remote = format!("repo:{}{}", location.repo, file.path);
                        let mut password = Field::new("Password (optional)", "");
                        password.secret = true;
                        self.overlay = Some(Overlay::Form(Form::new(
                            FormKind::Share(remote),
                            "Create a share link",
                            vec![
                                password,
                                Field::new("Expires in days (blank: no expiry)", "7"),
                            ],
                        )));
                    }
                    Action::Mkdir => {
                        self.overlay = Some(Overlay::Form(Form::new(
                            FormKind::Mkdir,
                            "New folder",
                            vec![Field::new("Folder name", "")],
                        )))
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            if matches!(self.overlay, Some(Overlay::Busy { .. })) {
                self.toast(
                    "Finishing the server request. You can quit when it completes.",
                    false,
                );
            } else {
                self.overlay = None;
                self.action(Action::Quit);
            }
            return;
        }
        let palette_shortcut = key.code == KeyCode::Char('p')
            && key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);
        if palette_shortcut && !matches!(self.overlay, Some(Overlay::Busy { .. })) {
            if matches!(self.overlay, Some(Overlay::Palette { .. })) {
                self.overlay = None;
            } else {
                self.open_palette();
            }
            return;
        }
        if self.overlay.is_some() {
            self.overlay_key(key);
            return;
        }
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filtering = false;
                    self.filter = Editor::default();
                    self.cursor = 0;
                    self.share_cursor = 0;
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Up => self.move_cursor(-1),
                KeyCode::Down => self.move_cursor(1),
                _ => {
                    if self.filter.handle(key) {
                        self.cursor = 0;
                        self.share_cursor = 0;
                    }
                }
            }
            return;
        }
        if ctrl && key.code == KeyCode::Char('p') {
            self.open_palette();
            return;
        }
        if ctrl || key.modifiers.contains(KeyModifiers::ALT) {
            return;
        }
        match key.code {
            KeyCode::Char('4') | KeyCode::Char(',') => self.switch(Workspace::Settings),
            KeyCode::Char('?') | KeyCode::F(1) => self.action(Action::Help),
            KeyCode::Char('1') => self.switch(Workspace::Files),
            KeyCode::Char('2') => self.switch(Workspace::Transfers),
            KeyCode::Char('3') => self.switch(Workspace::Shares),
            KeyCode::Char('q') => self.action(Action::Quit),
            KeyCode::Tab | KeyCode::BackTab if self.workspace == Workspace::Files => {
                self.focus = if self.focus == Focus::Files {
                    Focus::Libraries
                } else {
                    Focus::Files
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::PageUp => self.move_cursor(-10),
            KeyCode::PageDown => self.move_cursor(10),
            KeyCode::Home => self.move_cursor(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_cursor(isize::MAX),
            KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h')
                if self.workspace == Workspace::Files =>
            {
                self.parent()
            }
            KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => self.enter(),
            KeyCode::Left if self.workspace == Workspace::Settings && self.settings_cursor == 0 => {
                self.settings_enter()
            }
            KeyCode::Char('f') if self.workspace != Workspace::Transfers => {
                self.focus = Focus::Files;
                self.filtering = true;
            }
            KeyCode::Esc if self.workspace == Workspace::Settings => self.switch(Workspace::Files),
            KeyCode::Esc => {
                self.filter = Editor::default();
                self.selected.clear();
            }
            KeyCode::Char(' ')
                if self.workspace == Workspace::Files && self.focus == Focus::Files =>
            {
                if let Some(file) = self.current_file().filter(|f| f.kind == "file") {
                    let path = file.path.clone();
                    if !self.selected.remove(&path) {
                        self.selected.insert(path);
                    }
                    self.move_cursor(1);
                }
            }
            KeyCode::Char('a') if self.workspace == Workspace::Files => {
                let paths: Vec<_> = self
                    .file_indices()
                    .iter()
                    .map(|&i| &self.entries[i])
                    .filter(|f| f.kind == "file")
                    .map(|f| f.path.clone())
                    .collect();
                if paths.iter().all(|p| self.selected.contains(p)) {
                    for p in paths {
                        self.selected.remove(&p);
                    }
                } else {
                    self.selected.extend(paths);
                }
            }
            KeyCode::Char('u') if self.workspace == Workspace::Files => self.action(Action::Upload),
            KeyCode::Char('d') if self.workspace == Workspace::Files => {
                self.action(Action::Download)
            }
            KeyCode::Char('s') if self.workspace == Workspace::Files => self.action(Action::Share),
            KeyCode::Char('n') if self.workspace == Workspace::Files => self.action(Action::Mkdir),
            KeyCode::Char('N') if self.workspace == Workspace::Files => {
                self.action(Action::NewLibrary)
            }
            KeyCode::Char('g') => self.action(Action::Jump),
            KeyCode::Char('r') => self.action(Action::Refresh),
            KeyCode::Char('o') if self.workspace == Workspace::Files => self.action(Action::Sort),
            KeyCode::Char('i') => self.action(Action::Details),
            KeyCode::Char('y') => self.copy_current(),
            KeyCode::Char('x') if self.workspace == Workspace::Transfers => self.cancel_transfer(),
            KeyCode::Char('x') if self.workspace == Workspace::Shares => self.revoke_selected(),
            KeyCode::Char('C') if self.workspace == Workspace::Transfers => {
                self.transfers.retain(|t| t.status.active());
                self.transfer_cursor = self
                    .transfer_cursor
                    .min(self.transfers.len().saturating_sub(1));
            }
            _ => {}
        }
    }

    pub fn paste(&mut self, text: &str) {
        if matches!(self.overlay, Some(Overlay::Picker(_))) {
            let Some(Overlay::Picker(mut picker)) = self.overlay.take() else {
                unreachable!()
            };
            picker.input.insert(text);
            self.refresh_paths(&mut picker);
            self.overlay = Some(Overlay::Picker(picker));
            return;
        }
        match self.overlay.as_mut() {
            Some(Overlay::Palette { input, selected }) => {
                input.insert(text);
                *selected = 0;
            }
            Some(Overlay::Form(form)) => {
                form.fields[form.active].editor.insert(text);
                form.error = None;
            }
            None if self.filtering => {
                self.filter.insert(text);
                self.cursor = 0;
                self.share_cursor = 0;
            }
            _ => {}
        }
    }

    pub fn open_palette(&mut self) {
        self.overlay = Some(Overlay::Palette {
            input: Editor::default(),
            selected: 0,
        });
    }

    fn refresh_paths(&mut self, picker: &mut Picker) {
        self.load_paths(picker, false);
    }

    fn load_paths(&mut self, picker: &mut Picker, browse: bool) {
        picker.selected = 0;
        picker.chosen.clear();
        picker.entries.clear();
        picker.loading = true;
        picker.error = None;
        picker.generation = self.id();
        picker.browse_target = if browse {
            Some(super::paths::resolve(&picker.input.text, &picker.cwd))
        } else {
            None
        };
        self.requests.push(Request::MatchPaths {
            id: picker.generation,
            input: picker.input.text.clone(),
            cwd: picker.cwd.clone(),
            hidden: picker.hidden,
            directories_only: picker.directories_only(),
            browse,
        });
    }

    fn open_picker(&mut self, purpose: PickerPurpose) {
        let mut picker = Picker::new(purpose, self.local.clone());
        self.refresh_paths(&mut picker);
        self.overlay = Some(Overlay::Picker(picker));
    }

    fn review_download(&mut self, picker: &Picker, directory: &std::path::Path) {
        let PickerPurpose::Download { remotes } = &picker.purpose else {
            return;
        };
        if let Some(original) = &picker.return_to {
            let mut form = (**original).clone();
            form.fields[0].editor = Editor::new(directory.display().to_string());
            form.error = None;
            self.overlay = Some(Overlay::Form(form));
            return;
        }
        self.overlay = Some(Overlay::Form(Form::new(
            FormKind::Download(remotes.clone()),
            "Download files",
            vec![Field::new(
                "Save into an existing local directory",
                directory.display().to_string(),
            )],
        )));
    }

    fn review_upload(&mut self, picker: &Picker, files: Vec<PathBuf>) {
        let Some(location) = &self.location else {
            return;
        };
        if let Some(parent) = files.first().and_then(|file| file.parent()) {
            self.local = parent.to_path_buf();
        }
        if let (Some(original), Some(file)) = (&picker.return_to, files.first()) {
            let mut form = (**original).clone();
            let old_name = std::path::Path::new(&form.fields[0].editor.text)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let suggested = format!("{}/{}", location.remote().trim_end_matches('/'), old_name);
            if form.fields[1].editor.text == suggested {
                form.fields[1].editor = Editor::new(format!(
                    "{}/{}",
                    location.remote().trim_end_matches('/'),
                    file.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            form.fields[0].editor = Editor::new(file.display().to_string());
            form.error = None;
            self.overlay = Some(Overlay::Form(form));
            return;
        }
        let form = if files.len() == 1 {
            let file = &files[0];
            Form::new(
                FormKind::Upload,
                "Upload a file",
                vec![
                    Field::new("Local file", file.display().to_string()),
                    Field::new(
                        "Remote destination",
                        format!(
                            "{}/{}",
                            location.remote().trim_end_matches('/'),
                            file.file_name().unwrap_or_default().to_string_lossy()
                        ),
                    ),
                ],
            )
        } else {
            Form::new(
                FormKind::UploadMany(files),
                "Upload files",
                vec![Field::new(
                    "Remote directory",
                    location.remote().trim_end_matches('/'),
                )],
            )
        };
        self.overlay = Some(Overlay::Form(form));
    }

    pub fn browse_path(&mut self, field: usize) {
        if let Some(Overlay::Form(form)) = &mut self.overlay {
            if !form.can_browse(field) {
                return;
            }
            form.active = field;
        } else if !matches!(self.overlay, Some(Overlay::Picker(_))) {
            return;
        }
        self.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE));
    }

    fn browse_form(&mut self, form: Form) {
        let purpose = match &form.kind {
            FormKind::Upload => PickerPurpose::Upload,
            FormKind::Download(remotes) => PickerPurpose::Download {
                remotes: remotes.clone(),
            },
            _ => return,
        };
        // Form paths follow the process working directory, just like the transfer CLI.
        let cwd = std::env::current_dir().unwrap_or_else(|_| self.local.clone());
        let mut picker = Picker::new(purpose, cwd);
        picker.input = Editor::new(form.fields[0].editor.text.clone());
        picker.return_to = Some(Box::new(form));
        self.load_paths(&mut picker, true);
        self.overlay = Some(Overlay::Picker(picker));
    }

    fn overlay_key(&mut self, key: KeyEvent) {
        let Some(mut overlay) = self.overlay.take() else {
            return;
        };
        if key.code == KeyCode::Esc && !matches!(overlay, Overlay::Busy { .. }) {
            if let Overlay::Picker(picker) = overlay {
                if let Some(form) = picker.return_to {
                    self.overlay = Some(Overlay::Form(*form));
                }
            }
            return;
        }
        match &mut overlay {
            Overlay::Accounts { selected } => {
                let count = self.settings.accounts.len();
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => *selected = (*selected + 1).min(count),
                    KeyCode::Up | KeyCode::Char('k') => *selected = selected.saturating_sub(1),
                    KeyCode::Enter => {
                        if let Some(account) = self.settings.accounts.get(*selected) {
                            let credential = Credential::Saved(account.key.clone());
                            let id = self.id();
                            self.auth_request = id;
                            self.requests.push(Request::Authenticate { id, credential });
                            self.overlay = Some(Overlay::Busy {
                                id,
                                form: Box::new(Form::new(FormKind::Login, "Signing in", vec![])),
                            });
                        } else {
                            self.open_login();
                        }
                        return;
                    }
                    _ => {}
                }
            }
            Overlay::Palette { input, selected } => {
                let matches = palette_matches_in(&input.text, self.language());
                match key.code {
                    KeyCode::Down => {
                        *selected = (*selected + 1).min(matches.len().saturating_sub(1))
                    }
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Tab => {
                        if let Some(&index) = matches.get(*selected) {
                            *input = Editor::new(COMMANDS[index].name);
                            *selected = 0;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(&index) = matches.get(*selected) {
                            self.action(COMMANDS[index].action);
                            return;
                        }
                    }
                    _ => {
                        if input.handle(key) {
                            *selected = 0;
                        }
                    }
                }
            }
            Overlay::Form(form) => match key.code {
                KeyCode::F(4) | KeyCode::Char('b')
                    if key.code == KeyCode::F(4)
                        || key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if form.can_browse(form.active) {
                        self.browse_form(form.clone());
                        return;
                    }
                }
                KeyCode::Tab | KeyCode::Down => form.active = (form.active + 1) % form.fields.len(),
                KeyCode::BackTab | KeyCode::Up => {
                    form.active = (form.active + form.fields.len() - 1) % form.fields.len()
                }
                KeyCode::Char('o')
                    if key.modifiers.contains(KeyModifiers::CONTROL) && form.is_transfer() =>
                {
                    form.conflict = (form.conflict + 1) % 3
                }
                KeyCode::Enter => {
                    if let Err(error) = self.submit(form.clone()) {
                        form.error = Some(error);
                    } else {
                        return;
                    }
                }
                _ => {
                    if form.fields[form.active].editor.handle(key) {
                        form.error = None;
                    }
                }
            },
            Overlay::Picker(picker) => {
                let control = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                let mut reload = false;
                match key.code {
                    KeyCode::F(4) | KeyCode::Char('b') if key.code == KeyCode::F(4) || control => {
                        self.load_paths(picker, true);
                    }
                    KeyCode::Up if alt => {
                        if let Some(parent) = picker.cwd.parent() {
                            picker.cwd = parent.to_path_buf();
                            picker.input = Editor::default();
                            reload = true;
                        }
                    }
                    KeyCode::Down => {
                        picker.selected =
                            (picker.selected + 1).min(picker.entries.len().saturating_sub(1))
                    }
                    KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
                    KeyCode::PageDown => {
                        picker.selected =
                            (picker.selected + 10).min(picker.entries.len().saturating_sub(1))
                    }
                    KeyCode::PageUp => picker.selected = picker.selected.saturating_sub(10),
                    KeyCode::Char('.') if alt => {
                        picker.hidden = !picker.hidden;
                        reload = true;
                    }
                    KeyCode::Char('l') if control => {
                        picker.input = Editor::default();
                        reload = true;
                    }
                    KeyCode::Char('d') if control && picker.directories_only() => {
                        if let Some(entry) = picker
                            .entries
                            .get(picker.selected)
                            .filter(|entry| entry.is_dir)
                        {
                            self.review_download(picker, &entry.path);
                            return;
                        }
                    }
                    KeyCode::Char('a') if control && picker.allows_multiple() => {
                        let files: HashSet<_> = picker
                            .entries
                            .iter()
                            .filter(|e| !e.is_dir)
                            .map(|e| e.path.clone())
                            .collect();
                        if files == picker.chosen {
                            picker.chosen.clear();
                        } else {
                            picker.chosen = files;
                        }
                    }
                    KeyCode::Char(' ') if control && picker.allows_multiple() => {
                        if let Some(entry) =
                            picker.entries.get(picker.selected).filter(|e| !e.is_dir)
                        {
                            if !picker.chosen.remove(&entry.path) {
                                picker.chosen.insert(entry.path.clone());
                            }
                            picker.selected =
                                (picker.selected + 1).min(picker.entries.len().saturating_sub(1));
                        }
                    }
                    KeyCode::Tab => {
                        if let Some(completion) = super::paths::completion(
                            &picker.input.text,
                            &picker.cwd,
                            &picker.entries,
                            picker.selected,
                        ) {
                            picker.input = Editor::new(completion);
                            reload = true;
                        }
                    }
                    KeyCode::Enter => {
                        let files: Vec<_> = picker
                            .entries
                            .iter()
                            .filter(|e| !e.is_dir && picker.chosen.contains(&e.path))
                            .map(|e| e.path.clone())
                            .collect();
                        if !files.is_empty() {
                            self.review_upload(picker, files);
                            return;
                        }
                        if let Some(entry) = picker.entries.get(picker.selected) {
                            if picker.is_current_directory(entry) {
                                self.review_download(picker, &entry.path);
                                return;
                            } else if entry.is_dir {
                                picker.cwd = entry.path.clone();
                                picker.input = Editor::default();
                                reload = true;
                            } else if !picker.directories_only() {
                                self.review_upload(picker, vec![entry.path.clone()]);
                                return;
                            }
                        }
                    }
                    _ => {
                        if picker.input.handle(key) {
                            reload = true;
                        }
                    }
                }
                if reload {
                    self.refresh_paths(picker);
                }
            }
            Overlay::ShareReady(share) => {
                if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                    self.requests.push(Request::Copy(share.url.clone()));
                }
            }
            Overlay::Revoke { token, .. } if key.code == KeyCode::Enter => {
                if self.demo {
                    self.toast("This demo is read-only.", false);
                    return;
                }
                let id = self.id();
                self.requests.push(Request::Revoke {
                    id,
                    token: token.clone(),
                });
                self.overlay = Some(Overlay::Busy {
                    id,
                    form: Box::new(Form::new(FormKind::Jump, "Revoking share link", vec![])),
                });
                return;
            }
            Overlay::Quit if key.code == KeyCode::Enter => {
                self.quitting = true;
                for transfer in &mut self.transfers {
                    match transfer.status {
                        TransferStatus::Queued => transfer.status = TransferStatus::Cancelled,
                        TransferStatus::Running => {
                            transfer.detail = "Cancelling…".into();
                            self.requests
                                .push(Request::CancelTransfer { id: transfer.id });
                        }
                        _ => {}
                    }
                }
                self.should_quit = self.active_transfers() == 0;
                return;
            }
            Overlay::Details { .. } => match key.code {
                KeyCode::Char('d') if self.workspace == Workspace::Files => {
                    self.action(Action::Download);
                    return;
                }
                KeyCode::Char('s') if self.workspace == Workspace::Files => {
                    self.action(Action::Share);
                    return;
                }
                KeyCode::Char('x') if self.workspace == Workspace::Shares => {
                    self.revoke_selected();
                    return;
                }
                KeyCode::Char('y') => self.copy_current(),
                KeyCode::Enter => return,
                _ => {}
            },
            Overlay::Help { scroll } => match key.code {
                KeyCode::Char('?') | KeyCode::F(1) | KeyCode::Enter => return,
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1).min(24),
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::PageDown => *scroll = scroll.saturating_add(8).min(24),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(8),
                KeyCode::Home => *scroll = 0,
                _ => {}
            },
            _ => {}
        }
        self.overlay = Some(overlay);
    }

    fn submit(&mut self, form: Form) -> Result<(), String> {
        if self.demo && form.is_transfer() {
            return Err("This demo is read-only.".into());
        }
        let value = |i: usize| form.fields[i].editor.text.trim().to_string();
        let required = |i: usize| {
            let text = value(i);
            if text.is_empty() {
                Err(format!("{} cannot be empty.", form.fields[i].label))
            } else {
                Ok(text)
            }
        };
        let id = self.id();
        let request = match &form.kind {
            FormKind::UploadMany(files) => {
                let remote = required(0)?;
                if !remote.starts_with("repo:") {
                    return Err("Use repo:<library>/<path> for the upload destination.".into());
                }
                let directory = RemoteRef::parse_list_target(remote.trim_end_matches('/'), None)
                    .map_err(|e| e.to_string())?;
                let mut transfers = Vec::new();
                for local in files {
                    if !local.is_file() {
                        return Err(format!(
                            "Local file is no longer available: {}",
                            local.display()
                        ));
                    }
                    let name = local
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or("The path is not valid UTF-8.")?;
                    transfers.push(TransferSpec::Upload {
                        local: local.clone(),
                        remote: format!(
                            "repo:{}{}/{}",
                            directory.repo,
                            normalized_path(&directory.path).trim_end_matches('/'),
                            name
                        ),
                        policy: form.policy().into(),
                    });
                }
                for transfer in transfers {
                    self.enqueue(transfer);
                }
                return Ok(());
            }
            FormKind::Login => {
                let token = form.fields[0].editor.text.trim().to_string();
                validate_token(&token).map_err(|error| error.to_string())?;
                self.auth_request = id;
                Request::Authenticate {
                    id,
                    credential: Credential::Token {
                        token: SecretToken::new(token),
                        label: form.fields[1].editor.text.clone(),
                    },
                }
            }
            FormKind::Jump => {
                let remote =
                    RemoteRef::parse_list_target(&required(0)?, None).map_err(|e| e.to_string())?;
                let repo = self
                    .repos
                    .iter()
                    .find(|r| r.name == remote.repo)
                    .ok_or_else(|| format!("Library '{}' was not found.", remote.repo))?;
                self.navigate(Location {
                    repo: repo.name.clone(),
                    repo_id: repo.id.clone(),
                    path: normalized_path(&remote.path),
                });
                self.switch(Workspace::Files);
                return Ok(());
            }
            FormKind::Upload => {
                required(0)?;
                let local = expand_path(&form.fields[0].editor.text);
                if !local.is_file() {
                    return Err(
                        "Choose an existing local file. Directories are not supported.".into(),
                    );
                }
                let remote = required(1)?;
                if !remote.starts_with("repo:") {
                    return Err("Use repo:<library>/<path> for the upload destination.".into());
                }
                RemoteRef::parse_list_target(&remote, None).map_err(|e| e.to_string())?;
                self.enqueue(TransferSpec::Upload {
                    local,
                    remote,
                    policy: form.policy().into(),
                });
                return Ok(());
            }
            FormKind::Download(remotes) => {
                required(0)?;
                let local = expand_path(&form.fields[0].editor.text);
                if !local.is_dir() {
                    return Err("The download directory must already exist.".into());
                }
                self.local = local.clone();
                for remote in remotes {
                    self.enqueue(TransferSpec::Download {
                        remote: remote.clone(),
                        local: local.clone(),
                        policy: form.policy().into(),
                    });
                }
                self.selected.clear();
                return Ok(());
            }
            FormKind::Share(remote) => {
                let days = value(1);
                let expire_days = if days.is_empty() {
                    None
                } else {
                    Some(
                        days.parse::<u32>()
                            .ok()
                            .filter(|&n| n > 0)
                            .ok_or("Expiry must be a positive number of days.")?,
                    )
                };
                let password = form.fields[0].editor.text.clone();
                Request::CreateShare {
                    id,
                    remote: remote.clone(),
                    password: if password.is_empty() {
                        None
                    } else {
                        Some(password)
                    },
                    expire_days,
                }
            }
            FormKind::Mkdir => {
                let name = required(0)?;
                if name
                    .split('/')
                    .any(|s| s == "." || s == ".." || s.is_empty())
                    || name.starts_with('/')
                {
                    return Err(
                        "Use a relative folder name without empty, '.' or '..' segments.".into(),
                    );
                }
                let location = self.location.as_ref().ok_or("Open a library first.")?;
                Request::CreateDirectory {
                    id,
                    remote: format!("{}/{}", location.remote().trim_end_matches('/'), name),
                }
            }
            FormKind::NewLibrary => {
                let name = required(0)?;
                if name.contains('/') {
                    return Err("Library names cannot contain '/'.".into());
                }
                Request::CreateRepository { id, name }
            }
        };
        self.requests.push(request);
        self.overlay = Some(Overlay::Busy {
            id,
            form: Box::new(form),
        });
        Ok(())
    }

    fn enqueue(&mut self, spec: TransferSpec) {
        let id = self.id();
        let name = spec.name();
        self.transfers.push(Transfer {
            id,
            spec,
            status: TransferStatus::Queued,
            transferred: 0,
            total: None,
            started: None,
            finished: None,
            detail: String::new(),
            final_path: None,
        });
        self.toast(format!("Queued {name} · press 2 to view transfers"), false);
        self.start_next();
    }

    fn start_next(&mut self) {
        if self.quitting
            || self
                .transfers
                .iter()
                .any(|t| t.status == TransferStatus::Running)
        {
            return;
        }
        if let Some(t) = self
            .transfers
            .iter_mut()
            .find(|t| t.status == TransferStatus::Queued)
        {
            t.status = TransferStatus::Running;
            t.started = Some(Instant::now());
            self.requests.push(Request::Transfer {
                id: t.id,
                spec: t.spec.clone(),
            });
        }
    }

    pub fn cancel_transfer(&mut self) {
        if let Some(t) = self.transfers.get_mut(self.transfer_cursor) {
            match t.status {
                TransferStatus::Queued => {
                    t.status = TransferStatus::Cancelled;
                    t.detail = "Cancelled before starting".into();
                }
                TransferStatus::Running => {
                    t.detail = "Cancelling…".into();
                    self.requests.push(Request::CancelTransfer { id: t.id });
                }
                _ => {}
            }
        }
    }

    fn retry_transfer(&mut self) {
        if let Some(t) = self.transfers.get_mut(self.transfer_cursor) {
            if matches!(t.status, TransferStatus::Failed | TransferStatus::Cancelled) {
                t.status = TransferStatus::Queued;
                t.detail.clear();
                t.transferred = 0;
                t.total = None;
                t.started = None;
                t.finished = None;
                t.final_path = None;
                self.start_next();
            }
        }
    }

    fn revoke_selected(&mut self) {
        if let Some(share) = self.current_share() {
            if let Some(token) = &share.token {
                self.overlay = Some(Overlay::Revoke {
                    token: token.clone(),
                    path: share.path.clone().unwrap_or_else(|| share.url.clone()),
                });
            } else {
                self.toast("This share has no revocation token.", true);
            }
        }
    }

    fn copy_current(&mut self) {
        let text = match self.workspace {
            Workspace::Settings => None,
            Workspace::Files => self.current_file().and_then(|f| {
                self.location
                    .as_ref()
                    .map(|l| format!("repo:{}{}", l.repo, f.path))
            }),
            Workspace::Shares => self.current_share().map(|s| s.url.clone()),
            Workspace::Transfers => self
                .transfers
                .get(self.transfer_cursor)
                .map(|t| t.final_path.clone().unwrap_or_else(|| t.spec.destination())),
        };
        if let Some(text) = text {
            self.requests.push(Request::Copy(text));
        }
    }

    pub fn handle_message(&mut self, message: Message) {
        match message {
            Message::Session { epoch, message } => {
                if epoch == self.session_epoch {
                    self.handle_message(*message);
                }
            }
            Message::Authenticated { id, result } if id == self.auth_request => {
                let previous = self.overlay.take();
                match result {
                    Ok(authorized) => self.accept_account(authorized),
                    Err(error) => {
                        if let Some(Overlay::Busy { mut form, .. }) = previous {
                            if !form.fields.is_empty() {
                                form.error = Some(error.clone());
                                self.overlay = Some(Overlay::Form(*form));
                            } else {
                                self.overlay = Some(Overlay::Accounts { selected: 0 });
                            }
                        }
                        self.toast(error, true);
                    }
                }
            }
            Message::SignedOut { id, result } if id == self.auth_request => match result {
                Ok(settings) => {
                    let language = self.language();
                    self.reset_account();
                    self.settings = settings;
                    self.settings.language = language;
                    self.signed_in = false;
                    self.workspace = Workspace::Settings;
                    self.requests.push(Request::UseSession {
                        epoch: self.session_epoch,
                        token: None,
                    });
                    self.toast("Signed out", false);
                }
                Err(error) => {
                    self.overlay = None;
                    self.toast(error, true);
                }
            },
            Message::LanguageSaved { language, result } => {
                if let Err(error) = result {
                    self.toast(error, true);
                } else if language == self.language() {
                    self.toast("Settings saved", false);
                }
            }
            Message::Account(Ok(account)) => self.account = Some(account),
            Message::Account(Err(error)) => {
                self.toast(format!("Account information unavailable: {error}"), true)
            }
            Message::Repositories { id, result } if id == self.repo_generation => {
                self.repos_loading = false;
                match result {
                    Ok(repos) => {
                        self.connected = true;
                        self.repos = repos;
                        self.library_cursor =
                            self.library_cursor.min(self.repos.len().saturating_sub(1));
                        if let Some(remote) = self.initial_remote.take() {
                            match RemoteRef::parse_list_target(&remote, None) {
                                Ok(remote) => {
                                    if let Some(repo) =
                                        self.repos.iter().find(|r| r.name == remote.repo)
                                    {
                                        self.navigate(Location {
                                            repo: repo.name.clone(),
                                            repo_id: repo.id.clone(),
                                            path: normalized_path(&remote.path),
                                        });
                                    } else {
                                        self.toast(format!("Library '{}' was not found. Choose one from the library list.", remote.repo), true);
                                        self.focus = Focus::Libraries;
                                    }
                                }
                                Err(err) => {
                                    self.toast(err.to_string(), true);
                                    self.focus = Focus::Libraries;
                                }
                            }
                        } else if self.location.is_none() && !self.repos.is_empty() {
                            self.open_library();
                        } else if let Some(location) = &mut self.location {
                            if let Some(index) =
                                self.repos.iter().position(|r| r.id == location.repo_id)
                            {
                                location.repo = self.repos[index].name.clone();
                                self.library_cursor = index;
                            } else {
                                self.location = None;
                                self.entries.clear();
                                self.selected.clear();
                                self.generation = self.id();
                                self.open_library();
                            }
                        }
                    }
                    Err(error) => {
                        self.connected = false;
                        self.load_error = Some(error.clone());
                        self.toast(error, true);
                    }
                }
            }
            Message::Directory { id, result } if id == self.generation => {
                self.loading = false;
                match result {
                    Ok(entries) => {
                        self.entries = entries;
                        self.cursor = self.cursor.min(self.file_indices().len().saturating_sub(1));
                    }
                    Err(error) => self.load_error = Some(error),
                }
            }
            Message::PathMatches { id, result } => {
                if let Some(Overlay::Picker(picker)) = &mut self.overlay {
                    if picker.generation == id {
                        picker.loading = false;
                        match result {
                            Ok(matches) => {
                                let browse_target = picker.browse_target.take();
                                if browse_target.is_some() {
                                    picker.cwd = matches.root.clone();
                                    picker.input = Editor::default();
                                }
                                picker.entries = matches.entries;
                                if picker.directories_only() {
                                    if let Some(directory) = matches.directory {
                                        picker.entries.insert(
                                            0,
                                            LocalEntry {
                                                name: ".".into(),
                                                path: directory,
                                                is_dir: true,
                                                size: None,
                                            },
                                        );
                                    }
                                }
                                picker.match_root = matches.root;
                                picker.pattern = matches.pattern;
                                picker.truncated = matches.truncated;
                                if let Some(target) = browse_target {
                                    picker.selected = picker
                                        .entries
                                        .iter()
                                        .position(|entry| entry.path == target)
                                        .unwrap_or(0);
                                }
                            }
                            Err(error) => picker.error = Some(error),
                        }
                    }
                }
            }
            Message::Shares { id, result } if id == self.share_generation => {
                self.shares_loading = false;
                match result {
                    Ok(shares) => {
                        self.shares = shares;
                        self.shares_loaded = true;
                        self.share_cursor = self
                            .share_cursor
                            .min(self.share_indices().len().saturating_sub(1));
                    }
                    Err(error) => {
                        self.shares_error = Some(error.clone());
                        self.toast(error, true);
                    }
                }
            }
            Message::Operation { id, result } => {
                let previous = if matches!(self.overlay, Some(Overlay::Busy { id: active, .. }) if active == id)
                {
                    self.overlay.take()
                } else {
                    None
                };
                match result {
                    Ok(Operation::Shared(share)) => {
                        self.overlay = Some(Overlay::ShareReady(share));
                        self.shares_loaded = false;
                    }
                    Ok(Operation::RepositoryCreated(name)) => {
                        self.toast(format!("Library {name} is ready"), false);
                        self.initial_remote = Some(name);
                        self.refresh_repos();
                    }
                    Ok(Operation::DirectoryCreated) => {
                        self.toast("Folder created", false);
                        if let Some(location) = self.location.clone() {
                            self.navigate(location);
                        }
                    }
                    Ok(Operation::Revoked) => {
                        self.toast("Share link revoked", false);
                        self.refresh_shares();
                    }
                    Err(error) => {
                        if let Some(Overlay::Busy { mut form, .. }) = previous {
                            if !form.fields.is_empty() {
                                form.error = Some(error.clone());
                                self.overlay = Some(Overlay::Form(*form));
                            }
                        }
                        self.toast(error, true);
                    }
                }
            }
            Message::Progress {
                id,
                transferred,
                total,
                warning,
            } => {
                if let Some(t) = self
                    .transfers
                    .iter_mut()
                    .find(|t| t.id == id && t.status == TransferStatus::Running)
                {
                    t.transferred = transferred;
                    t.total = total;
                    if let Some(warning) = warning {
                        t.detail = warning;
                    }
                }
            }
            Message::TransferFinished {
                id,
                result,
                cancelled,
            } => {
                if let Some(t) = self.transfers.iter_mut().find(|t| t.id == id) {
                    t.finished = Some(Instant::now());
                    if cancelled {
                        t.status = TransferStatus::Cancelled;
                        t.detail = "Cancelled. Partial downloads can resume on retry.".into();
                    } else {
                        match result {
                            Ok(outcome) => {
                                t.status = TransferStatus::Done;
                                t.transferred = outcome.bytes;
                                t.total = Some(outcome.bytes);
                                t.final_path = Some(outcome.path);
                                t.detail = if outcome.uniquified {
                                    "Saved with a unique name to avoid overwriting".into()
                                } else {
                                    "Transfer complete".into()
                                };
                            }
                            Err(error) => {
                                t.status = TransferStatus::Failed;
                                t.detail = error;
                            }
                        }
                    }
                    let upload_done = matches!(t.spec, TransferSpec::Upload { .. })
                        && t.status == TransferStatus::Done;
                    let name = t.spec.name();
                    let failed = t.status == TransferStatus::Failed;
                    let status = t.status.label();
                    self.toast(format!("{name} · {status}"), failed);
                    if upload_done {
                        if let Some(location) = self.location.clone() {
                            self.navigate(location);
                        }
                    }
                }
                if self.quitting {
                    self.should_quit = self.active_transfers() == 0;
                } else {
                    self.start_next();
                }
            }
            Message::Copied(result) => match result {
                Ok(()) => self.toast("Copied to clipboard", false),
                Err(error) => self.toast(error, true),
            },
            _ => {}
        }
    }
}

#[cfg(test)]
pub fn palette_matches(query: &str) -> Vec<usize> {
    palette_matches_in(query, Language::English)
}

pub fn palette_matches_in(query: &str, language: Language) -> Vec<usize> {
    let query = query.trim().trim_start_matches('/');
    let mut matches: Vec<_> = COMMANDS
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
            let localized = language.tr(command.name);
            let score = [command.name, localized.as_ref()]
                .iter()
                .filter_map(|name| {
                    if name.eq_ignore_ascii_case(query) {
                        Some(0)
                    } else {
                        fuzzy_score(name, query).map(|score| score + 1)
                    }
                })
                .min()
                .or_else(|| {
                    fuzzy_score(
                        &format!(
                            "{} {}",
                            command.description,
                            language.tr(command.description)
                        ),
                        query,
                    )
                    .map(|score| score + 250)
                });
            score.map(|score| (index, score))
        })
        .collect();
    matches.sort_by_key(|&(index, score)| (score, index));
    matches.into_iter().map(|(index, _)| index).collect()
}

pub fn fuzzy_score(text: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    let text = text.to_lowercase();
    let query = query.to_lowercase();
    if let Some(index) = text.find(&query) {
        return Some(index);
    }
    let mut chars = text.chars().enumerate();
    let mut score = 100;
    let mut last = 0;
    for needle in query.chars() {
        let (index, _) = chars.find(|(_, ch)| *ch == needle)?;
        score += index.saturating_sub(last);
        last = index;
    }
    Some(score)
}

pub fn normalized_path(path: &str) -> String {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}

pub fn expand_path(input: &str) -> PathBuf {
    if input == "~" || input.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            return PathBuf::from(home).join(input.strip_prefix("~/").unwrap_or(""));
        }
    }
    PathBuf::from(input)
}

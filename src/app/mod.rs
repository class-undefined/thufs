pub mod account_service;
pub mod auth_service;
pub mod list_service;
pub mod pull_service;
pub mod push_service;
pub mod share_service;

use crate::{config::ConfigManager, output::Renderer, seafile::SeafileClient};

use self::account_service::AccountService;
use self::auth_service::AuthService;
use self::list_service::ListService;
use self::pull_service::PullService;
use self::push_service::PushService;
use self::share_service::ShareService;

pub struct App {
    pub renderer: Renderer,
    pub account_service: AccountService,
    pub auth_service: AuthService,
    pub list_service: ListService,
    pub push_service: PushService,
    pub pull_service: PullService,
    pub share_service: ShareService,
}

impl App {
    pub fn new() -> Self {
        let config = ConfigManager::new();
        let seafile = SeafileClient::new(config.clone());

        Self {
            renderer: Renderer::new(),
            account_service: AccountService::new(seafile.clone()),
            auth_service: AuthService::new(config.clone()),
            list_service: ListService::new(seafile.clone()),
            push_service: PushService::new(config.clone(), seafile.clone()),
            pull_service: PullService::new(config.clone(), seafile.clone()),
            share_service: ShareService::new(config.clone(), seafile.clone()),
        }
    }
}

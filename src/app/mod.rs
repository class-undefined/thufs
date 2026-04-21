pub mod auth_service;
pub mod list_service;
pub mod pull_service;
pub mod push_service;

use crate::{config::ConfigManager, output::Renderer, seafile::SeafileClient};

use self::auth_service::AuthService;
use self::list_service::ListService;
use self::pull_service::PullService;
use self::push_service::PushService;

pub struct App {
    pub renderer: Renderer,
    pub auth_service: AuthService,
    pub list_service: ListService,
    pub push_service: PushService,
    pub pull_service: PullService,
}

impl App {
    pub fn new() -> Self {
        let config = ConfigManager::new();
        let seafile = SeafileClient::new(config.clone());

        Self {
            renderer: Renderer::new(),
            auth_service: AuthService::new(config.clone()),
            list_service: ListService::new(seafile.clone()),
            push_service: PushService::new(config.clone(), seafile.clone()),
            pull_service: PullService::new(config.clone(), seafile.clone()),
        }
    }
}

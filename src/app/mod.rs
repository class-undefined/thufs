pub mod auth_service;

#[allow(dead_code)]
use crate::{config::ConfigManager, output::Renderer, seafile::SeafileClient};

use self::auth_service::AuthService;

pub struct App {
    pub renderer: Renderer,
    pub auth_service: AuthService,
    pub seafile: SeafileClient,
}

impl App {
    pub fn new() -> Self {
        let config = ConfigManager::new();
        let seafile = SeafileClient::new(config.clone());

        Self {
            renderer: Renderer::new(),
            auth_service: AuthService::new(config),
            seafile,
        }
    }
}

pub mod auth_service;
pub mod list_service;

#[allow(dead_code)]
use crate::{config::ConfigManager, output::Renderer, seafile::SeafileClient};

use self::auth_service::AuthService;
use self::list_service::ListService;

pub struct App {
    pub renderer: Renderer,
    pub auth_service: AuthService,
    #[allow(dead_code)]
    pub seafile: SeafileClient,
    pub list_service: ListService,
}

impl App {
    pub fn new() -> Self {
        let config = ConfigManager::new();
        let seafile = SeafileClient::new(config.clone());

        Self {
            renderer: Renderer::new(),
            auth_service: AuthService::new(config),
            seafile: seafile.clone(),
            list_service: ListService::new(seafile),
        }
    }
}

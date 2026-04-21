pub mod auth_service;

use crate::{config::ConfigManager, output::Renderer};

use self::auth_service::AuthService;

pub struct App {
    pub renderer: Renderer,
    pub auth_service: AuthService,
}

impl App {
    pub fn new() -> Self {
        let config = ConfigManager::new();

        Self {
            renderer: Renderer::new(),
            auth_service: AuthService::new(config),
        }
    }
}

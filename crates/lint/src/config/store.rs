extern crate rustc_session;

use std::sync::OnceLock;

use rustc_session::Session;

use super::assemble::Config;

/// The configuration shared by every registered lint pass.
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Owns the process-wide configuration initialized before lint registration.
pub struct ConfigStore;

impl ConfigStore {
    /// Loads and validates configuration, reporting fatal diagnostics through rustc.
    pub fn initialize(session: &Session) {
        let config = Config::load().unwrap_or_else(|message| {
            session
                .dcx()
                .fatal(format!("invalid rlib configuration: {message}"))
        });
        if CONFIG.set(config).is_err() {
            session
                .dcx()
                .fatal("rlib configuration was initialized more than once");
        }
    }

    /// Returns the configuration initialized during root lint registration.
    pub fn get() -> &'static Config {
        CONFIG
            .get()
            .expect("rlib configuration must be initialized before lint registration")
    }
}

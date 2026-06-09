mod agent;
mod config;
pub mod prompt;

pub use agent::FundamentalCatalogManagerAgent;
pub use config::FundamentalCatalogManagerConfig;

pub const WORKER_NAME: &str = "fundamental_catalog_manager";

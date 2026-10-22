pub mod combat;
pub mod unit;
pub mod commands;
pub mod config;

pub struct Data {
    pub config: config::Config,
}
pub type Error = Box<dyn std::error::Error + Send + Sync>;

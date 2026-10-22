use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeathMessage {
    pub message: String,
    pub credited_to: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    admin_user_id: String,
    #[serde(default)]
    death_messages: Vec<DeathMessage>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub death_messages: Arc<RwLock<Vec<DeathMessage>>>,
    pub admin_user_id: u64,
    config_path: PathBuf,
}

impl Config {
    /// Loads optional `config.json` from the working directory. A missing file
    /// gives defaults: no admin (admin-only commands disabled), no death messages.
    pub fn load() -> Self {
        let config_path = std::env::current_dir()
            .unwrap_or_default()
            .join("config.json");

        let config_file: ConfigFile = match fs::read_to_string(&config_path) {
            Ok(s) => serde_json::from_str(&s).expect("Failed to parse config.json"),
            Err(_) => ConfigFile::default(),
        };

        let admin_id = config_file.admin_user_id.parse::<u64>().unwrap_or(0);

        println!(
            "Loaded config: {} death messages, admin: {}",
            config_file.death_messages.len(),
            admin_id
        );

        Config {
            death_messages: Arc::new(RwLock::new(config_file.death_messages)),
            admin_user_id: admin_id,
            config_path,
        }
    }

    pub async fn add_death_message(&self, message: String, credited_to: String) -> bool {
        let mut messages = self.death_messages.write().await;
        if messages.iter().any(|m| m.message == message) {
            return false;
        }
        messages.push(DeathMessage { message, credited_to });
        self.save_to_disk(&messages);
        true
    }

    pub async fn remove_death_message(&self, message: &str) -> bool {
        let mut messages = self.death_messages.write().await;
        let before = messages.len();
        messages.retain(|m| m.message != message);
        let removed = messages.len() < before;
        if removed {
            self.save_to_disk(&messages);
        }
        removed
    }

    pub async fn list_death_messages(&self) -> Vec<DeathMessage> {
        self.death_messages.read().await.clone()
    }

    pub async fn get_random_death_message(&self) -> String {
        let messages = self.death_messages.read().await;
        if messages.is_empty() {
            return "[killed]".to_string();
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        let idx = (hasher.finish() as usize) % messages.len();
        format!("[{}]", messages[idx].message)
    }

    fn save_to_disk(&self, death_messages: &[DeathMessage]) {
        let config_file = ConfigFile {
            admin_user_id: self.admin_user_id.to_string(),
            death_messages: death_messages.to_vec(),
        };

        let json = serde_json::to_string_pretty(&config_file)
            .expect("Failed to serialize config");

        fs::write(&self.config_path, json).expect("Failed to write config.json");
    }
}

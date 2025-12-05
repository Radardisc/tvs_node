use serde::{Deserialize, Serialize};
use tfs_http::app_config::AppConfig;

/// Unified configuration for TVS Node
/// Combines TFS HTTP configuration with TVS-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvsNodeConfig {
    /// TFS HTTP server configuration (ports, node info, logging, etc.)
    #[serde(flatten)]
    pub tfs: AppConfig,

    /// TVS vote server configuration (optional)
    pub tvs: Option<TvsServerConfig>,
}

/// Configuration for the TVS vote server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvsServerConfig {
    /// Port for the vote server (default: 8090)
    #[serde(default = "default_vote_port")]
    pub vote_port: u16,

    /// Host for the vote server (default: "127.0.0.1")
    #[serde(default = "default_vote_host")]
    pub vote_host: String,

    /// Enable the vote server (default: true if tvs section exists)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_vote_port() -> u16 {
    8090
}

fn default_vote_host() -> String {
    "127.0.0.1".to_string()
}

fn default_enabled() -> bool {
    true
}

impl Default for TvsServerConfig {
    fn default() -> Self {
        Self {
            vote_port: default_vote_port(),
            vote_host: default_vote_host(),
            enabled: default_enabled(),
        }
    }
}

#[allow(dead_code)]
impl TvsNodeConfig {
    /// Read configuration from a JSON file
    pub fn read_config(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_content = std::fs::read_to_string(config_path)?;
        let config: TvsNodeConfig = serde_json::from_str(&config_content)?;
        Ok(config)
    }

    /// Override config values with environment variables
    /// This allows Docker containers to override config.json settings via env vars
    pub fn apply_env_overrides(&mut self) {
        // TFS server ports
        if let Ok(port) = std::env::var("CLUSTER_MESSAGE_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                self.tfs.server.cluster_message_port = p;
            }
        }

        if let Ok(port) = std::env::var("APP_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                self.tfs.server.app_port = p;
            }
        }

        if let Ok(port) = std::env::var("ADMIN_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                self.tfs.server.admin_port = p;
            }
        }

        // TVS vote server configuration
        if let Some(ref mut tvs) = self.tvs {
            if let Ok(host) = std::env::var("TVS_VOTE_HOST") {
                tvs.vote_host = host;
            }

            if let Ok(port) = std::env::var("TVS_VOTE_PORT") {
                if let Ok(p) = port.parse::<u16>() {
                    tvs.vote_port = p;
                }
            }
        }

        // Node identification
        if let Ok(name) = std::env::var("NODE_NAME") {
            self.tfs.node_name = Some(name);
        }
    }

    /// Get the TFS app config
    pub fn tfs_config(&self) -> &AppConfig {
        &self.tfs
    }

    /// Get the TVS server config, or None if disabled
    pub fn tvs_config(&self) -> Option<&TvsServerConfig> {
        self.tvs.as_ref().filter(|c| c.enabled)
    }

    /// Check if TVS vote server should be started
    pub fn should_start_vote_server(&self) -> bool {
        self.tvs_config().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tvs_config() {
        let config = TvsServerConfig::default();
        assert_eq!(config.vote_port, 8090);
        assert_eq!(config.vote_host, "127.0.0.1");
        assert!(config.enabled);
    }

    #[test]
    fn test_tvs_config_parsing() {
        let json = r#"{
            "server": {
                "cluster_message_port": 8080,
                "app_port": 8081,
                "admin_port": 8082
            },
            "node_name": "test_node",
            "tvs": {
                "vote_port": 9000,
                "vote_host": "0.0.0.0",
                "enabled": true
            }
        }"#;

        let config: TvsNodeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.tfs.node_name, Some("test_node".to_string()));

        let tvs = config.tvs_config().unwrap();
        assert_eq!(tvs.vote_port, 9000);
        assert_eq!(tvs.vote_host, "0.0.0.0");
    }

    #[test]
    fn test_tvs_config_disabled() {
        let json = r#"{
            "server": {
                "cluster_message_port": 8080,
                "app_port": 8081,
                "admin_port": 8082
            },
            "tvs": {
                "enabled": false
            }
        }"#;

        let config: TvsNodeConfig = serde_json::from_str(json).unwrap();
        assert!(config.tvs_config().is_none());
        assert!(!config.should_start_vote_server());
    }

    #[test]
    fn test_tvs_config_missing() {
        let json = r#"{
            "server": {
                "cluster_message_port": 8080,
                "app_port": 8081,
                "admin_port": 8082
            }
        }"#;

        let config: TvsNodeConfig = serde_json::from_str(json).unwrap();
        assert!(config.tvs_config().is_none());
        assert!(!config.should_start_vote_server());
    }

    #[test]
    fn test_tvs_config_defaults() {
        let json = r#"{
            "server": {
                "cluster_message_port": 8080,
                "app_port": 8081,
                "admin_port": 8082
            },
            "tvs": {}
        }"#;

        let config: TvsNodeConfig = serde_json::from_str(json).unwrap();
        let tvs = config.tvs_config().unwrap();
        assert_eq!(tvs.vote_port, 8090);
        assert_eq!(tvs.vote_host, "127.0.0.1");
        assert!(tvs.enabled);
    }
}

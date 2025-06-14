use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub remote_ip: String,
    pub remote_port: u16,
    pub screen: Screen,
    pub remote_screen: Screen,
    pub host_position: HostPosition,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HostPosition {
    Left,
    Right,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn create_template<P: AsRef<Path>>(path: P) -> Result<()> {
        let template = Config {
            remote_ip: "192.168.1.100".to_string(),
            remote_port: 5000,
            screen: Screen {
                width: 2600,
                height: 1440,
            },
            remote_screen: Screen {
                width: 1920,
                height: 1080,
            },
            host_position: HostPosition::Left,
        };

        let yaml = serde_yaml::to_string(&template)?;
        fs::write(path, yaml)?;
        Ok(())
    }
}

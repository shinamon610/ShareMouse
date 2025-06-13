use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use anyhow::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub mode: Mode,
    pub remote_ip: String,
    pub remote_port: u16,
    pub screen: Screen,
    pub remote_screen: Screen,
    pub layout: Layout,
    pub edge: Edge,
    pub protocol: Protocol,
    pub buffer_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Sender,
    Receiver,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Edge {
    pub sender_to_receiver: EdgeDirection,
    pub receiver_to_sender: EdgeDirection,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeDirection {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Udp,
    Tcp,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layout {
    pub position: Position,
    pub remote_position: Position,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Left,
    Right,
    Top,
    Bottom,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
    
    pub fn create_template<P: AsRef<Path>>(path: P) -> Result<()> {
        let template = Config {
            mode: Mode::Sender,
            remote_ip: "100.64.1.79".to_string(),
            remote_port: 5000,
            screen: Screen {
                width: 2600,
                height: 1440,
            },
            remote_screen: Screen {
                width: 1920,
                height: 1080,
            },
            layout: Layout {
                position: Position::Left,
                remote_position: Position::Right,
            },
            edge: Edge {
                sender_to_receiver: EdgeDirection::Right,
                receiver_to_sender: EdgeDirection::Left,
            },
            protocol: Protocol::Udp,
            buffer_size: 4096,
        };
        
        let yaml = serde_yaml::to_string(&template)?;
        fs::write(path, yaml)?;
        Ok(())
    }
}
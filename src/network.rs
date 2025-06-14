use crate::capturer::MouseEvent;
use crate::config::Config;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMouseEvent {
    pub x: f64,
    pub y: f64,
    pub delta_x: Option<f64>,
    pub delta_y: Option<f64>,
    pub event_type: String,
}

impl From<MouseEvent> for NetworkMouseEvent {
    fn from(event: MouseEvent) -> Self {
        Self {
            x: event.x,
            y: event.y,
            delta_x: event.delta_x,
            delta_y: event.delta_y,
            event_type: format!("{:?}", event.event_type),
        }
    }
}

impl From<NetworkMouseEvent> for MouseEvent {
    fn from(net_event: NetworkMouseEvent) -> Self {
        use crate::capturer::MouseEventType;

        let event_type = match net_event.event_type.as_str() {
            "Move" => MouseEventType::Move,
            "LeftClick" => MouseEventType::LeftClick,
            "LeftRelease" => MouseEventType::LeftRelease,
            "RightClick" => MouseEventType::RightClick,
            "RightRelease" => MouseEventType::RightRelease,
            "MiddleClick" => MouseEventType::MiddleClick,
            "MiddleRelease" => MouseEventType::MiddleRelease,
            "ScrollUp" => MouseEventType::ScrollUp,
            "ScrollDown" => MouseEventType::ScrollDown,
            _ => MouseEventType::Move,
        };

        Self {
            x: net_event.x,
            y: net_event.y,
            delta_x: net_event.delta_x,
            delta_y: net_event.delta_y,
            event_type,
        }
    }
}

pub struct NetworkSender {
    config: Config,
}

impl NetworkSender {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn start(&self, mut receiver: mpsc::UnboundedReceiver<MouseEvent>) -> Result<()> {
        let remote_addr: SocketAddr =
            format!("{}:{}", self.config.remote_ip, self.config.remote_port).parse()?;
        log::info!("NetworkSender starting, will send to {}", remote_addr);

        // senderは常にUDP
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let local_addr = socket.local_addr()?;
        log::info!(
            "UDP socket bound to {}, will send to {}",
            local_addr,
            remote_addr
        );

        while let Some(event) = receiver.recv().await {
            log::info!(
                "NetworkSender received event: {:?} at ({}, {})",
                event.event_type,
                event.x,
                event.y
            );
            let net_event = NetworkMouseEvent::from(event);
            let data = bincode::serialize(&net_event)?;
            match socket.send_to(&data, remote_addr).await {
                Ok(bytes_sent) => {
                    log::info!("Sent {} bytes to {}", bytes_sent, remote_addr);
                }
                Err(e) => {
                    log::error!("Failed to send to {}: {}", remote_addr, e);
                }
            }
        }

        Ok(())
    }
}

pub struct NetworkReceiver {
    port: u16,
}

impl NetworkReceiver {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(&self, sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()> {
        let bind_addr: SocketAddr = format!("0.0.0.0:{}", self.port).parse()?;

        // receiverは常にUDP、固定バッファサイズ
        let socket = UdpSocket::bind(bind_addr).await?;
        let mut buf = vec![0u8; 4096];

        log::info!("UDP receiver listening on {}", bind_addr);
        loop {
            let (len, addr) = socket.recv_from(&mut buf).await?;
            log::debug!("Received {} bytes from {}", len, addr);
            log::debug!("Raw bytes: {:?}", &buf[..len]);
            match bincode::deserialize::<NetworkMouseEvent>(&buf[..len]) {
                Ok(net_event) => {
                    log::debug!("Parsed event: {:?}", net_event);
                    let event = MouseEvent::from(net_event);
                    let _ = sender.send(event);
                }
                Err(e) => {
                    log::warn!("Failed to deserialize network event: {}", e);
                    log::debug!(
                        "Attempting to deserialize as string: {:?}",
                        String::from_utf8_lossy(&buf[..len])
                    );
                }
            }
        }
    }
}

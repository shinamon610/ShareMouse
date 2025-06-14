use crate::config::Config;
use crate::event::MouseEvent;
use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;

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
            log::info!("NetworkSender received event: {:?}", event);
            let data = bincode::serialize(&event)?;
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
            match bincode::deserialize::<MouseEvent>(&buf[..len]) {
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

use anyhow::Result;
use tokio::net::{UdpSocket, TcpListener, TcpStream};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use crate::capturer::MouseEvent;
use crate::config::{Config, Protocol};
use std::net::SocketAddr;

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
        let remote_addr: SocketAddr = format!("{}:{}", self.config.remote_ip, self.config.remote_port).parse()?;
        log::info!("NetworkSender starting, will send to {}", remote_addr);
        
        match self.config.protocol {
            Protocol::Udp => {
                let socket = UdpSocket::bind("0.0.0.0:0").await?;
                let local_addr = socket.local_addr()?;
                log::info!("UDP socket bound to {}, will send to {}", local_addr, remote_addr);
                
                while let Some(event) = receiver.recv().await {
                    log::info!("NetworkSender received event: {:?} at ({}, {})", event.event_type, event.x, event.y);
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
            }
            Protocol::Tcp => {
                let mut stream = TcpStream::connect(remote_addr).await?;
                
                while let Some(event) = receiver.recv().await {
                    let net_event = NetworkMouseEvent::from(event);
                    let data = bincode::serialize(&net_event)?;
                    
                    use tokio::io::AsyncWriteExt;
                    let len = data.len() as u32;
                    stream.write_all(&len.to_be_bytes()).await?;
                    stream.write_all(&data).await?;
                }
            }
        }
        
        Ok(())
    }
}

pub struct NetworkReceiver {
    config: Config,
}

impl NetworkReceiver {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
    
    pub async fn start(&self, sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()> {
        let bind_addr: SocketAddr = format!("0.0.0.0:{}", self.config.remote_port).parse()?;
        
        match self.config.protocol {
            Protocol::Udp => {
                let socket = UdpSocket::bind(bind_addr).await?;
                let mut buf = vec![0u8; self.config.buffer_size];
                
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
                            log::debug!("Attempting to deserialize as string: {:?}", 
                                      String::from_utf8_lossy(&buf[..len]));
                        }
                    }
                }
            }
            Protocol::Tcp => {
                let listener = TcpListener::bind(bind_addr).await?;
                
                loop {
                    let (mut stream, _addr) = listener.accept().await?;
                    let sender = sender.clone();
                    
                    tokio::spawn(async move {
                        use tokio::io::AsyncReadExt;
                        let mut len_buf = [0u8; 4];
                        
                        loop {
                            if stream.read_exact(&mut len_buf).await.is_err() {
                                break;
                            }
                            
                            let len = u32::from_be_bytes(len_buf) as usize;
                            let mut data_buf = vec![0u8; len];
                            
                            if stream.read_exact(&mut data_buf).await.is_err() {
                                break;
                            }
                            
                            if let Ok(net_event) = bincode::deserialize::<NetworkMouseEvent>(&data_buf) {
                                let event = MouseEvent::from(net_event);
                                let _ = sender.send(event);
                            }
                        }
                    });
                }
            }
        }
    }
}
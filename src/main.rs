use clap::{Parser, Subcommand};
use log::{info, error};
use std::path::PathBuf;
use capturer::MouseCapturer;
use injector::MouseInjector;

mod config;
mod capturer;
mod injector;
mod network;

#[derive(Parser)]
#[command(name = "sharemouse")]
#[command(about = "A lightweight mouse sharing tool for macOS and Linux")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,
    
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    Start,
    Stop,
    Status,
    Template,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log_level))
        .init();
    
    match cli.command {
        Commands::Start => {
            info!("Starting ShareMouse...");
            let config = config::Config::load(&cli.config)?;
            start_service(config).await?;
        }
        Commands::Stop => {
            info!("Stopping ShareMouse...");
        }
        Commands::Status => {
            info!("ShareMouse status");
        }
        Commands::Template => {
            config::Config::create_template(&cli.config)?;
            info!("Template config created at {:?}", cli.config);
        }
    }
    
    Ok(())
}

async fn start_service(config: config::Config) -> anyhow::Result<()> {
    match config.mode {
        config::Mode::Sender => {
            info!("Starting in sender mode");
            start_sender(config).await?;
        }
        config::Mode::Receiver => {
            info!("Starting in receiver mode");
            start_receiver(config).await?;
        }
    }
    Ok(())
}

async fn start_sender(config: config::Config) -> anyhow::Result<()> {
    use tokio::sync::mpsc;
    
    let (capture_tx, capture_rx) = mpsc::unbounded_channel();
    let (network_tx, network_rx) = mpsc::unbounded_channel();
    
    #[cfg(target_os = "macos")]
    let capturer = capturer::macos::MacOSCapturer::new();
    
    #[cfg(target_os = "linux")]
    let capturer = capturer::linux::LinuxCapturer::new("/dev/input/event0");
    
    let edge_detector = EdgeDetector::new(config.clone());
    let network_sender = network::NetworkSender::new(config.clone());
    
    tokio::spawn(async move {
        if let Err(e) = capturer.start_capture(capture_tx).await {
            error!("Capture error: {}", e);
        }
    });
    
    tokio::spawn(async move {
        edge_detector.process_events(capture_rx, network_tx).await;
    });
    
    network_sender.start(network_rx).await?;
    
    Ok(())
}

async fn start_receiver(config: config::Config) -> anyhow::Result<()> {
    use tokio::sync::mpsc;
    
    let (network_tx, mut network_rx) = mpsc::unbounded_channel();
    
    #[cfg(target_os = "macos")]
    let injector = injector::macos::MacOSInjector::new()?;
    
    #[cfg(target_os = "linux")]
    let injector = injector::linux::LinuxInjector::new()?;
    
    let network_receiver = network::NetworkReceiver::new(config.clone());
    
    tokio::spawn(async move {
        if let Err(e) = network_receiver.start(network_tx).await {
            error!("Network receiver error: {}", e);
        }
    });
    
    while let Some(event) = network_rx.recv().await {
        if let Err(e) = injector.inject_event(event) {
            error!("Injection error: {}", e);
        }
    }
    
    Ok(())
}

struct EdgeDetector {
    config: config::Config,
}

impl EdgeDetector {
    fn new(config: config::Config) -> Self {
        Self { config }
    }
    
    async fn process_events(
        &self,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<capturer::MouseEvent>,
        sender: tokio::sync::mpsc::UnboundedSender<capturer::MouseEvent>,
    ) {
        while let Some(event) = receiver.recv().await {
            if self.should_send_event(&event) {
                let _ = sender.send(event);
            }
        }
    }
    
    fn should_send_event(&self, event: &capturer::MouseEvent) -> bool {
        use crate::config::EdgeDirection;
        
        match self.config.edge.sender_to_receiver {
            EdgeDirection::Right => event.x >= (self.config.screen.width - 1) as f64,
            EdgeDirection::Left => event.x <= 0.0,
            EdgeDirection::Top => event.y <= 0.0,
            EdgeDirection::Bottom => event.y >= (self.config.screen.height - 1) as f64,
        }
    }
}

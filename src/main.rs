use capturer::MouseCapturer;
use clap::{Parser, Subcommand};
use injector::MouseInjector;
use log::{error, info};
use std::path::PathBuf;

mod capturer;
mod config;
mod coordinate;
mod event;
mod injector;
mod network;
mod virtual_model;

use virtual_model::{SharedVirtualModel, VirtualModel};
#[derive(Parser)]
#[command(name = "sharemouse")]
#[command(about = "A lightweight mouse sharing tool for macOS and Linux")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    Send {
        #[arg(short, long, default_value = "config.yaml")]
        config: PathBuf,
    },
    Receive {
        #[arg(short, long, default_value = "5000")]
        port: u16,
    },
    Template {
        #[arg(short, long, default_value = "config.yaml")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log_level))
        .init();

    match cli.command {
        Commands::Send { config } => {
            info!("Starting Sending");
            let config = config::Config::load(&config)?;
            start_sender(config).await?;
        }
        Commands::Receive { port } => {
            info!("Start Receiving on port {}", port);
            start_receiver(port).await?;
        }
        Commands::Template { config } => {
            config::Config::create_template(&config)?;
            info!("Template config created at {:?}", config);
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
async fn start_sender(config: config::Config) -> anyhow::Result<()> {
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;

    let virtual_model: SharedVirtualModel = Arc::new(Mutex::new(VirtualModel::new(config.clone())));

    let (network_tx, network_rx) = mpsc::unbounded_channel();

    let capturer = capturer::macos::MacOSCapturer::new();

    let network_sender = network::NetworkSender::new(config.clone());

    tokio::spawn(async move {
        if let Err(e) = capturer
            .start_capture_with_model(network_tx, virtual_model)
            .await
        {
            error!("Capture error: {}", e);
        }
    });

    network_sender.start(network_rx).await?;

    Ok(())
}

#[cfg(target_os = "linux")]
async fn start_sender(_: config::Config) -> anyhow::Result<()> {
    todo!()
}

#[cfg(target_os = "macos")]
async fn start_receiver(_: u16) -> anyhow::Result<()> {
    todo!()
}

#[cfg(target_os = "linux")]
async fn start_receiver(port: u16) -> anyhow::Result<()> {
    use tokio::sync::mpsc;

    let (network_tx, mut network_rx) = mpsc::unbounded_channel();

    let mut injector = injector::linux::LinuxInjector::new()?;

    let network_receiver = network::NetworkReceiver::new(port);

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

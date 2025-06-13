use clap::{Parser, Subcommand};
use log::{info, error};
use std::path::PathBuf;
use capturer::MouseCapturer;
use injector::MouseInjector;

mod config;
mod capturer;
mod injector;
mod network;
mod coordinate;
mod virtual_mouse;

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
    use std::sync::{Arc, Mutex};
    use virtual_mouse::{VirtualMouse, SharedVirtualMouse};
    use coordinate::CoordinateTransformer;
    
    let (capture_tx, capture_rx) = mpsc::unbounded_channel();
    let (network_tx, network_rx) = mpsc::unbounded_channel();
    // 仮想マウス状態を初期化
    let virtual_mouse: SharedVirtualMouse = Arc::new(Mutex::new(VirtualMouse::new(&config)));
    
    #[cfg(target_os = "macos")]
    let capturer = capturer::macos::MacOSCapturer::new();
    
    #[cfg(target_os = "linux")]
    let capturer = capturer::linux::LinuxCapturer::new("/dev/input/event0", config.screen.width, config.screen.height);
    
    let virtual_mouse_processor = VirtualMouseProcessor::new(config.clone(), virtual_mouse.clone());
    let network_sender = network::NetworkSender::new(config.clone());
    
    // 物理マウスキャプチャ
    tokio::spawn(async move {
        if let Err(e) = capturer.start_capture(capture_tx).await {
            error!("Capture error: {}", e);
        }
    });
    
    // 仮想マウス処理（物理マウス → 仮想座標更新 → 制御判定）
    // 注入は別スレッドでなく同期処理で行う
    tokio::spawn(async move {
        virtual_mouse_processor.process_events(capture_rx, network_tx).await;
    });
    
    // ネットワーク送信
    network_sender.start(network_rx).await?;
    
    Ok(())
}

async fn start_receiver(config: config::Config) -> anyhow::Result<()> {
    use tokio::sync::mpsc;
    
    let (network_tx, mut network_rx) = mpsc::unbounded_channel();
    
    #[cfg(target_os = "macos")]
    let mut injector = injector::macos::MacOSInjector::new()?;
    
    #[cfg(target_os = "linux")]
    let mut injector = injector::linux::LinuxInjector::new()?;
    
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

struct VirtualMouseProcessor {
    config: config::Config,
    virtual_mouse: virtual_mouse::SharedVirtualMouse,
    transformer: coordinate::CoordinateTransformer,
}

impl VirtualMouseProcessor {
    fn new(config: config::Config, virtual_mouse: virtual_mouse::SharedVirtualMouse) -> Self {
        let transformer = coordinate::CoordinateTransformer::new(config.clone());
        Self { config, virtual_mouse, transformer }
    }
    
    async fn process_events(
        &self,
        mut capture_rx: tokio::sync::mpsc::UnboundedReceiver<capturer::MouseEvent>,
        network_tx: tokio::sync::mpsc::UnboundedSender<capturer::MouseEvent>,
    ) {
        log::info!("VirtualMouseProcessor started, waiting for events...");
        
        while let Some(physical_event) = capture_rx.recv().await {
            let mut vm = self.virtual_mouse.lock().unwrap();
            
            // 1. 物理マウス位置から仮想座標を更新
            let physical_coord = coordinate::LocalCoordinate::from(physical_event.clone());
            vm.update_from_physical(physical_coord, &self.transformer);
            
            // 2. 現在の制御領域を判定
            let should_control_side = vm.determine_control_side(&self.transformer);
            let control_changed = vm.control_side != should_control_side;
            
            if control_changed {
                vm.switch_control(should_control_side);
                
                // 制御権移譲時：相手側に初期位置を送信
                if let Some(transfer_event) = vm.create_transfer_event(&self.transformer) {
                    log::info!("Control transfer: sending initial position ({}, {}) to remote", 
                              transfer_event.x, transfer_event.y);
                    let _ = network_tx.send(transfer_event);
                }
            }
            
            // 3. Remote制御時のみネットワーク送信
            if let virtual_mouse::ControlSide::Remote = vm.control_side {
                if let Some(remote_coord) = vm.get_remote_coordinate(&self.transformer) {
                    let network_event = capturer::MouseEvent {
                        x: remote_coord.x,
                        y: remote_coord.y,
                        event_type: physical_event.event_type.clone(),
                    };
                    log::debug!("Sending to remote: ({}, {})", network_event.x, network_event.y);
                    let _ = network_tx.send(network_event);
                }
            }
            
            log::debug!("Virtual position: ({}, {}), Control: {:?}", 
                       vm.virtual_position.x, vm.virtual_position.y, vm.control_side);
        }
        
        log::warn!("VirtualMouseProcessor stopped receiving events");
    }
}

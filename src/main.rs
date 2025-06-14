use capturer::MouseCapturer;
use clap::{Parser, Subcommand};
use injector::MouseInjector;
use log::{error, info};
use std::path::PathBuf;

mod capturer;
mod config;
mod coordinate;
mod injector;
mod network;
mod virtual_mouse;

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
    use tokio::sync::mpsc;

    let (network_tx, network_rx) = mpsc::unbounded_channel();

    let capturer = capturer::macos::MacOSCapturer::new(
        config.screen.width,
        config.screen.height,
        match config.edge.sender_to_receiver {
            config::EdgeDirection::Left => "left",
            config::EdgeDirection::Right => "right",
            config::EdgeDirection::Top => "top",
            config::EdgeDirection::Bottom => "bottom",
        },
    );

    let network_sender = network::NetworkSender::new(config.clone());

    // 物理マウスキャプチャ → 直接ネットワーク送信
    tokio::spawn(async move {
        if let Err(e) = capturer.start_capture(network_tx).await {
            error!("Capture error: {}", e);
        }
    });

    // ネットワーク送信
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

struct VirtualMouseProcessor {
    config: config::Config,
    virtual_mouse: virtual_mouse::SharedVirtualMouse,
    transformer: coordinate::CoordinateTransformer,
}

impl VirtualMouseProcessor {
    fn new(config: config::Config, virtual_mouse: virtual_mouse::SharedVirtualMouse) -> Self {
        let transformer = coordinate::CoordinateTransformer::new(config.clone());
        Self {
            config,
            virtual_mouse,
            transformer,
        }
    }

    async fn process_events(
        &self,
        mut capture_rx: tokio::sync::mpsc::UnboundedReceiver<capturer::MouseEvent>,
        network_tx: tokio::sync::mpsc::UnboundedSender<capturer::MouseEvent>,
    ) {
        log::info!("VirtualMouseProcessor started, waiting for events...");

        while let Some(physical_event) = capture_rx.recv().await {
            log::debug!(
                "Received physical event: ({:.1}, {:.1})",
                physical_event.x,
                physical_event.y
            );

            let mut vm = self.virtual_mouse.lock().unwrap();

            let physical_coord = coordinate::LocalCoordinate::from(physical_event.clone());
            let delta =
                if let (Some(dx), Some(dy)) = (physical_event.delta_x, physical_event.delta_y) {
                    Some((dx, dy))
                } else {
                    None
                };

            // 1. 現在の制御状態に応じて座標更新
            vm.update_from_physical(physical_coord.clone(), delta, &self.transformer);

            // 2. 制御領域を再判定（座標更新後）
            let should_control_side = vm.determine_control_side(&self.transformer, &physical_coord);
            let control_changed = vm.control_side != should_control_side;

            log::debug!(
                "Control check: current={:?}, should={:?}, changed={}",
                vm.control_side,
                should_control_side,
                control_changed
            );

            if control_changed {
                log::info!(
                    "Control changing from {:?} to {:?} at virtual ({}, {})",
                    vm.control_side,
                    should_control_side,
                    vm.virtual_position.x,
                    vm.virtual_position.y
                );
                vm.switch_control(should_control_side, &physical_coord);

                // 制御権移譲時：相手側に初期位置を送信
                if let Some(transfer_event) = vm.create_transfer_event(&self.transformer) {
                    log::info!(
                        "Control transfer: sending initial position ({}, {}) to remote",
                        transfer_event.x,
                        transfer_event.y
                    );
                    let _ = network_tx.send(transfer_event);
                }
            }

            // 3. Remote制御時のみネットワーク送信
            if let virtual_mouse::ControlSide::Remote = vm.control_side {
                if let Some(remote_coord) = vm.get_remote_coordinate(&self.transformer) {
                    let network_event = capturer::MouseEvent {
                        x: remote_coord.x,
                        y: remote_coord.y,
                        delta_x: physical_event.delta_x,
                        delta_y: physical_event.delta_y,
                        event_type: physical_event.event_type.clone(),
                    };
                    log::info!(
                        "Sending to remote: ({}, {}) [virtual: ({}, {})]",
                        network_event.x,
                        network_event.y,
                        vm.virtual_position.x,
                        vm.virtual_position.y
                    );
                    let _ = network_tx.send(network_event);
                }
            }

            log::debug!(
                "Virtual position: ({}, {}), Control: {:?}",
                vm.virtual_position.x,
                vm.virtual_position.y,
                vm.control_side
            );
        }

        log::warn!("VirtualMouseProcessor stopped receiving events");
    }
}

use crate::config::Config;
use crate::event::MouseEvent;

use crate::virtual_model::SharedVirtualModel;
use anyhow::Result;
use std::sync::Mutex as StdMutex;
use std::sync::Once;
use tokio::sync::mpsc;

// グローバルな状態を管理するための構造体
struct GlobalState {
    virtual_model: Option<SharedVirtualModel>,
    sender: Option<mpsc::UnboundedSender<MouseEvent>>,
    is_running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    config: Option<Config>,
}

static GLOBAL_STATE: StdMutex<Option<GlobalState>> = StdMutex::new(None);
static INIT: Once = Once::new();

pub trait MouseCapturer {
    async fn start_capture_with_model(
        &self,
        config: &Config,
        sender: mpsc::UnboundedSender<MouseEvent>,
        virtual_model: SharedVirtualModel,
    ) -> Result<()>;
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use core_graphics::display::CGDisplayShowCursor;
    use core_graphics::event::{CGEvent, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    pub struct MacOSCapturer {
        is_running: Arc<AtomicBool>,
    }

    impl MacOSCapturer {
        pub fn new() -> Self {
            Self {
                is_running: Arc::new(AtomicBool::new(false)),
            }
        }

        /// マウスを画面中央に固定する関数
        pub fn warp_to_center(&self, config: &Config) -> Result<()> {
            let (center_x, center_y) = config.host_center();
            let center_point = CGPoint::new(center_x, center_y);

            match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                Ok(event_source) => {
                    match CGEvent::new_mouse_event(
                        event_source,
                        CGEventType::MouseMoved,
                        center_point,
                        CGMouseButton::Left,
                    ) {
                        Ok(event) => {
                            event.post(core_graphics::event::CGEventTapLocation::HID);
                            log::debug!(
                                "Mouse warped to center: ({:.1}, {:.1})",
                                center_x,
                                center_y
                            );
                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to create mouse event: {:?}", e);
                            Err(anyhow::anyhow!("Failed to create mouse event"))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create event source: {:?}", e);
                    Err(anyhow::anyhow!("Failed to create event source"))
                }
            }
        }
    }

    impl MouseCapturer for MacOSCapturer {
        async fn start_capture_with_model(
            &self,
            config: &Config,
            sender: mpsc::UnboundedSender<MouseEvent>,
            virtual_model: SharedVirtualModel,
        ) -> Result<()> {
            self.is_running.store(true, Ordering::SeqCst);
            log::info!("Starting macOS mouse capture with rdev");

            // アクセシビリティ権限をチェック
            log::info!("Checking accessibility permissions...");
            match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                Ok(event_source) => {
                    match CGEvent::new_mouse_event(
                        event_source,
                        CGEventType::MouseMoved,
                        CGPoint::new(0.0, 0.0),
                        CGMouseButton::Left,
                    ) {
                        Ok(_event) => {
                            log::info!("Accessibility permissions OK");
                        }
                        Err(_) => {
                            log::error!(
                                "Failed to create CGEvent - please grant accessibility permissions"
                            );
                            return Err(anyhow::anyhow!("Accessibility permissions required"));
                        }
                    }
                }
                Err(_) => {
                    log::error!(
                        "Failed to create CGEventSource - please grant accessibility permissions"
                    );
                    return Err(anyhow::anyhow!("Accessibility permissions required"));
                }
            }

            // 初期マウス位置を設定
            let current_position = unsafe {
                use cocoa::appkit::NSEvent;
                use cocoa::base::nil;
                let mouse_location = NSEvent::mouseLocation(nil);
                CGPoint::new(mouse_location.x, mouse_location.y)
            };

            {
                let mut locked = virtual_model.lock().unwrap();
                locked.init(config, current_position.x, current_position.y);
                log::info!(
                    "VirtualModel initialized at ({}, {})",
                    current_position.x,
                    current_position.y
                );
            }

            // グローバル状態を設定
            {
                let mut global_state = GLOBAL_STATE.lock().unwrap();
                *global_state = Some(GlobalState {
                    virtual_model: Some(virtual_model.clone()),
                    sender: Some(sender.clone()),
                    is_running: self.is_running.clone(),
                    config: Some(config.clone()),
                });
            }

            // rdevでマウスイベントをリッスン（別スレッドで実行）
            std::thread::spawn(move || {
                use rdev::{listen, Event, EventType};

                fn event_callback(event: Event) {
                    let global_state = GLOBAL_STATE.lock().unwrap();
                    if let Some(state) = global_state.as_ref() {
                        if !state.is_running.load(std::sync::atomic::Ordering::SeqCst) {
                            return;
                        }

                        if let (Some(vm), Some(sender), Some(config)) = (
                            state.virtual_model.as_ref(),
                            state.sender.as_ref(),
                            state.config.as_ref(),
                        ) {
                            match event.event_type {
                                EventType::MouseMove { x, y } => {
                                    log::debug!("Mouse moved to: ({}, {})", x, y);

                                    // VirtualModelを更新
                                    if let Ok(mut vm) = vm.lock() {
                                        vm.update(config, x, y);
                                        log::debug!(
                                            "VirtualModel updated: ({}, {})",
                                            vm.virtual_x,
                                            vm.virtual_y
                                        );
                                        if !vm.in_host(config) {
                                            let (x, y) = vm.receiver_position(config);
                                            let mouse_event = MouseEvent::Move { x, y };
                                            if let Err(e) = sender.send(mouse_event) {
                                                log::error!("Failed to send mouse event: {}", e);
                                            }
                                        }
                                    }
                                }
                                EventType::ButtonPress(button) => {
                                    let mouse_event = match button {
                                        rdev::Button::Left => MouseEvent::LeftClick,
                                        rdev::Button::Right => MouseEvent::RightClick,
                                        rdev::Button::Middle => MouseEvent::MiddleClick,
                                        _ => return,
                                    };

                                    if let Err(e) = sender.send(mouse_event) {
                                        log::error!("Failed to send mouse click event: {}", e);
                                    }
                                }
                                EventType::ButtonRelease(button) => {
                                    let mouse_event = match button {
                                        rdev::Button::Left => MouseEvent::LeftRelease,
                                        rdev::Button::Right => MouseEvent::RightRelease,
                                        rdev::Button::Middle => MouseEvent::MiddleRelease,
                                        _ => return,
                                    };

                                    if let Err(e) = sender.send(mouse_event) {
                                        log::error!("Failed to send mouse release event: {}", e);
                                    }
                                }
                                EventType::Wheel { delta_x, delta_y } => {
                                    let mouse_event = MouseEvent::Scroll { delta_x, delta_y };

                                    if let Err(e) = sender.send(mouse_event) {
                                        log::error!("Failed to send mouse scroll event: {}", e);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if let Err(e) = listen(event_callback) {
                    log::error!("rdev listen error: {:?}", e);
                }
            });

            // メインループを維持
            while self.is_running.load(Ordering::SeqCst) {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            log::info!("Mouse capture stopped");
            Ok(())
        }
    }
}

use crate::event::MouseEvent;
use crate::event::MouseEventType;
use crate::virtual_model::SharedVirtualModel;
use anyhow::Result;
use tokio::sync::mpsc;

pub trait MouseCapturer {
    async fn start_capture_with_model(
        &self,
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
        pub fn warp_to_center(&self) -> Result<()> {
            use core_graphics::display::CGMainDisplayID;

            let (_, display_bounds) = unsafe {
                let display_id = CGMainDisplayID();
                let display_bounds = core_graphics::display::CGDisplayBounds(display_id);
                (display_id, display_bounds)
            };

            let center_x = display_bounds.origin.x + display_bounds.size.width / 2.0;
            let center_y = display_bounds.origin.y + display_bounds.size.height / 2.0;

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
            sender: mpsc::UnboundedSender<MouseEvent>,
            virtual_model: SharedVirtualModel,
        ) -> Result<()> {
            self.is_running.store(true, Ordering::SeqCst);
            log::info!("Starting macOS mouse capture with VirtualModel");

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

            // 現在のマウス位置を取得する関数
            let get_mouse_location = || -> CGPoint {
                use cocoa::appkit::NSEvent;
                use cocoa::base::nil;

                unsafe {
                    let mouse_location = NSEvent::mouseLocation(nil);
                    CGPoint::new(mouse_location.x, mouse_location.y)
                }
            };

            let mut last_position = get_mouse_location();
            {
                log::info!(
                    "Mouse capture at position ({}, {})",
                    last_position.x,
                    last_position.y
                );
                let mut locked = virtual_model.lock().unwrap();
                locked.init(last_position.x, last_position.y);
            }

            while self.is_running.load(Ordering::SeqCst) {
                let current_position = get_mouse_location();

                // 相対移動量を計算
                let delta_x = current_position.x - last_position.x;
                let delta_y = current_position.y - last_position.y;

                // 移動があった場合のみ処理
                if delta_x.abs() > 0.1 || delta_y.abs() > 0.1 {
                    log::debug!(
                        "Mouse moved: position=({:.1}, {:.1}), delta=({:.1}, {:.1})",
                        current_position.x,
                        current_position.y,
                        delta_x,
                        delta_y
                    );

                    // VirtualModelを直接更新
                    if let Ok(mut vm) = virtual_model.lock() {
                        vm.virtual_x = current_position.x;
                        vm.virtual_y = current_position.y;
                        log::debug!("VirtualModel updated: ({}, {})", vm.virtual_x, vm.virtual_y);
                    }

                    // 絶対座標でMouseEventを送信
                    let mouse_event = MouseEvent {
                        x: current_position.x,
                        y: current_position.y,
                        event_type: MouseEventType::Move,
                    };

                    if let Err(e) = sender.send(mouse_event) {
                        log::error!("Failed to send mouse event: {}", e);
                        break;
                    }

                    last_position = current_position;
                }
                self.warp_to_center().unwrap();

                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }

            log::info!("Mouse capture stopped");
            Ok(())
        }
    }
}

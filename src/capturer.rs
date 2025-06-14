use crate::event::MouseEvent;
use crate::event::MouseEventType;
use anyhow::Result;
use tokio::sync::mpsc;

pub trait MouseCapturer {
    async fn start_capture(&self, sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()>;
    fn stop_capture(&self) -> Result<()>;
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
        screen_width: f64,
        screen_height: f64,
        transfer_edge: String,
    }

    impl MacOSCapturer {
        pub fn new(screen_width: u32, screen_height: u32, transfer_edge: &str) -> Self {
            Self {
                is_running: Arc::new(AtomicBool::new(false)),
                screen_width: screen_width as f64,
                screen_height: screen_height as f64,
                transfer_edge: transfer_edge.to_string(),
            }
        }
    }

    impl MouseCapturer for MacOSCapturer {
        async fn start_capture(&self, sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()> {
            self.is_running.store(true, Ordering::SeqCst);
            log::info!("Starting macOS CGEvent-based mouse capture");

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
                            log::error!("Failed to create CGEvent - please grant accessibility permissions in System Preferences > Security & Privacy > Privacy > Accessibility");
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
            log::info!(
                "Mouse capture started at position ({}, {})",
                last_position.x,
                last_position.y
            );

            while self.is_running.load(Ordering::SeqCst) {
                let current_position = get_mouse_location();

                // 相対移動量を計算（取得するが使用しない）
                let delta_x = current_position.x - last_position.x;
                let delta_y = current_position.y - last_position.y;

                // 移動があった場合のみ処理
                if delta_x.abs() > 0.1 || delta_y.abs() > 0.1 {
                    // 絶対座標を送信
                    let mouse_event = MouseEvent {
                        x: current_position.x,
                        y: current_position.y,
                        event_type: MouseEventType::Move,
                    };

                    log::debug!(
                        "Sending absolute position: ({:.1}, {:.1}), delta: ({:.1}, {:.1})",
                        current_position.x,
                        current_position.y,
                        delta_x,
                        delta_y
                    );

                    if sender.send(mouse_event).is_err() {
                        log::error!("Failed to send mouse event");
                        break;
                    }

                    last_position = current_position;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }

            log::info!("CGEvent mouse capture stopped");
            Ok(())
        }

        fn stop_capture(&self) -> Result<()> {
            self.is_running.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
}

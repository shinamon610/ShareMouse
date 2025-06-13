use anyhow::Result;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub x: f64,
    pub y: f64,
    pub event_type: MouseEventType,
}

#[derive(Debug, Clone)]
pub enum MouseEventType {
    Move,
    LeftClick,
    RightClick,
    MiddleClick,
    LeftRelease,
    RightRelease,
    MiddleRelease,
    ScrollUp,
    ScrollDown,
}

pub trait MouseCapturer {
    async fn start_capture(&self, sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()>;
    fn stop_capture(&self) -> Result<()>;
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    pub struct MacOSCapturer {
        is_running: Arc<AtomicBool>,
    }
    
    impl MacOSCapturer {
        pub fn new() -> Self {
            Self {
                is_running: Arc::new(AtomicBool::new(false)),
            }
        }
    }
    
    impl MouseCapturer for MacOSCapturer {
        async fn start_capture(&self, _sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()> {
            // Simplified implementation for now - would need proper event tap setup
            self.is_running.store(true, Ordering::SeqCst);
            
            // TODO: Implement proper CGEventTap setup
            loop {
                if !self.is_running.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            
            Ok(())
        }
        
        fn stop_capture(&self) -> Result<()> {
            self.is_running.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use evdev::{Device, InputEventKind, RelativeAxisType, Key};
    
    pub struct LinuxCapturer {
        device_path: String,
        current_x: f64,
        current_y: f64,
    }
    
    impl LinuxCapturer {
        pub fn new(device_path: &str) -> Self {
            Self {
                device_path: device_path.to_string(),
                current_x: 0.0,
                current_y: 0.0,
            }
        }
    }
    
    impl MouseCapturer for LinuxCapturer {
        async fn start_capture(&self, sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()> {
            let mut device = Device::open(&self.device_path)?;
            let mut current_x = self.current_x;
            let mut current_y = self.current_y;
            
            loop {
                let events = device.fetch_events()?;
                for event in events {
                    match event.kind() {
                        InputEventKind::RelAxis(axis) => {
                            match axis {
                                RelativeAxisType::REL_X => {
                                    current_x += event.value() as f64;
                                    current_x = current_x.max(0.0);
                                    let mouse_event = MouseEvent {
                                        x: current_x,
                                        y: current_y,
                                        event_type: MouseEventType::Move,
                                    };
                                    let _ = sender.send(mouse_event);
                                }
                                RelativeAxisType::REL_Y => {
                                    current_y += event.value() as f64;
                                    current_y = current_y.max(0.0);
                                    let mouse_event = MouseEvent {
                                        x: current_x,
                                        y: current_y,
                                        event_type: MouseEventType::Move,
                                    };
                                    let _ = sender.send(mouse_event);
                                }
                                RelativeAxisType::REL_WHEEL => {
                                    let scroll_event = MouseEvent {
                                        x: current_x,
                                        y: current_y,
                                        event_type: if event.value() > 0 {
                                            MouseEventType::ScrollUp
                                        } else {
                                            MouseEventType::ScrollDown
                                        },
                                    };
                                    let _ = sender.send(scroll_event);
                                }
                                _ => {}
                            }
                        }
                        InputEventKind::Key(key) => {
                            let event_type = match key {
                                Key::BTN_LEFT => {
                                    if event.value() == 1 {
                                        Some(MouseEventType::LeftClick)
                                    } else if event.value() == 0 {
                                        Some(MouseEventType::LeftRelease)
                                    } else { None }
                                }
                                Key::BTN_RIGHT => {
                                    if event.value() == 1 {
                                        Some(MouseEventType::RightClick)
                                    } else if event.value() == 0 {
                                        Some(MouseEventType::RightRelease)
                                    } else { None }
                                }
                                Key::BTN_MIDDLE => {
                                    if event.value() == 1 {
                                        Some(MouseEventType::MiddleClick)
                                    } else if event.value() == 0 {
                                        Some(MouseEventType::MiddleRelease)
                                    } else { None }
                                }
                                _ => None,
                            };
                            
                            if let Some(event_type) = event_type {
                                let mouse_event = MouseEvent {
                                    x: current_x,
                                    y: current_y,
                                    event_type,
                                };
                                let _ = sender.send(mouse_event);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        fn stop_capture(&self) -> Result<()> {
            Ok(())
        }
    }
}
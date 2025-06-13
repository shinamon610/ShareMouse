use anyhow::Result;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub x: f64,
    pub y: f64,
    pub delta_x: Option<f64>,  // 相対移動量
    pub delta_y: Option<f64>,  // 相対移動量
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
    use hidapi::HidApi;
    
    pub struct MacOSCapturer {
        is_running: Arc<AtomicBool>,
    }
    
    impl MacOSCapturer {
        pub fn new() -> Self {
            Self {
                is_running: Arc::new(AtomicBool::new(false)),
            }
        }
        
        fn find_mouse_device(api: &HidApi) -> Option<hidapi::HidDevice> {
            // HIDマウスデバイスを検索（Usage Page: 0x01, Usage: 0x02）
            for device_info in api.device_list() {
                if device_info.usage_page() == 0x01 && device_info.usage() == 0x02 {
                    log::info!("Found mouse device: {:04x}:{:04x} - {}", 
                              device_info.vendor_id(), device_info.product_id(),
                              device_info.product_string().unwrap_or("Unknown"));
                    
                    if let Ok(device) = device_info.open_device(api) {
                        return Some(device);
                    }
                }
            }
            None
        }
    }
    
    impl MouseCapturer for MacOSCapturer {
        async fn start_capture(&self, sender: mpsc::UnboundedSender<MouseEvent>) -> Result<()> {
            self.is_running.store(true, Ordering::SeqCst);
            
            log::info!("Starting macOS mouse capture with HID API");
            
            let api = HidApi::new()?;
            let device = Self::find_mouse_device(&api)
                .ok_or_else(|| anyhow::anyhow!("No mouse device found"))?;
            
            device.set_blocking_mode(false)?;
            
            let mut virtual_x = 0.0f64;
            let mut virtual_y = 0.0f64;
            
            while self.is_running.load(Ordering::SeqCst) {
                let mut buf = [0u8; 64];
                match device.read(&mut buf) {
                    Ok(size) if size > 0 => {
                        // 標準的なマウスHIDレポート: [buttons, delta_x, delta_y, wheel]
                        if size >= 3 {
                            let delta_x = buf[1] as i8 as f64;
                            let delta_y = buf[2] as i8 as f64;
                            
                            if delta_x != 0.0 || delta_y != 0.0 {
                                virtual_x += delta_x;
                                virtual_y += delta_y;
                                
                                log::debug!("HID mouse delta: ({}, {}), virtual: ({}, {})", 
                                           delta_x, delta_y, virtual_x, virtual_y);
                                
                                let mouse_event = MouseEvent {
                                    x: virtual_x,
                                    y: virtual_y,
                                    delta_x: Some(delta_x),
                                    delta_y: Some(delta_y),
                                    event_type: MouseEventType::Move,
                                };
                                
                                if sender.send(mouse_event).is_err() {
                                    log::error!("Failed to send mouse event");
                                    break;
                                }
                            }
                        }
                    }
                    Ok(_) => {
                        // データなし、少し待つ
                        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                    }
                    Err(e) => {
                        log::warn!("HID read error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                }
            }
            
            log::info!("Mouse capture stopped");
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
        screen_width: f64,
        screen_height: f64,
    }
    
    impl LinuxCapturer {
        pub fn new(device_path: &str, screen_width: u32, screen_height: u32) -> Self {
            Self {
                device_path: device_path.to_string(),
                current_x: 0.0,
                current_y: 0.0,
                screen_width: screen_width as f64,
                screen_height: screen_height as f64,
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
                                    current_x = current_x.max(0.0).min(self.screen_width - 1.0);
                                    let mouse_event = MouseEvent {
                                        x: current_x,
                                        y: current_y,
                                        delta_x: Some(event.value() as f64),
                                        delta_y: Some(0.0),
                                        event_type: MouseEventType::Move,
                                    };
                                    log::debug!("Mouse X: {}, Y: {}", current_x, current_y);
                                    let _ = sender.send(mouse_event);
                                }
                                RelativeAxisType::REL_Y => {
                                    current_y += event.value() as f64;
                                    current_y = current_y.max(0.0).min(self.screen_height - 1.0);
                                    let mouse_event = MouseEvent {
                                        x: current_x,
                                        y: current_y,
                                        delta_x: Some(0.0),
                                        delta_y: Some(event.value() as f64),
                                        event_type: MouseEventType::Move,
                                    };
                                    log::debug!("Mouse X: {}, Y: {}", current_x, current_y);
                                    let _ = sender.send(mouse_event);
                                }
                                RelativeAxisType::REL_WHEEL => {
                                    let scroll_event = MouseEvent {
                                        x: current_x,
                                        y: current_y,
                                        delta_x: None,
                                        delta_y: None,
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
                                    delta_x: None,
                                    delta_y: None,
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
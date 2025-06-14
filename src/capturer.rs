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
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::event::{CGEvent, CGEventType, CGMouseButton};
    use core_graphics::geometry::CGPoint;
    use core_graphics::display::{CGDisplayShowCursor, CGWarpMouseCursorPosition};
    use std::sync::Mutex;
    
    pub struct MacOSCapturer {
        is_running: Arc<AtomicBool>,
        sender: Arc<Mutex<Option<mpsc::UnboundedSender<MouseEvent>>>>,
        screen_center: CGPoint,
        is_secondary_control: Arc<AtomicBool>, // Linux側を制御中かどうか
        screen_width: f64,
        screen_height: f64,
        transfer_edge: String, // "left", "right", "top", "bottom"
    }
    
    impl MacOSCapturer {
        pub fn new(screen_width: u32, screen_height: u32, transfer_edge: &str) -> Self {
            let width = screen_width as f64;
            let height = screen_height as f64;
            Self {
                is_running: Arc::new(AtomicBool::new(false)),
                sender: Arc::new(Mutex::new(None)),
                screen_center: CGPoint::new(width / 2.0, height / 2.0),
                is_secondary_control: Arc::new(AtomicBool::new(false)),
                screen_width: width,
                screen_height: height,
                transfer_edge: transfer_edge.to_string(),
            }
        }
        
        // 制御モードを切り替える公開メソッド
        pub fn set_secondary_control(&self, enable: bool) {
            self.is_secondary_control.store(enable, Ordering::SeqCst);
            if enable {
                log::info!("Switching to secondary control mode (controlling Linux)");
                unsafe {
                    CGWarpMouseCursorPosition(self.screen_center);
                }
            } else {
                log::info!("Switching to primary control mode (controlling macOS)");
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
                        },
                        Err(_) => {
                            log::error!("Failed to create CGEvent - please grant accessibility permissions in System Preferences > Security & Privacy > Privacy > Accessibility");
                            return Err(anyhow::anyhow!("Accessibility permissions required"));
                        }
                    }
                },
                Err(_) => {
                    log::error!("Failed to create CGEventSource - please grant accessibility permissions");
                    return Err(anyhow::anyhow!("Accessibility permissions required"));
                }
            }
            
            // senderを保存
            {
                let mut sender_guard = self.sender.lock().unwrap();
                *sender_guard = Some(sender);
            }
            
            // カーソを表示
            unsafe {
                CGDisplayShowCursor(0);
            }
            
            
            let mut last_position = CGPoint::new(0.0, 0.0);
            let mut virtual_x = 2560.0f64;
            let mut virtual_y = 720.0f64;
            
            // 現在のマウス位置を取得する関数
            let get_mouse_location = || -> CGPoint {
                // Cocoa NSEventを使用してマウス位置を取得
                use cocoa::appkit::NSEvent;
                use cocoa::base::nil;
                
                unsafe {
                    let mouse_location = NSEvent::mouseLocation(nil);
                    let point = CGPoint::new(mouse_location.x, mouse_location.y);
                    log::debug!("Got mouse position: ({:.1}, {:.1})", point.x, point.y);
                    point
                }
            };
            
            last_position = get_mouse_location();
            log::info!("Mouse capture started at position ({}, {})", last_position.x, last_position.y);
            
            let mut loop_count = 0;
            
            while self.is_running.load(Ordering::SeqCst) {
                loop_count += 1;
                
                if loop_count % 1000 == 0 {
                    log::debug!("CGEvent polling iteration: {}", loop_count);
                }
                
                let current_position = get_mouse_location();
                
                if self.is_secondary_control.load(Ordering::SeqCst) {
                    // Linux側制御中：移動量を計算してLinux側に送信
                    let delta_x = current_position.x - self.screen_center.x;
                    let delta_y = current_position.y - self.screen_center.y;
                    
                    if delta_x.abs() > 2.0 || delta_y.abs() > 2.0 {
                        // Linux側には移動量のみ送信（座標は無関係）
                        let mouse_event = MouseEvent {
                            x: 0.0,  // 座標は無視
                            y: 0.0,  // 座標は無視
                            delta_x: Some(delta_x),
                            delta_y: Some(delta_y),
                            event_type: MouseEventType::Move,
                        };
                        
                        log::info!("Secondary control: sending delta=({:.1}, {:.1}) to Linux", 
                                  delta_x, delta_y);
                        
                        if let Ok(sender_guard) = self.sender.lock() {
                            if let Some(sender_ref) = sender_guard.as_ref() {
                                if sender_ref.send(mouse_event).is_err() {
                                    log::error!("Failed to send mouse event");
                                    break;
                                }
                            }
                        }
                        
                        // マウスを中央に戻す
                        unsafe {
                            CGWarpMouseCursorPosition(self.screen_center);
                        }
                    }
                } else {
                    // macOS側制御中：通常のマウス移動
                    let delta_x = current_position.x - last_position.x;
                    let delta_y = current_position.y - last_position.y;
                    
                    if delta_x.abs() > 0.5 || delta_y.abs() > 0.5 {
                        // 画面端検知（設定に基づく端での移譲）
                        let at_edge = match self.transfer_edge.as_str() {
                            "left" => current_position.x <= 1.0,
                            "right" => current_position.x >= self.screen_width - 1.0,
                            "top" => current_position.y <= 1.0,
                            "bottom" => current_position.y >= self.screen_height - 1.0,
                            _ => false,
                        };
                        
                        if at_edge {
                            log::info!("Reached {} edge at ({:.1}, {:.1}), switching to secondary control", 
                                     self.transfer_edge, current_position.x, current_position.y);
                            self.is_secondary_control.store(true, Ordering::SeqCst);
                            
                            // マウスを中央に移動
                            unsafe {
                                CGWarpMouseCursorPosition(self.screen_center);
                            }
                            continue; // この回はイベント送信をスキップ
                        }
                        
                        // 通常のmacOS側移動（ネットワーク送信なし）
                        log::debug!("Primary control: position=({:.1}, {:.1}) - not sending to network", 
                                   current_position.x, current_position.y);
                        
                        last_position = current_position;
                    }
                }
                
                // 短い間隔でポーリング
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
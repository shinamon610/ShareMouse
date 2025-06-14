use anyhow::Result;
use crate::capturer::MouseEvent;

pub trait MouseInjector {
    fn inject_event(&mut self, event: MouseEvent) -> Result<()>;
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use crate::capturer::{MouseEvent, MouseEventType};
    use core_graphics::event::{CGEvent, CGEventType, CGMouseButton, EventField, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;
    
    pub struct MacOSInjector {
        event_source: CGEventSource,
    }
    
    impl MacOSInjector {
        pub fn new() -> Result<Self> {
            let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .map_err(|_| anyhow::anyhow!("Failed to create event source"))?;
            Ok(Self { event_source })
        }
    }
    
    impl MouseInjector for MacOSInjector {
        fn inject_event(&mut self, event: MouseEvent) -> Result<()> {
            let location = CGPoint::new(event.x, event.y);
            
            let cg_event = match event.event_type {
                MouseEventType::Move => {
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::MouseMoved,
                        location,
                        CGMouseButton::Left,
                    ).map_err(|_| anyhow::anyhow!("Failed to create mouse move event"))?
                }
                MouseEventType::LeftClick => {
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::LeftMouseDown,
                        location,
                        CGMouseButton::Left,
                    ).map_err(|_| anyhow::anyhow!("Failed to create left click event"))?
                }
                MouseEventType::LeftRelease => {
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::LeftMouseUp,
                        location,
                        CGMouseButton::Left,
                    ).map_err(|_| anyhow::anyhow!("Failed to create left release event"))?
                }
                MouseEventType::RightClick => {
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::RightMouseDown,
                        location,
                        CGMouseButton::Right,
                    ).map_err(|_| anyhow::anyhow!("Failed to create right click event"))?
                }
                MouseEventType::RightRelease => {
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::RightMouseUp,
                        location,
                        CGMouseButton::Right,
                    ).map_err(|_| anyhow::anyhow!("Failed to create right release event"))?
                }
                MouseEventType::MiddleClick => {
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::OtherMouseDown,
                        location,
                        CGMouseButton::Center,
                    ).map_err(|_| anyhow::anyhow!("Failed to create middle click event"))?
                }
                MouseEventType::MiddleRelease => {
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::OtherMouseUp,
                        location,
                        CGMouseButton::Center,
                    ).map_err(|_| anyhow::anyhow!("Failed to create middle release event"))?
                }
                MouseEventType::ScrollUp => {
                    let event = CGEvent::new(self.event_source.clone())
                        .map_err(|_| anyhow::anyhow!("Failed to create scroll event"))?;
                    event.set_type(CGEventType::ScrollWheel);
                    event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1, 1);
                    event
                }
                MouseEventType::ScrollDown => {
                    let event = CGEvent::new(self.event_source.clone())
                        .map_err(|_| anyhow::anyhow!("Failed to create scroll event"))?;
                    event.set_type(CGEventType::ScrollWheel);
                    event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1, -1);
                    event
                }
            };
            
            cg_event.post(CGEventTapLocation::HID);
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use crate::capturer::{MouseEvent, MouseEventType};
    use std::process::Command;
    
    pub struct LinuxInjector {
        // For Wayland, we'll use external tools or direct protocol calls
    }
    
    impl LinuxInjector {
        pub fn new() -> Result<Self> {
            // For Wayland, we don't need uinput device creation
            Ok(Self {})
        }
    }
    
    impl MouseInjector for LinuxInjector {
        fn inject_event(&mut self, event: MouseEvent) -> Result<()> {
            log::info!("Injecting event: {:?} at ({}, {}) with delta ({:?}, {:?})", 
                      event.event_type, event.x, event.y, event.delta_x, event.delta_y);
            
            match event.event_type {
                MouseEventType::Move => {
                    // 絶対移動のみ対応
                    if event.x >= 0.0 && event.y >= 0.0 {
                        self.move_cursor_wayland(event.x as i32, event.y as i32)?;
                    } else {
                        log::debug!("Ignoring invalid coordinates ({}, {})", event.x, event.y);
                    }
                }
                MouseEventType::LeftClick => {
                    self.click_wayland(1, true)?;
                }
                MouseEventType::LeftRelease => {
                    self.click_wayland(1, false)?;
                }
                MouseEventType::RightClick => {
                    self.click_wayland(3, true)?;
                }
                MouseEventType::RightRelease => {
                    self.click_wayland(3, false)?;
                }
                MouseEventType::MiddleClick => {
                    self.click_wayland(2, true)?;
                }
                MouseEventType::MiddleRelease => {
                    self.click_wayland(2, false)?;
                }
                MouseEventType::ScrollUp => {
                    self.scroll_wayland(1)?;
                }
                MouseEventType::ScrollDown => {
                    self.scroll_wayland(-1)?;
                }
            }
            
            Ok(())
        }
        
    }
    
    impl LinuxInjector {
        fn move_cursor_wayland(&self, x: i32, y: i32) -> Result<()> {
            log::debug!("Moving cursor to ({}, {}) with ydotool", x, y);
            
            Command::new("ydotool")
                .args(["mousemove", "-a", &x.to_string(), &y.to_string()])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to execute ydotool: {}", e))?;
            
            Ok(())
        }
        
        fn click_wayland(&self, button: i32, press: bool) -> Result<()> {
            let action = if press { "press" } else { "release" };
            log::debug!("Mouse {} button {} with ydotool", action, button);
            
            if press {
                Command::new("ydotool")
                    .args(["click", &button.to_string()])
                    .output()
                    .map_err(|e| anyhow::anyhow!("Failed to execute ydotool: {}", e))?;
            }
            // releaseは通常clickで自動的に処理される
            
            Ok(())
        }
        
        fn scroll_wayland(&self, direction: i32) -> Result<()> {
            log::debug!("Scroll direction {} with ydotool", direction);
            
            let scroll_dir = if direction > 0 { "4" } else { "5" };
            Command::new("ydotool")
                .args(["click", scroll_dir])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to execute ydotool: {}", e))?;
            
            Ok(())
        }
    }
}
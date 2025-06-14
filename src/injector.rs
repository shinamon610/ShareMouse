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
                    // 移動量がある場合は相対移動、そうでなければ絶対移動
                    if let (Some(dx), Some(dy)) = (event.delta_x, event.delta_y) {
                        self.move_cursor_relative_wayland(dx as i32, dy as i32)?;
                    } else {
                        self.move_cursor_wayland(event.x as i32, event.y as i32)?;
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
            log::debug!("Moving Wayland cursor to ({}, {})", x, y);
            
            // Try different approaches for Wayland cursor movement
            
            // Approach 1: Try wlrctl if available
            if let Ok(_) = Command::new("wlrctl")
                .args(["pointer", "move", &x.to_string(), &y.to_string()])
                .output() {
                return Ok(());
            }
            
            // Approach 2: Try wl-pointer-warp if available  
            if let Ok(_) = Command::new("wl-pointer-warp")
                .args([&x.to_string(), &y.to_string()])
                .output() {
                return Ok(());
            }
            
            // Approach 3: Try hyprctl for Hyprland
            if let Ok(_) = Command::new("hyprctl")
                .args(["dispatch", "movecursor", &format!("{} {}", x, y)])
                .output() {
                return Ok(());
            }
            
            log::warn!("No suitable Wayland cursor tool found");
            Ok(())
        }
        
        fn move_cursor_relative_wayland(&self, dx: i32, dy: i32) -> Result<()> {
            log::info!("Attempting Wayland relative cursor movement by ({}, {})", dx, dy);
            
            // Approach 1: Try wlrctl relative movement
            log::info!("Trying wlrctl pointer move-relative {} {}", dx, dy);
            match Command::new("wlrctl")
                .args(["pointer", "move-relative", &dx.to_string(), &dy.to_string()])
                .output() {
                Ok(output) => {
                    log::info!("wlrctl exit status: {}, stdout: {}, stderr: {}", 
                              output.status, 
                              String::from_utf8_lossy(&output.stdout),
                              String::from_utf8_lossy(&output.stderr));
                    if output.status.success() {
                        return Ok(());
                    }
                }
                Err(e) => log::info!("wlrctl command failed: {}", e),
            }
            
            // Approach 2: Try hyprctl relative movement for Hyprland
            log::info!("Trying hyprctl dispatch movecursor relative {} {}", dx, dy);
            match Command::new("hyprctl")
                .args(["dispatch", "movecursor", "relative", &dx.to_string(), &dy.to_string()])
                .output() {
                Ok(output) => {
                    log::info!("hyprctl exit status: {}, stdout: {}, stderr: {}", 
                              output.status, 
                              String::from_utf8_lossy(&output.stdout),
                              String::from_utf8_lossy(&output.stderr));
                    if output.status.success() {
                        return Ok(());
                    }
                }
                Err(e) => log::info!("hyprctl command failed: {}", e),
            }
            
            // Fallback: Try ydotool for relative movement
            log::info!("Trying ydotool mousemove -- {} {}", dx, dy);
            match Command::new("ydotool")
                .args(["mousemove", "--", &dx.to_string(), &dy.to_string()])
                .output() {
                Ok(output) => {
                    log::info!("ydotool exit status: {}, stdout: {}, stderr: {}", 
                              output.status, 
                              String::from_utf8_lossy(&output.stdout),
                              String::from_utf8_lossy(&output.stderr));
                    if output.status.success() {
                        return Ok(());
                    }
                }
                Err(e) => log::info!("ydotool command failed: {}", e),
            }
            
            log::warn!("No suitable Wayland relative cursor movement tool found");
            Ok(())
        }
        
        fn click_wayland(&self, button: i32, press: bool) -> Result<()> {
            let action = if press { "press" } else { "release" };
            log::debug!("Wayland mouse {} button {}", action, button);
            
            // Try wlrctl
            if let Ok(_) = Command::new("wlrctl")
                .args(["pointer", "click", &button.to_string()])
                .output() {
                return Ok(());
            }
            
            log::warn!("No suitable Wayland click tool found");
            Ok(())
        }
        
        fn scroll_wayland(&self, direction: i32) -> Result<()> {
            log::debug!("Wayland scroll direction {}", direction);
            
            // Try wlrctl
            if let Ok(_) = Command::new("wlrctl")
                .args(["pointer", "scroll", &direction.to_string()])
                .output() {
                return Ok(());
            }
            
            log::warn!("No suitable Wayland scroll tool found");
            Ok(())
        }
    }
}
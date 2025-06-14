use crate::event::MouseEvent;
use anyhow::Result;

pub trait MouseInjector {
    fn inject_event(&mut self, event: MouseEvent) -> Result<()>;
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use crate::event::MouseEvent;
    use core_graphics::event::{
        CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField,
    };
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
            let cg_event = match event {
                MouseEvent::Move { x, y } => {
                    let location = CGPoint::new(x, y);
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::MouseMoved,
                        location,
                        CGMouseButton::Left,
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to create mouse move event"))?
                }
                MouseEvent::LeftClick => {
                    // クリック時は現在のマウス位置を使用
                    let current_pos = unsafe {
                        use cocoa::appkit::NSEvent;
                        use cocoa::base::nil;
                        let location = NSEvent::mouseLocation(nil);
                        CGPoint::new(location.x, location.y)
                    };
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::LeftMouseDown,
                        current_pos,
                        CGMouseButton::Left,
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to create left click event"))?
                }
                MouseEvent::LeftRelease => {
                    let current_pos = unsafe {
                        use cocoa::appkit::NSEvent;
                        use cocoa::base::nil;
                        let location = NSEvent::mouseLocation(nil);
                        CGPoint::new(location.x, location.y)
                    };
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::LeftMouseUp,
                        current_pos,
                        CGMouseButton::Left,
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to create left release event"))?
                }
                MouseEvent::RightClick => {
                    let current_pos = unsafe {
                        use cocoa::appkit::NSEvent;
                        use cocoa::base::nil;
                        let location = NSEvent::mouseLocation(nil);
                        CGPoint::new(location.x, location.y)
                    };
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::RightMouseDown,
                        current_pos,
                        CGMouseButton::Right,
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to create right click event"))?
                }
                MouseEvent::RightRelease => {
                    let current_pos = unsafe {
                        use cocoa::appkit::NSEvent;
                        use cocoa::base::nil;
                        let location = NSEvent::mouseLocation(nil);
                        CGPoint::new(location.x, location.y)
                    };
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::RightMouseUp,
                        current_pos,
                        CGMouseButton::Right,
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to create right release event"))?
                }
                MouseEvent::MiddleClick => {
                    let current_pos = unsafe {
                        use cocoa::appkit::NSEvent;
                        use cocoa::base::nil;
                        let location = NSEvent::mouseLocation(nil);
                        CGPoint::new(location.x, location.y)
                    };
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::OtherMouseDown,
                        current_pos,
                        CGMouseButton::Center,
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to create middle click event"))?
                }
                MouseEvent::MiddleRelease => {
                    let current_pos = unsafe {
                        use cocoa::appkit::NSEvent;
                        use cocoa::base::nil;
                        let location = NSEvent::mouseLocation(nil);
                        CGPoint::new(location.x, location.y)
                    };
                    CGEvent::new_mouse_event(
                        self.event_source.clone(),
                        CGEventType::OtherMouseUp,
                        current_pos,
                        CGMouseButton::Center,
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to create middle release event"))?
                }
                MouseEvent::Scroll { delta_x: _, delta_y } => {
                    let event = CGEvent::new(self.event_source.clone())
                        .map_err(|_| anyhow::anyhow!("Failed to create scroll event"))?;
                    event.set_type(CGEventType::ScrollWheel);
                    event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1, delta_y);
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
    use crate::event::MouseEvent;
    use std::process::Command;

    pub struct LinuxInjector;

    impl LinuxInjector {
        pub fn new() -> Result<Self> {
            // ydotoolデーモンの可用性をチェック
            let output = Command::new("ydotool")
                .args(["--help"])
                .output()
                .map_err(|e| anyhow::anyhow!("ydotool not found or not executable: {}", e))?;

            if !output.status.success() {
                return Err(anyhow::anyhow!("ydotool command failed"));
            }

            Ok(Self)
        }
    }

    impl MouseInjector for LinuxInjector {
        fn inject_event(&mut self, event: MouseEvent) -> Result<()> {
            log::info!("Injecting event: {:?}", event);

            match event {
                MouseEvent::Move { x, y } => {
                    // 絶対移動のみ対応
                    if x >= 0.0 && y >= 0.0 {
                        self.move_cursor_wayland(x as i32, y as i32)?;
                    } else {
                        log::debug!("Ignoring invalid coordinates ({}, {})", x, y);
                    }
                }
                MouseEvent::LeftClick => {
                    self.click_wayland(1, true)?;
                }
                MouseEvent::LeftRelease => {
                    self.click_wayland(1, false)?;
                }
                MouseEvent::RightClick => {
                    self.click_wayland(3, true)?;
                }
                MouseEvent::RightRelease => {
                    self.click_wayland(3, false)?;
                }
                MouseEvent::MiddleClick => {
                    self.click_wayland(2, true)?;
                }
                MouseEvent::MiddleRelease => {
                    self.click_wayland(2, false)?;
                }
                MouseEvent::Scroll { delta_x: _, delta_y } => {
                    // delta_yが正の場合は上スクロール、負の場合は下スクロール
                    let direction = if delta_y > 0 { 1 } else { -1 };
                    self.scroll_wayland(direction)?;
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

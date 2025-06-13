use anyhow::Result;
use crate::capturer::MouseEvent;

pub trait MouseInjector {
    fn inject_event(&self, event: MouseEvent) -> Result<()>;
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
        fn inject_event(&self, event: MouseEvent) -> Result<()> {
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
    use uinput::Device;
    
    pub struct LinuxInjector {
        device: Device,
    }
    
    impl LinuxInjector {
        pub fn new() -> Result<Self> {
            let device = uinput::default()?
                .name("sharemouse-virtual")?
                .event(uinput::event::Event::Controller(uinput::event::controller::Controller::Mouse(uinput::event::controller::Mouse::Left)))?
                .event(uinput::event::Event::Controller(uinput::event::controller::Controller::Mouse(uinput::event::controller::Mouse::Right)))?
                .event(uinput::event::Event::Controller(uinput::event::controller::Controller::Mouse(uinput::event::controller::Mouse::Middle)))?
                .event(uinput::event::Event::Relative(uinput::event::relative::Relative::Position(uinput::event::relative::Position::X)))?
                .event(uinput::event::Event::Relative(uinput::event::relative::Relative::Position(uinput::event::relative::Position::Y)))?
                .event(uinput::event::Event::Relative(uinput::event::relative::Relative::Wheel(uinput::event::relative::Wheel::Vertical)))?
                .create()?;
            
            Ok(Self { device })
        }
    }
    
    impl MouseInjector for LinuxInjector {
        fn inject_event(&self, event: MouseEvent) -> Result<()> {
            match event.event_type {
                MouseEventType::Move => {
                    // TODO: Implement relative movement for Linux
                }
                MouseEventType::LeftClick => {
                    // TODO: Implement left click for Linux
                }
                MouseEventType::LeftRelease => {
                    // TODO: Implement left release for Linux
                }
                MouseEventType::RightClick => {
                    // TODO: Implement right click for Linux
                }
                MouseEventType::RightRelease => {
                    // TODO: Implement right release for Linux
                }
                MouseEventType::MiddleClick => {
                    // TODO: Implement middle click for Linux
                }
                MouseEventType::MiddleRelease => {
                    // TODO: Implement middle release for Linux
                }
                MouseEventType::ScrollUp => {
                    // TODO: Implement scroll up for Linux
                }
                MouseEventType::ScrollDown => {
                    // TODO: Implement scroll down for Linux
                }
            }
            
            Ok(())
        }
    }
}
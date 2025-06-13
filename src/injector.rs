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
        fn inject_event(&mut self, event: MouseEvent) -> Result<()> {
            use uinput::event::controller::{Controller, Mouse};
            
            log::debug!("Injecting event: {:?} at ({}, {})", event.event_type, event.x, event.y);
            
            match event.event_type {
                MouseEventType::Move => {
                    // For absolute positioning, we need to use ABS events
                    // This is a simplified implementation - proper absolute positioning requires more setup
                    log::debug!("Moving mouse to ({}, {})", event.x, event.y);
                }
                MouseEventType::LeftClick => {
                    self.device.click(&Controller::Mouse(Mouse::Left))?;
                    self.device.synchronize()?;
                }
                MouseEventType::LeftRelease => {
                    self.device.release(&Controller::Mouse(Mouse::Left))?;
                    self.device.synchronize()?;
                }
                MouseEventType::RightClick => {
                    self.device.click(&Controller::Mouse(Mouse::Right))?;
                    self.device.synchronize()?;
                }
                MouseEventType::RightRelease => {
                    self.device.release(&Controller::Mouse(Mouse::Right))?;
                    self.device.synchronize()?;
                }
                MouseEventType::MiddleClick => {
                    self.device.click(&Controller::Mouse(Mouse::Middle))?;
                    self.device.synchronize()?;
                }
                MouseEventType::MiddleRelease => {
                    self.device.release(&Controller::Mouse(Mouse::Middle))?;
                    self.device.synchronize()?;
                }
                MouseEventType::ScrollUp => {
                    // Scroll events need different handling in uinput
                    log::debug!("Scroll up event");
                }
                MouseEventType::ScrollDown => {
                    // Scroll events need different handling in uinput  
                    log::debug!("Scroll down event");
                }
            }
            
            Ok(())
        }
    }
}
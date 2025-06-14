use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct MouseEvent {
    pub x: f64,
    pub y: f64,
    pub event_type: MouseEventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

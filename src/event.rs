use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseEvent {
    Move { x: f64, y: f64 },
    LeftClick,
    RightClick,
    MiddleClick,
    LeftRelease,
    RightRelease,
    MiddleRelease,
    Scroll { delta_x: i64, delta_y: i64 },
}

use crate::config::{Config, Position};
use crate::event::MouseEvent;

#[derive(Debug, Clone)]
pub struct VirtualCoordinate {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct LocalCoordinate {
    pub x: f64,
    pub y: f64,
}

pub struct CoordinateTransformer {
    pub config: Config,
}

impl CoordinateTransformer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// ローカル座標 → 仮想座標変換
    pub fn local_to_virtual(&self, local: LocalCoordinate) -> VirtualCoordinate {
        match self.config.layout.position {
            Position::Left => VirtualCoordinate {
                x: local.x,
                y: local.y,
            },
            Position::Right => VirtualCoordinate {
                x: local.x + self.config.remote_screen.width as f64,
                y: local.y,
            },
            Position::Top => VirtualCoordinate {
                x: local.x,
                y: local.y,
            },
            Position::Bottom => VirtualCoordinate {
                x: local.x,
                y: local.y + self.config.remote_screen.height as f64,
            },
        }
    }

    /// 仮想座標 → ローカル座標変換
    pub fn virtual_to_local(&self, virtual_coord: VirtualCoordinate) -> LocalCoordinate {
        match self.config.layout.position {
            Position::Left => LocalCoordinate {
                x: virtual_coord.x,
                y: virtual_coord.y,
            },
            Position::Right => LocalCoordinate {
                x: virtual_coord.x - self.config.remote_screen.width as f64,
                y: virtual_coord.y,
            },
            Position::Top => LocalCoordinate {
                x: virtual_coord.x,
                y: virtual_coord.y,
            },
            Position::Bottom => LocalCoordinate {
                x: virtual_coord.x,
                y: virtual_coord.y - self.config.remote_screen.height as f64,
            },
        }
    }

    /// エッジ検出（仮想座標系で）
    pub fn is_at_transfer_edge(&self, local: &LocalCoordinate) -> bool {
        use crate::config::EdgeDirection;

        match self.config.edge.sender_to_receiver {
            EdgeDirection::Right => {
                // 自分が左側の場合、右端で転送
                matches!(self.config.layout.position, Position::Left)
                    && local.x >= (self.config.screen.width as f64 - 5.0)
            }
            EdgeDirection::Left => {
                // 自分が右側の場合、左端で転送
                matches!(self.config.layout.position, Position::Right) && local.x <= 5.0
            }
            EdgeDirection::Bottom => {
                // 自分が上側の場合、下端で転送
                matches!(self.config.layout.position, Position::Top)
                    && local.y >= (self.config.screen.height as f64 - 5.0)
            }
            EdgeDirection::Top => {
                // 自分が下側の場合、上端で転送
                matches!(self.config.layout.position, Position::Bottom) && local.y <= 5.0
            }
        }
    }

    /// 相手側での初期マウス位置を計算（エッジからの移行時）
    pub fn calculate_remote_entry_position(&self, local: &LocalCoordinate) -> LocalCoordinate {
        use crate::config::EdgeDirection;

        match self.config.edge.sender_to_receiver {
            EdgeDirection::Right => {
                // 右端から移行 → 相手の左端
                LocalCoordinate {
                    x: 5.0,
                    y: local.y.min(self.config.remote_screen.height as f64 - 1.0),
                }
            }
            EdgeDirection::Left => {
                // 左端から移行 → 相手の右端
                LocalCoordinate {
                    x: self.config.remote_screen.width as f64 - 5.0,
                    y: local.y.min(self.config.remote_screen.height as f64 - 1.0),
                }
            }
            EdgeDirection::Bottom => {
                // 下端から移行 → 相手の上端
                LocalCoordinate {
                    x: local.x.min(self.config.remote_screen.width as f64 - 1.0),
                    y: 5.0,
                }
            }
            EdgeDirection::Top => {
                // 上端から移行 → 相手の下端
                LocalCoordinate {
                    x: local.x.min(self.config.remote_screen.width as f64 - 1.0),
                    y: self.config.remote_screen.height as f64 - 5.0,
                }
            }
        }
    }

    /// 仮想画面全体のサイズを取得
    pub fn get_virtual_screen_size(&self) -> (u32, u32) {
        match self.config.layout.position {
            Position::Left | Position::Right => {
                let total_width = self.config.screen.width + self.config.remote_screen.width;
                let max_height = self
                    .config
                    .screen
                    .height
                    .max(self.config.remote_screen.height);
                (total_width, max_height)
            }
            Position::Top | Position::Bottom => {
                let max_width = self
                    .config
                    .screen
                    .width
                    .max(self.config.remote_screen.width);
                let total_height = self.config.screen.height + self.config.remote_screen.height;
                (max_width, total_height)
            }
        }
    }
}

impl From<MouseEvent> for LocalCoordinate {
    fn from(event: MouseEvent) -> Self {
        match event {
            MouseEvent::Move { x, y } => Self { x, y },
            _ => Self { x: 0.0, y: 0.0 }, // デフォルト値を使用（クリックなどの場合）
        }
    }
}

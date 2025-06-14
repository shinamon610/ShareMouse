use crate::config::{Config, HostPosition};
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
        match self.config.host_position {
            HostPosition::Left => VirtualCoordinate {
                x: local.x,
                y: local.y,
            },
            HostPosition::Right => VirtualCoordinate {
                x: local.x + self.config.remote_screen.width as f64,
                y: local.y,
            },
        }
    }

    /// 仮想座標 → ローカル座標変換
    pub fn virtual_to_local(&self, virtual_coord: VirtualCoordinate) -> LocalCoordinate {
        match self.config.host_position {
            HostPosition::Left => LocalCoordinate {
                x: virtual_coord.x,
                y: virtual_coord.y,
            },
            HostPosition::Right => LocalCoordinate {
                x: virtual_coord.x - self.config.remote_screen.width as f64,
                y: virtual_coord.y,
            },
        }
    }

    /// エッジ検出（ローカル座標で）
    pub fn is_at_transfer_edge(&self, local: &LocalCoordinate) -> bool {
        match self.config.host_position {
            HostPosition::Left => {
                // 左側画面の場合、右端に到達したら転送
                local.x >= (self.config.screen.width as f64 - 5.0)
            }
            HostPosition::Right => {
                // 右側画面の場合、左端に到達したら転送
                local.x <= 5.0
            }
        }
    }

    /// 相手側での初期マウス位置を計算（エッジからの移行時）
    pub fn calculate_remote_entry_position(&self, local: &LocalCoordinate) -> LocalCoordinate {
        match self.config.host_position {
            HostPosition::Left => {
                // 左側画面の右端から移行 → 相手の左端
                LocalCoordinate {
                    x: 5.0,
                    y: local.y.min(self.config.remote_screen.height as f64 - 1.0),
                }
            }
            HostPosition::Right => {
                // 右側画面の左端から移行 → 相手の右端
                LocalCoordinate {
                    x: self.config.remote_screen.width as f64 - 5.0,
                    y: local.y.min(self.config.remote_screen.height as f64 - 1.0),
                }
            }
        }
    }

    /// 仮想画面全体のサイズを取得
    pub fn get_virtual_screen_size(&self) -> (u32, u32) {
        // 左右配置のみ対応
        let total_width = self.config.screen.width + self.config.remote_screen.width;
        let max_height = self
            .config
            .screen
            .height
            .max(self.config.remote_screen.height);
        (total_width, max_height)
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

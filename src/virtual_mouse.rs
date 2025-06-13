use crate::config::Config;
use crate::coordinate::{CoordinateTransformer, LocalCoordinate, VirtualCoordinate};
use crate::capturer::{MouseEvent, MouseEventType};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy)]
pub enum ControlSide {
    Local,  // macOS側制御中
    Remote, // Linux側制御中
}

#[derive(Debug, Clone)]
pub struct VirtualMouse {
    // 仮想座標系での現在位置
    pub virtual_position: VirtualCoordinate,
    // 現在どちら側が制御中か
    pub control_side: ControlSide,
    // 前回の物理マウス位置（移動量計算用）
    pub last_physical_position: Option<LocalCoordinate>,
}

impl VirtualMouse {
    pub fn new(config: &Config) -> Self {
        let transformer = CoordinateTransformer::new(config.clone());
        
        // 初期位置：自分の画面の中央
        let initial_local = LocalCoordinate {
            x: config.screen.width as f64 / 2.0,
            y: config.screen.height as f64 / 2.0,
        };
        let initial_virtual = transformer.local_to_virtual(initial_local.clone());
        
        Self {
            virtual_position: initial_virtual,
            control_side: ControlSide::Local,
            last_physical_position: Some(initial_local),
        }
    }
    
    /// 物理マウス位置から仮想座標を更新（Local制御時）
    pub fn update_from_physical(&mut self, physical_pos: LocalCoordinate, delta: Option<(f64, f64)>, transformer: &CoordinateTransformer) {
        match self.control_side {
            ControlSide::Local => {
                // Local側制御時：物理位置を仮想座標に変換
                self.virtual_position = transformer.local_to_virtual(physical_pos.clone());
                log::debug!("Local control: physical ({}, {}) -> virtual ({}, {})", 
                           physical_pos.x, physical_pos.y, self.virtual_position.x, self.virtual_position.y);
                self.last_physical_position = Some(physical_pos);
            }
            ControlSide::Remote => {
                // Remote側制御時：OSの生の移動量をそのまま仮想座標に適用
                if let Some((delta_x, delta_y)) = delta {
                    log::debug!("Remote control: using OS delta ({}, {})", delta_x, delta_y);
                    
                    let old_virtual = self.virtual_position.clone();
                    self.virtual_position.x += delta_x;
                    self.virtual_position.y += delta_y;
                    
                    // 仮想画面境界内に制限
                    let (virtual_width, virtual_height) = transformer.get_virtual_screen_size();
                    self.virtual_position.x = self.virtual_position.x.max(0.0).min(virtual_width as f64 - 1.0);
                    self.virtual_position.y = self.virtual_position.y.max(0.0).min(virtual_height as f64 - 1.0);
                    
                    log::debug!("Remote control: virtual ({}, {}) -> ({}, {})", 
                               old_virtual.x, old_virtual.y, self.virtual_position.x, self.virtual_position.y);
                } else {
                    log::warn!("Remote control but no delta information available");
                }
                self.last_physical_position = Some(physical_pos);
            }
        }
    }
    
    /// 制御権を切り替える
    pub fn switch_control(&mut self, new_side: ControlSide, current_physical_pos: &LocalCoordinate) {
        if self.control_side != new_side {
            log::info!("Control switched from {:?} to {:?}", self.control_side, new_side);
            self.control_side = new_side;
            
            // 制御権切り替え時に物理マウス位置をリセット
            // これで次回のdelta計算が正しく動作する
            self.last_physical_position = Some(current_physical_pos.clone());
            log::info!("Reset physical position to ({}, {}) for new control", 
                      current_physical_pos.x, current_physical_pos.y);
        }
    }
    
    /// 現在の制御領域を判定（物理座標も考慮）
    pub fn determine_control_side(&self, transformer: &CoordinateTransformer, physical_pos: &LocalCoordinate) -> ControlSide {
        use crate::config::Position;
        
        match transformer.config.layout.position {
            Position::Left => {
                // 自分が左側：仮想X座標でどちら側か判定
                if self.virtual_position.x < transformer.config.screen.width as f64 {
                    ControlSide::Local
                } else {
                    ControlSide::Remote
                }
            }
            Position::Right => {
                // 自分が右側：物理座標が左端近くなら強制的にRemote制御
                if physical_pos.x <= 5.0 {
                    ControlSide::Remote
                } else if self.virtual_position.x >= transformer.config.remote_screen.width as f64 {
                    ControlSide::Local
                } else {
                    ControlSide::Remote
                }
            }
            Position::Top => {
                // 自分が上側：仮想Y座標でどちら側か判定
                if self.virtual_position.y < transformer.config.screen.height as f64 {
                    ControlSide::Local
                } else {
                    ControlSide::Remote
                }
            }
            Position::Bottom => {
                // 自分が下側：仮想Y座標でどちら側か判定
                if self.virtual_position.y >= transformer.config.remote_screen.height as f64 {
                    ControlSide::Local
                } else {
                    ControlSide::Remote
                }
            }
        }
    }
    
    /// 現在位置に基づいてローカル座標を取得
    pub fn get_local_coordinate(&self, transformer: &CoordinateTransformer) -> Option<LocalCoordinate> {
        match self.control_side {
            ControlSide::Local => {
                Some(transformer.virtual_to_local(self.virtual_position.clone()))
            }
            ControlSide::Remote => {
                // Remote制御時はローカル座標なし
                None
            }
        }
    }
    
    /// Linux側への送信用ローカル座標を取得
    pub fn get_remote_coordinate(&self, transformer: &CoordinateTransformer) -> Option<LocalCoordinate> {
        match self.control_side {
            ControlSide::Remote => {
                // 仮想座標をリモート側のローカル座標に変換
                let remote_local = self.virtual_to_remote_local(transformer);
                Some(remote_local)
            }
            ControlSide::Local => None,
        }
    }
    
    /// 仮想座標をリモート側のローカル座標に変換
    fn virtual_to_remote_local(&self, transformer: &CoordinateTransformer) -> LocalCoordinate {
        use crate::config::Position;
        
        match transformer.config.layout.remote_position {
            Position::Left => LocalCoordinate {
                x: self.virtual_position.x,
                y: self.virtual_position.y.min(transformer.config.remote_screen.height as f64 - 1.0),
            },
            Position::Right => LocalCoordinate {
                x: self.virtual_position.x - transformer.config.screen.width as f64,
                y: self.virtual_position.y.min(transformer.config.remote_screen.height as f64 - 1.0),
            },
            Position::Top => LocalCoordinate {
                x: self.virtual_position.x.min(transformer.config.remote_screen.width as f64 - 1.0),
                y: self.virtual_position.y,
            },
            Position::Bottom => LocalCoordinate {
                x: self.virtual_position.x.min(transformer.config.remote_screen.width as f64 - 1.0),
                y: self.virtual_position.y - transformer.config.screen.height as f64,
            },
        }
    }
    
    /// 制御権移譲時のマウスイベントを作成
    pub fn create_transfer_event(&self, transformer: &CoordinateTransformer) -> Option<MouseEvent> {
        if let Some(remote_coord) = self.get_remote_coordinate(transformer) {
            Some(MouseEvent {
                x: remote_coord.x,
                y: remote_coord.y,
                delta_x: None,
                delta_y: None,
                event_type: MouseEventType::Move,
            })
        } else {
            None
        }
    }
}

impl PartialEq for ControlSide {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), 
                 (ControlSide::Local, ControlSide::Local) | 
                 (ControlSide::Remote, ControlSide::Remote))
    }
}

/// スレッドセーフな仮想マウス状態管理
pub type SharedVirtualMouse = Arc<Mutex<VirtualMouse>>;
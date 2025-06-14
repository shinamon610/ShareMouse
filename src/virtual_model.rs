use std::sync::{Arc, Mutex};

use crate::config::Config;

/// 仮想マウスモデル - virtual_xとvirtual_yを管理
pub struct VirtualModel {
    pub virtual_x: f64,
    pub virtual_y: f64,
    pub config: Config,
}
impl VirtualModel {
    pub fn new(config: Config) -> Self {
        Self {
            virtual_x: 0.0,
            virtual_y: 0.0,
            config: config,
        }
    }
    pub fn init(&mut self, x: f64, y: f64) {
        self.virtual_x = x;
        self.virtual_y = y;
    }
}

/// スレッドセーフなVirtualModel
pub type SharedVirtualModel = Arc<Mutex<VirtualModel>>;

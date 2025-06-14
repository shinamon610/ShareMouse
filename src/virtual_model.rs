use std::sync::{Arc, Mutex};

use crate::config::Config;

/// 仮想マウスモデル - virtual_xとvirtual_yを管理
pub struct VirtualModel {
    pub virtual_x: f64,
    pub virtual_y: f64,
    pub is_init: bool,
    pub config: Config,
}
impl VirtualModel {
    pub fn new(config: Config) -> Self {
        Self {
            virtual_x: 0.0,
            virtual_y: 0.0,
            is_init: true,
            config: config,
        }
    }
}

/// スレッドセーフなVirtualModel
pub type SharedVirtualModel = Arc<Mutex<VirtualModel>>;

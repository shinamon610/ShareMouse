use std::sync::{Arc, Mutex};

use crate::config::{Config, HostPosition};

/// 仮想マウスモデル - virtual_xとvirtual_yを管理
pub struct VirtualModel {
    pub virtual_x: f64,
    pub virtual_y: f64,
}

fn inner_crop(target: f64, max: f64) -> f64 {
    target.max(0.0).min(max)
}

impl VirtualModel {
    pub fn new(config: Config) -> Self {
        Self {
            virtual_x: 0.0,
            virtual_y: 0.0,
        }
    }
    pub fn init(&mut self, config: &Config, x: f64, y: f64) {
        self.virtual_x = if config.host_position == HostPosition::Right {
            x + config.remote_screen.width as f64
        } else {
            x
        };

        self.virtual_y = y;
    }
    pub fn in_host(&self, config: &Config) -> bool {
        if config.host_position == HostPosition::Right {
            return config.remote_screen.width as f64 <= self.virtual_x;
        }
        return config.screen.width as f64 <= self.virtual_x;
    }
    pub fn crop(&self, config: &Config, x: f64, y: f64) -> (f64, f64) {
        let n_x = inner_crop(x, (config.screen.width + config.remote_screen.width) as f64);
        let n_y = inner_crop(y, config.remote_screen.height as f64);
        return (n_x, n_y);
    }
    pub fn update(&mut self, config: &Config, x: f64, y: f64) {
        if self.in_host(config) {
            self.virtual_x = x;
            self.virtual_y = y;
            return;
        }
        let (center_x, center_y) = config.host_center();
        let d_x = x - center_x;
        let d_y = y - center_y;
        let (n_x, n_y) = self.crop(config, self.virtual_x + d_x, self.virtual_y + d_y);
        self.virtual_x = n_x;
        self.virtual_y = n_y;
    }
}

/// スレッドセーフなVirtualModel
pub type SharedVirtualModel = Arc<Mutex<VirtualModel>>;

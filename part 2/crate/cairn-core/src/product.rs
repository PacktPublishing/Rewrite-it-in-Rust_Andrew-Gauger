use crate::status::UnknownStatus;
use serde::{Deserialize, Serialize};
use std::fmt;

/// `VL_CATEGORY`: Sensor;Gateway;Module;Accessory;Service;Kit
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub enum Category {
    Sensor,
    Gateway,
    Module,
    Accessory,
    Service,
    Kit,
}

impl Category {
    /// R37: what the manufacturer builds. Service is Cairn's own work.
    pub fn is_hardware(self) -> bool {
        match self {
            Category::Sensor
            | Category::Gateway
            | Category::Module
            | Category::Accessory
            | Category::Kit => true,
            Category::Service => false,
        }
    }
}

impl TryFrom<&str> for Category {
    type Error = UnknownStatus;

    fn try_from(s: &str) -> Result<Category, UnknownStatus> {
        Ok(match s {
            "Sensor" => Category::Sensor,
            "Gateway" => Category::Gateway,
            "Module" => Category::Module,
            "Accessory" => Category::Accessory,
            "Service" => Category::Service,
            "Kit" => Category::Kit,
            _ => return Err(UnknownStatus(s.to_string())),
        })
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

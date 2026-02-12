use std::collections::HashMap;

/// Represents the status of a device based on various discovery methods.
#[derive(Debug, Default, Clone)]
pub struct DeviceStatus {
    pub in_udev: bool,
    pub in_sysfs: bool,
    pub hw_responding: bool,
    pub driver_bound: Option<String>, // Some("rk808") or None
}

/// Specific details for different hardware buses
#[derive(Debug, Clone)]
pub enum DeviceKind {
    I2c { bus: u32, address: u16 },
    Usb { port: u32, vendor_id: u16, product_id: u16 },
}

/// Device class
#[derive(Debug, Clone)]
pub struct TuxDevice {
    pub name: String,
    pub kind: DeviceKind,
    pub status: DeviceStatus,
    pub attributes: HashMap<String, String>, // Extra optional info
}
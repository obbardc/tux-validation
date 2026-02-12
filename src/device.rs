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
    I2c { bus: u8, address: u16 },
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

impl TuxDevice {
    /// Create a device instance from a udev entry
    pub fn from_udev(dev: &udev::Device) -> Option<Self> {
        // Decide if clent device
        let parent = dev.parent().expect("No parent!");
        let parent_sysname = parent.sysname().to_str().unwrap_or("");
        let parent_name_parts:Vec<&str> = parent_sysname.split('-').collect();
        let kind = if parent_name_parts[0] == "i2c" {
            let dev_name_parts:Vec<&str> = dev.sysname().to_str().unwrap_or("").split('-').collect();
            let bus = dev_name_parts[0].parse().ok()?;
            let addr = u16::from_str_radix(dev_name_parts[1], 16).ok()?;
            DeviceKind::I2c { bus: bus, address: addr }
        } else {
            return None; // Skip adapters/masters for this list
        };

        let driver = dev.driver().map_or("", |s| { s.to_str().unwrap_or("") });
        
        Some(TuxDevice {
            name: dev.attribute_value("name")
                .map_or("", |v| {v.to_str().unwrap_or("")}).to_string(),
            kind,
            status: DeviceStatus {
                in_udev: true,
                in_sysfs: true, // If udev sees it, sysfs has it?
                hw_responding: false, // To be filled by hw_probe
                driver_bound: Some(driver.to_string())
            },
            attributes: HashMap::new(),
        })
    }
}
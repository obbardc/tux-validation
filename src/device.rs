use std::collections::HashMap;
use serde::Serialize;

/// Represents the status of a device based on various discovery methods.
#[derive(Debug, Default, Clone, Serialize)]
pub struct DeviceStatus {
    pub in_udev: bool,
    pub in_sysfs: bool,
    pub hw_responding: bool,
    pub driver_bound: Option<String>, // Some("rk808") or None
}

#[derive(Debug, Clone, Serialize)]
pub enum DeviceAddress {
    I2c { bus: u8, address: u16 }, // e.g. {7, 0x000a}
    Usb { port: String }, // e.g. "1-1.2"
    Pci { slot: String }, // e.g. "00:02.0"
}

impl DeviceAddress {
    /// Returns the I2C address if this is an I2C device, otherwise None
    pub fn as_i2c_address(&self) -> Option<u16> {
        if let Self::I2c { address, .. } = self {
            Some(*address)
        } else {
            None
        }
    }
}

/// Device class
#[derive(Debug, Clone, Serialize)]
pub struct TuxDevice {
    pub name: String,
    pub address: DeviceAddress,
    pub status: DeviceStatus,
    pub attributes: HashMap<String, String>, // Extra optional info
}

#[derive(Debug, Serialize)]
pub enum Subsystem {
    I2c,
    Usb,
    Pci,
    Gpio,
}

///TODO: Does it make sense?
#[derive(Debug, Serialize)]
pub enum BusStatus {
    Active,
    Inactive,
    Missing
}

/// Hardware group (bus/controller/adaptor) class
pub struct TuxBus {
    pub name: String,           // e.g., "i2c-7"
    pub subsystem: Subsystem,   // Enum: I2c, Usb, Pci
    pub id: String,             // e.g. 7 as in i2c-7
    pub devices: Vec<TuxDevice>,
    pub status: BusStatus,      // Is the controller itself healthy?
    pub metadata: HashMap<String, String>   // For various metadata
}

impl TuxDevice {
    /// Create a device instance from a udev entry
    pub fn from_udev(dev: &udev::Device) -> Option<Self> {
        // Decide if clent device
        let parent = dev.parent().expect("No parent!");
        let parent_sysname = parent.sysname().to_str().unwrap_or("");
        let parent_name_parts:Vec<&str> = parent_sysname.split('-').collect();
        let address = if parent_name_parts[0] == "i2c" {
            let dev_name_parts:Vec<&str> = dev.sysname().to_str().unwrap_or("").split('-').collect();
            let bus = dev_name_parts[0].parse().ok()?;
            let addr = u16::from_str_radix(dev_name_parts[1], 16).ok()?;
            DeviceAddress::I2c { bus: bus, address: addr }
        } else {
            return None; // Skip adapters/masters for this list
        };

        let driver = dev.driver().map_or("", |s| { s.to_str().unwrap_or("") });
        
        Some(TuxDevice {
            name: dev.attribute_value("name")
                .map_or("", |v| {v.to_str().unwrap_or("")}).to_string(),
            address,
            status: DeviceStatus {
                in_udev: true,
                in_sysfs: true, // If udev sees it, sysfs has it?
                hw_responding: false, // To be filled by hw_probe
                driver_bound: Some(driver.to_string())
            },
            attributes: HashMap::new(),
        })
    }

    /// Print device details in JSON format
    pub fn print_json(&self) -> anyhow::Result<()> {
        let device_json = serde_json::to_string_pretty(self)?;
        println!("{}", device_json);
        Ok(())
    }
}
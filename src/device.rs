use serde::{Serialize, Serializer};
use std::collections::HashMap;

/// Represents the status of a device based on various discovery methods.
#[derive(Debug, Default, Clone, Serialize)]
pub struct DeviceStatus {
    pub in_udev: bool,
    pub hw_responding: bool,
    pub driver_bound: Option<String>, // Some("rk808") or None
}

#[derive(Debug, Clone, Serialize)]
pub enum DeviceAddress {
    I2c {
        bus: u8,
        #[serde(serialize_with = "to_hex")]
        address: u16, // e.g. {7, 0x000a}
    },
    Usb {
        port: String,
    }, // e.g. "1-1.2"
    Pci {
        slot: String,
    }, // e.g. "00:02.0"
}

/// Custom serializer to turn numbers into 0xYY hex strings
fn to_hex<S>(value: &u16, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("0x{:02x}", value))
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
    Missing,
}

/// Hardware group (bus/controller/adaptor) class
pub struct TuxBus {
    pub name: String,         // e.g., "i2c-7"
    pub subsystem: Subsystem, // Enum: I2c, Usb, Pci
    pub id: String,           // e.g. 7 as in i2c-7
    pub devices: Vec<TuxDevice>,
    pub status: BusStatus,                 // Is the controller itself healthy?
    pub metadata: HashMap<String, String>, // For various metadata
}

impl TuxDevice {
    /// Create a device instance from a udev entry
    pub fn from_udev(dev: &udev::Device) -> Option<Self> {
        // Decide if clent device
        let parent = dev.parent()?;
        let parent_sysname = parent.sysname().to_str()?;

        let address = if parent_sysname.starts_with("i2c-") {
            // --- I2C LOGIC ---
            let bus = parent_sysname.strip_prefix("i2c-")?.parse::<u8>().ok()?;
            let dev_sysname = dev.sysname().to_str()?;
            let addr_str = dev_sysname.split('-').nth(1)?;
            let addr = match u16::from_str_radix(addr_str, 16) {
                Ok(val) => val,
                Err(_) => {
                    //To catch ACPI/non-hex addresses
                    eprintln!(
                        "Skipping I2C device with non-hex address format: '{}' (Bus: {})",
                        dev_sysname, bus
                    );
                    return None;
                }
            };
            DeviceAddress::I2c { bus, address: addr }
        } else if parent_sysname.contains("usb") {
            // --- FUTURE USB LOGIC ---
            return None;
        } else {
            return None;
        };

        let driver = dev.driver().and_then(|s| s.to_str()).map(|s| s.to_string());
        let name = dev
            .attribute_value("name")
            .and_then(|v| v.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Some(TuxDevice {
            name,
            address,
            status: DeviceStatus {
                in_udev: true,
                hw_responding: false, // To be filled by hw_probe
                driver_bound: driver,
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

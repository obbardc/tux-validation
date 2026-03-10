use serde::{Serialize, Serializer};
use std::collections::HashMap;

/// Represents the status of a device based on various discovery methods.
#[derive(Debug, Default, Clone, Serialize)]
pub struct DeviceStatus {
    pub in_udev: bool,
    pub hw_responding: Option<bool>,
    pub driver_bound: Option<String>, // Some("rk808") or None
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "props")]
pub enum DeviceAddress {
    I2c {
        bus: u8,
        #[serde(serialize_with = "to_hex")]
        address: u16, // e.g. {7, 0x000a}
    },
    Usb {
        bus: u8,
        port_path: String, // e.g., "1-1.2" (Bus 1, Port 1, Hub Port 2)
        vid: String,       // "046d"
        pid: String,       // "c05a"
    },
    Ethernet {
        interface: String, // e.g., "eth0"
        mac: String,       // e.g., "00:1a:2b:3c:4d:5e"
    },
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
    /// Returns the USB port_path if this is a USB device, otherwise None
    pub fn as_usb_port_path(&self) -> Option<String> {
        if let Self::Usb { port_path, .. } = self {
            Some(port_path.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "props")]
pub enum DeviceDetails {
    Usb(UsbProperties),
    I2c(I2cProperties),
    Ethernet(EthernetProperties),
    None,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsbProperties {
    pub speed: String,
    pub interfaces: Vec<UsbInterface>,
    pub dev_num: u8,
    pub serial_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsbInterface {
    pub if_num: u32,
    pub class: String,
    pub driver: Option<String>,
}

//TODO: need it?
#[derive(Debug, Clone, Serialize)]
pub struct I2cProperties;

#[derive(Debug, Clone, Serialize)]
pub struct EthernetProperties {
    // --- Hardware Layer ---
    pub speed: u32,           // 10, 100, 1000
    pub duplex: String,       // "full", "half"
    pub link_detected: bool,  // Physical Carrier
    pub pci_bus_id: Option<String>,
    // --- Software/Network Layer ---
    pub operstate: String,    // "up", "down", "testing"
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub dhcp_enabled: bool,
    pub firmware_version: Option<String>,
}

/// Device class
#[derive(Debug, Clone, Serialize)]
pub struct TuxDevice {
    pub name: String,
    pub address: DeviceAddress,
    pub status: DeviceStatus,
    pub details: DeviceDetails,
    pub children: Vec<TuxDevice>,
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
        //TODO: might be better to use Visitor design pattern.
        let dev_sysname = dev.sysname().to_str()?;
        let subsystem = dev.subsystem()?.to_str()?;
        let parent = dev.parent()?;
        let parent_sysname = parent.sysname().to_str()?;
        let address = if parent_sysname.starts_with("i2c-") {
            // --- I2C LOGIC ---
            let bus = parent_sysname.strip_prefix("i2c-")?.parse::<u8>().ok()?;
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
        } else if dev.devtype().is_some_and(|t| t == "usb_device") {
            // --- USB LOGIC ---
            DeviceAddress::Usb {
                bus: dev.attribute_value("busnum")?.to_str()?.parse().ok()?,
                port_path: dev_sysname.to_string(),
                vid: dev.attribute_value("idVendor")?.to_str()?.to_string(),
                pid: dev.attribute_value("idProduct")?.to_str()?.to_string(),
            }
        } else if subsystem == "net" {
            let mac = dev.attribute_value("address")
                .and_then(|v| v.to_str())
                .unwrap_or("00:00:00:00:00:00")
                .to_string();
            
            DeviceAddress::Ethernet {
                interface: dev_sysname.to_string(),
                mac,
            }
        } else {
            return None;
        };

        // TODO: Should we be collecting all device attributes or this is not reasonable and
        //  then it's just easier to keep the udev::Device objects?
        let driver = if let Some(d) = dev.driver() {
            d.to_str().map(|s| s.to_string())
        } else if subsystem == "net" {
        // If we're in 'net', explicitly look at the parent
            dev.parent().and_then(|parent| {
                parent.driver().and_then(|d| d.to_str().map(|s| s.to_string()))
            })
        } else {
            None
        };
        let name = match &address {
            DeviceAddress::I2c { .. } => dev
                .attribute_value("name")
                .and_then(|v| v.to_str())
                .unwrap_or("Unknown I2C Device")
                .to_string(),

            DeviceAddress::Usb { .. } => dev
                .attribute_value("product")
                .and_then(|v| v.to_str())
                .unwrap_or("Unknown USB Device")
                .to_string(),

            DeviceAddress::Ethernet { interface, .. } => interface.clone(),

            DeviceAddress::Pci { .. } => {
                // To be elaborated later
                "PCI Device".to_string()
            },

            _ => "Unknown Device".to_string(),
        };

        Some(TuxDevice {
            name,
            address,
            status: DeviceStatus {
                in_udev: true,
                hw_responding: None, // To be filled by hw_probe
                driver_bound: driver,
            },
            details: DeviceDetails::None,
            children: Vec::new(),
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

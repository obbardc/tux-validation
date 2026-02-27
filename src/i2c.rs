use crate::device::{
    BusStatus, DeviceAddress, DeviceDetails, DeviceStatus, I2cProperties, Subsystem, TuxBus,
    TuxDevice,
};
use crate::config::I2cExpectation;
use anyhow::Result;
use i2cdev::core::*;
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};
use nix::errno::Errno;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use udev::Enumerator;
use colored::*;

/// Finds all available i2c devices in /dev.
///
/// Returns the list of found devices.
pub fn discover_buses() -> Result<Vec<PathBuf>> {
    let mut buses = Vec::new();
    for entry in fs::read_dir("/dev")? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if name.starts_with("i2c-") {
            buses.push(path);
        }
    }
    // Sort them so they appear as i2c-0, i2c-1, i2c-2, .. i2c-10, ..
    buses.sort_by_key(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.strip_prefix("i2c-"))
            .and_then(|x| x.parse::<u8>().ok())
            .unwrap_or(0)
    });
    Ok(buses)
}

pub trait I2cScanner {
    fn scan_hw_probe(&self) -> Result<(Vec<u16>, Vec<u16>)>; // TODO: add address range as parameter
    fn scan_sysfs(&self) -> Result<Vec<u16>>; // TODO: add address range as parameter
}

/// A specific I2C bus scanner.
pub struct LinuxI2cScanner {
    pub bus_id: u8,
}

impl I2cScanner for LinuxI2cScanner {
    /// Scans a given I2C bus ID via hardware probe (smbus_write_quick).
    ///
    /// Might potentially be disruptive for the bus.
    /// TODO: add some kind of safety check?
    fn scan_hw_probe(&self) -> Result<(Vec<u16>, Vec<u16>)> {
        let mut unbound = Vec::new();
        let mut bound = Vec::new();
        let bus_path = format!("/dev/i2c-{}", self.bus_id);

        for addr in 0x08..=0x77 {
            match LinuxI2CDevice::new(&bus_path, addr) {
                Ok(mut dev) => {
                    if dev.smbus_write_quick(false).is_ok() {
                        unbound.push(addr);
                    }
                }
                Err(e) => match e {
                    LinuxI2CError::Errno(code) => {
                        let errno = Errno::from_i32(code);
                        if errno == Errno::EBUSY {
                            bound.push(addr);
                        } else {
                            eprintln!("Unexpected Errno at 0x{:02x}: {}", addr, errno);
                        }
                    }
                    LinuxI2CError::Io(io_err) => match io_err.kind() {
                        std::io::ErrorKind::NotFound => {
                            anyhow::bail!("Bus {} not found at {}", self.bus_id, bus_path);
                        }
                        std::io::ErrorKind::PermissionDenied => {
                            anyhow::bail!("Permission denied accessing {}. Try sudo.", bus_path);
                        }
                        _ => {
                            eprintln!("IO Error at 0x{:02x}: {}", addr, io_err);
                        }
                    },
                },
            }
        }
        Ok((unbound, bound))
    }

    /// Scans /sys/bus/i2c-xxx for kernel-recognised devices.
    fn scan_sysfs(&self) -> Result<Vec<u16>> {
        let mut detected = Vec::new();

        for addr in 0x08..=0x77 {
            let base_path = format!("/sys/bus/i2c/devices/{}-{:04x}", &self.bus_id, addr);
            if Path::new(&base_path).exists() {
                detected.push(addr);
            }
        }
        Ok(detected)
    }
}

/// Holds results of an I2C bus scan for specific addresses.
pub struct I2cValidationResult {
    pub missing: Vec<u16>,
    pub unexpected: Vec<u16>,
    pub present: Vec<u16>,
    pub probed: Vec<u16>,
}

/// Scan an I2C bus and check for specific device addresses.
///
/// TODO: Uses scan_sysfs - can now upgrade to use udev
pub fn validate_bus(
    scanner: &impl I2cScanner,
    expected_addresses: &[u16],
    enable_hw_probe: bool,
) -> Result<I2cValidationResult> {
    let (hw_unbound, hw_bound) = if enable_hw_probe {
        scanner.scan_hw_probe()?
    } else {
        (Vec::new(), Vec::new())
    };
    let detected_sysfs = scanner.scan_sysfs()?;

    let mut result = I2cValidationResult {
        missing: Vec::new(),
        unexpected: Vec::new(),
        present: Vec::new(),
        probed: Vec::new(),
    };

    for &addr in expected_addresses {
        if hw_unbound.contains(&addr) || hw_bound.contains(&addr) {
            result.present.push(addr);
            result.probed.push(addr);
        } else if detected_sysfs.contains(&addr) {
            result.present.push(addr);
        } else {
            result.missing.push(addr);
        }
    }

    for &addr in &hw_unbound {
        if !expected_addresses.contains(&addr) {
            result.unexpected.push(addr);
            result.probed.push(addr);
        }
    }

    for &addr in &hw_bound {
        if !expected_addresses.contains(&addr) {
            result.unexpected.push(addr);
            result.probed.push(addr);
        }
    }

    for &addr in &detected_sysfs {
        if !expected_addresses.contains(&addr) && !result.unexpected.contains(&addr) {
            result.unexpected.push(addr);
        }
    }

    Ok(result)
}

/// Holds results of the I2C subsystem full scan (both hw probe and sysfs).
pub struct I2cBusReport {
    pub bus_path: String,
    pub kernel_detected: Vec<u16>,  // From /sys
    pub hardware_unbound: Vec<u16>, // From smbus_write_quick - unbound
    pub hardware_bound: Vec<u16>,   // From smbus_write_quick - bound to a driver
}

/// Returns either `name` or entry from `uevent` of a particular I2C device.
pub fn get_device_info(bus_id: u32, addr: u16) -> String {
    let base_path = format!("/sys/bus/i2c/devices/{}-{:04x}", bus_id, addr);
    let name_path = format!("{}/name", base_path);
    let uevent_path = format!("{}/uevent", base_path);

    // 1. Try the 'name' file first
    if let Ok(name) = fs::read_to_string(name_path) {
        return name.trim().to_string();
    }

    // 2. Fallback: Parse 'uevent'
    if let Ok(uevent) = fs::read_to_string(uevent_path) {
        for line in uevent.lines() {
            if line.starts_with("OF_COMPATIBLE_0=") {
                return line
                    .split('=')
                    .nth(1)
                    .unwrap_or("Unknown")
                    .split(',')
                    .next_back() // e.g. get 'rk808' from 'rockchip,rk808'
                    .unwrap_or("Unknown")
                    .to_string();
            }
        }
    }

    "Unidentified".to_string()
}

/// Performs full scan of I2C subsystem for the full range of addresses.
///
/// Both sysfs scan and harware probes (optional, via smbus_quick_write) are performed.
pub fn full_system_scan(enable_hw_probe: bool) -> Result<Vec<I2cBusReport>> {
    let busses = discover_buses()?;
    let mut reports = Vec::new();

    for path in busses {
        let bus_str = path.to_string_lossy().to_string();
        let bus_id: u8 = bus_str
            .strip_prefix("/dev/i2c-")
            .and_then(|x| x.parse::<u8>().ok())
            .expect("invalid bus string");
        let scanner = LinuxI2cScanner { bus_id };

        // 1. Live Hardware Probe - not super Rust-idiomatic but will do
        let (hw_unbound, hw_bound) = if enable_hw_probe {
            scanner.scan_hw_probe()?
        } else {
            (Vec::new(), Vec::new())
        };

        // 2. Sysfs check
        let knl_detected = scanner.scan_sysfs()?;

        reports.push(I2cBusReport {
            bus_path: bus_str,
            kernel_detected: knl_detected,
            hardware_unbound: hw_unbound,
            hardware_bound: hw_bound,
        });
    }
    Ok(reports)
}

/// Utility function, printing out the (static) device list (debug) data from udev.
pub fn scan_i2c_subsystem_with_udev() -> Result<()> {
    let mut enumerator = Enumerator::new()?;

    // We filter for the "i2c" subsystem to avoid seeing USB, PCI, etc.
    enumerator.match_subsystem("i2c")?;

    for device in enumerator.scan_devices()? {
        println!();
        println!("{:#?}", device);

        println!("  [properties]");
        for property in device.properties() {
            println!("    - {:?} {:?}", property.name(), property.value());
        }

        println!("  [attributes]");
        for attribute in device.attributes() {
            println!("    - {:?} {:?}", attribute.name(), attribute.value());
        }
    }

    Ok(())
}

/// Sweeps through udev records for I2C clients and returns a {bus: device} map.
pub fn get_i2c_udev_map() -> Result<HashMap<u8, Vec<udev::Device>>> {
    let mut map = HashMap::new();
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("i2c")?;
    for device in enumerator.scan_devices()? {
        let name = device.sysname().to_str().unwrap_or("");
        // Add a bus
        if name.starts_with("i2c-")
            && name.matches('-').count() == 1
            && let Ok(id) = name[4..].parse::<u8>()
        {
            // Initialize the entry so the bus exists in the map
            map.entry(id).or_insert_with(Vec::new);
        }
        // Add devices
        if let Some(parent) = device.parent() {
            let parent_name = parent.sysname().to_str().unwrap_or("");
            if let Some(id_str) = parent_name.strip_prefix("i2c-")
                && let Ok(bus_id) = id_str.parse::<u8>()
            {
                map.entry(bus_id).or_insert_with(Vec::new).push(device);
            }
        }
    }
    Ok(map)
}

/// Performs full audit (scan) of the I2C subsystem via udev and return found device info.
///
/// Optionally performs hardware probe (via SMBus Write Quick) - this probes the full range
///  of possible addresses (so can find "ghost" devices not in udev).
pub fn audit_all_i2c_buses(enable_hw_probe: bool) -> anyhow::Result<Vec<TuxBus>> {
    let udev_map = get_i2c_udev_map()?;
    let mut board_report = Vec::new();

    for (bus_id, devices) in udev_map {
        let mut bus_node = TuxBus {
            name: format!("i2c-{}", bus_id),
            subsystem: Subsystem::I2c,
            id: bus_id.to_string(),
            devices: Vec::new(),
            status: BusStatus::Active,
            metadata: HashMap::new(),
        };

        // Perform hardware probe
        let scanner = LinuxI2cScanner { bus_id };
        let (unbound_hw, bound_hw) = if enable_hw_probe {
            scanner.scan_hw_probe()?
        } else {
            (Vec::new(), Vec::new())
        };

        // Cross-reference with udev inventory
        for dev in devices {
            if let Some(mut t_dev) = TuxDevice::from_udev(&dev) {
                if let Some(addr) = t_dev.address.as_i2c_address() {
                    t_dev.status.hw_responding =
                        bound_hw.contains(&addr) || unbound_hw.contains(&addr);
                }
                bus_node.devices.push(t_dev);
            }
        }

        // Find ghosts (In HW but not in udev)
        for addr in unbound_hw {
            if !bus_node
                .devices
                .iter()
                .any(|d| d.address.as_i2c_address().unwrap() == addr)
            {
                bus_node.devices.push(TuxDevice {
                    name: String::from("Unknown"),
                    address: DeviceAddress::I2c {
                        bus: bus_id,
                        address: addr,
                    },
                    status: DeviceStatus {
                        in_udev: false,
                        hw_responding: true,
                        driver_bound: None,
                    },
                    details: DeviceDetails::I2c(I2cProperties),
                    children: Vec::new(),
                    attributes: HashMap::new(),
                });
            }
        }

        board_report.push(bus_node);
    }

    board_report.sort_by_key(|bus| bus.id.parse::<u8>().unwrap_or(0));

    Ok(board_report)
}

pub fn print_and_verify_i2c(buses: &[TuxBus], blueprint: &[I2cExpectation]) {
    for bus in buses {
        // Skip empty buses for cleaner output
        if bus.devices.is_empty() { continue; }

        println!("\n{} (Bus {})", "I2C Bus".bold(), bus.id.yellow());
        for device in &bus.devices {
            audit_i2c_device(device, blueprint);
        }
    }
}

fn audit_i2c_device(dev: &TuxDevice, blueprint: &[I2cExpectation]) {
    if let DeviceAddress::I2c { bus, address } = &dev.address {
        // Match against blueprint
        let expectation = blueprint.iter().find(|e| e.bus == *bus && e.parsed_address() == Some(*address));
        
        let icon = if expectation.is_some() { "★".yellow() } else { "•".white() };
        let addr_str = format!("0x{:02x}", address);
        
        // Print base device line

        let driver = dev.status.driver_bound.as_deref().unwrap_or("none");
        let hw_resp = if dev.status.hw_responding { "ACK".green() } else { "NACK".red() };

        // Verification Logic
        match expectation {
            Some(exp) => {
                println!("  {} {} [{}]", icon, dev.name.cyan(), addr_str.green().dimmed());
                if let Some(req_driver) = &exp.required_driver {
                    if req_driver == driver {
                        println!("    ┗━ Driver {} - expected (HW: {})", 
                            driver.green().bold(), hw_resp);
                    } else {
                        println!("    ┗━ Driver {}- expected {} (HW: {})", 
                            driver.red().bold(), req_driver.cyan(), hw_resp);
                    }
                } else {
                    println!("    ┗━ Driver: {} (HW: {})", driver.blue().bold(), hw_resp);
                }
            },
            None => {
                println!("  {} {} [{}]", icon, dev.name.cyan(), addr_str.red().dimmed());
                println!("    ┗━ Driver: {} (HW: {})", driver.blue().bold(), hw_resp);
            }
        }
    }
}
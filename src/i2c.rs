use anyhow::Result;
use i2cdev::core::*;
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};
use nix::errno::Errno;
use std::fs;
use std::path::{Path, PathBuf};

/// Finds all available i2c character devices in /dev
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
    // Sort them so they appear as i2c-0, i2c-1...
    buses.sort();
    Ok(buses)
}

pub trait I2cScanner {
    fn scan_hw_probe(&self) -> Result<(Vec<u16>, Vec<u16>)>; // TODO: add address range as parameter
    fn scan_sysfs(&self) -> Result<Vec<u16>>; // TODO: add address range as parameter
}

pub struct LinuxI2cScanner {
    pub bus_id: u8,
}

impl I2cScanner for LinuxI2cScanner {
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

pub struct I2cValidationResult {
    pub missing: Vec<u16>,
    pub unexpected: Vec<u16>,
    pub present: Vec<u16>,
    pub probed: Vec<u16>,
}

pub fn validate_bus(
    scanner: &impl I2cScanner,
    expected_addresses: &[u16],
) -> Result<I2cValidationResult> {
    let (hw_unbound, hw_bound) = scanner.scan_hw_probe()?;
    let detected_sysfs = scanner.scan_sysfs()?;

    let mut result = I2cValidationResult {
        missing: Vec::new(),
        unexpected: Vec::new(),
        present: Vec::new(),
        probed: Vec::new(),
    };

    for &addr in expected_addresses {
        if hw_unbound.contains(&addr) {
            result.present.push(addr);
            result.probed.push(addr);
        } else if hw_bound.contains(&addr) {
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

pub struct I2cBusReport {
    pub bus_path: String,
    pub kernel_detected: Vec<u16>,  // From /sys
    pub hardware_unbound: Vec<u16>, // From smbus_write_quick - unbound
    pub hardware_bound: Vec<u16>,   // From smbus_write_quick - bound to a driver
}

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
                    .last() // Get 'rk808' from 'rockchip,rk808'
                    .unwrap_or("Unknown")
                    .to_string();
            }
        }
    }

    "Unidentified".to_string()
}

pub fn full_system_scan() -> Result<Vec<I2cBusReport>> {
    let busses = discover_buses()?;
    let mut reports = Vec::new();

    for path in busses {
        let bus_str = path.to_string_lossy().to_string();
        let bus_id: u8 = bus_str
            .strip_prefix("/dev/i2c-")
            .and_then(|x| x.parse::<u8>().ok())
            .expect("invalid bus string");
        let scanner = LinuxI2cScanner { bus_id: bus_id };

        // 1. Live Hardware Probe
        let (hw_unbound, hw_bound) = scanner.scan_hw_probe()?;

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

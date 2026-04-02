//! Configuration models for `tux-validation`.
//!
//! This module defines the expected hardware and software state of the system.
//! These structs are deserialized directly from the user-provided TOML blueprint.

use serde::Deserialize;

/// The root configuration blueprint containing all system expectations.
#[derive(Deserialize, Debug, Default)]
pub struct Config {
    /// Expected USB devices and their topology.
    #[serde(default)]
    pub usb_devices: Vec<UsbExpectation>,
    /// Expected I2C chips and bus assignments.
    #[serde(default)]
    pub i2c_devices: Vec<I2cExpectation>,
    /// Expected network interfaces (Ethernet and Wi-Fi) and their link states.
    #[serde(default)]
    pub network_devices: Vec<NetworkExpectation>,
    /// Expected PCIe devices, including bandwidth and speed constraints.
    #[serde(default)]
    pub pci_devices: Vec<PciExpectation>,
    /// Expected Systemd user-space daemons and their lifecycle states.
    #[serde(default)]
    pub systemd_services: Vec<SystemdExpectation>,
}

//TODO: make all test fields optional?
/// Defines the expected constraints for a specific USB device.
#[derive(Deserialize, Debug)]
pub struct UsbExpectation {
    /// A human-readable identifier for reporting (e.g., "External Camera").
    pub name: String,
    /// USB Vendor ID as a 4-character hex string (e.g., "046d").
    pub vid: String,
    /// USB Product ID as a 4-character hex string (e.g., "082d").
    pub pid: String,
    /// The physical port topology path (e.g., "3-1.4" for bus 3, port 1, sub-port 4).
    pub expected_port: String,
    /// The exact kernel driver that must be bound to this device (e.g., "uvcvideo").
    pub required_driver: String,
    /// Minimum negotiated USB speed in Mbps (e.g., "480" for High Speed, "5000" for SuperSpeed).
    pub min_speed: Option<String>,
}

/// Defines the expected constraints for an I2C device.
#[derive(Deserialize, Debug)]
pub struct I2cExpectation {
    /// A human-readable identifier for reporting (e.g., "rk808 PMIC").
    pub name: String,
    /// The integer ID of the I2C bus (e.g., `0` for `/dev/i2c-0`).
    pub bus: u8,
    /// The I2C chip address represented as a hex string (e.g., "0x1b").
    pub address: String,
    /// The kernel driver that must be bound to this chip, if applicable.
    pub required_driver: Option<String>,
}

impl I2cExpectation {
    /// Helper to safely parse the hex string from the TOML blueprint into a numeric `u16`.
    /// Strips the "0x" prefix if present.
    pub fn parsed_address(&self) -> Option<u16> {
        let clean = self.address.trim_start_matches("0x");
        u16::from_str_radix(clean, 16).ok()
    }
}

/// Defines the expected constraints for a Network interface (Ethernet or Wi-Fi).
#[derive(Deserialize, Debug)]
pub struct NetworkExpectation {
    /// The exact OS-level interface name (e.g., "eth0" or "wlan0").
    pub interface_name: String,
    /// Whether the physical/wireless link carrier must be detected (`true` = UP).
    pub link_status: bool,
    /// Minimum negotiated link speed in Mbps (primarily for Ethernet, e.g., `1000`).
    pub speed: Option<u32>,
    /// The expected kernel network driver (e.g., "igb" or "iwlwifi").
    pub driver: Option<String>,
    /// The expected hardware MAC address (e.g., "aa:bb:cc:dd:ee:ff").
    pub mac_address: Option<String>,
    /// An IPv4 address that must be assigned to this interface.
    pub expected_ip: Option<String>,
    /// The name of the wireless network this interface must be associated with (Wi-Fi only).
    pub expected_ssid: Option<String>,
}

/// Defines the expected constraints for a PCI/PCIe device.
#[derive(Deserialize, Debug)]
pub struct PciExpectation {
    /// The exact Bus-Device-Function (BDF) address (e.g., "0000:01:00.0").
    pub address: String,
    /// The expected human-readable device name or vendor substring (e.g., "NVIDIA").
    pub device: Option<String>,
    /// The kernel driver that must be bound to this PCIe endpoint (e.g., "nvme").
    pub driver: Option<String>,
    /// The minimum physical PCIe lane width currently negotiated (e.g., `4` for x4).
    pub min_link_width: Option<u8>,
    /// The minimum PCIe link speed currently negotiated in GT/s (e.g., `8.0` for Gen3).
    pub min_link_speed: Option<f32>,
}

/// Defines the expected state of a Systemd unit/service.
#[derive(Deserialize, Debug)]
pub struct SystemdExpectation {
    /// The exact name of the unit as recognized by systemd (e.g., "sshd.service").
    pub name: String,
    /// A human-readable description (used purely for metadata/reporting, not strictly asserted).
    pub description: Option<String>,
    /// The expected unit load state (e.g., "loaded", "not-found", "bad-setting").
    pub load_state: Option<String>,
    /// The expected general unit state (e.g., "active", "inactive", "failed").
    pub active_state: Option<String>,
    /// The expected unit-type-specific detailed state (e.g., "running", "exited", "dead").
    pub sub_state: Option<String>,
}

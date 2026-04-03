//! Debugging and ad-hoc discovery utilities.
//!
//! This module provides raw data extraction tools meant primarily for developers
//! and engineers who need to inspect the underlying OS-level `udev` data before
//! writing or debugging their configuration blueprints.

use anyhow::Result;
use udev::Enumerator;

/// Dumps the raw `udev` properties and attributes for all devices within a given subsystem.
///
/// This is a diagnostic utility that prints the unparsed, raw data directly from the Linux
/// `sysfs` tree to the terminal. It is highly useful for discovering the exact string values
/// required for TOML blueprints (e.g., finding the exact `ID_PCI_CLASS_FROM_DATABASE`
/// property of an unrecognized PCIe card).
///
/// # Arguments
/// * `subsystem` - The exact kernel subsystem string to filter by (e.g., "usb", "i2c", "net", "pci").
pub fn subsystem_info_udev(subsystem: String) -> Result<()> {
    let mut enumerator = Enumerator::new()?;

    // Filter for subsystem
    enumerator.match_subsystem(subsystem)?;

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

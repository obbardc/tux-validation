use crate::device::{BusStatus, DeviceDetails, PcieProperties, Subsystem, TuxBus, TuxDevice};
use anyhow::Result;
use std::collections::HashMap;
use udev::Enumerator;

pub fn audit_pci_subsystem() -> Result<Vec<TuxBus>> {
    let mut enumerator = Enumerator::new()?;

    // Not every devboard will have pci subsystem entries without plugged devices
    if enumerator.match_subsystem("pci").is_err() {
        return Ok(Vec::new());
    }

    let mut pci_devices = Vec::new();

    for dev in enumerator.scan_devices()? {
        if let Some(mut tux_dev) = TuxDevice::from_udev(&dev) {
            // Helper to parse speeds, treating "Unknown" or missing as None
            let parse_speed = |val: Option<&std::ffi::OsStr>| -> Option<String> {
                let s = val?.to_str()?.to_string();
                if s == "Unknown" { None } else { Some(s) }
            };

            // Helper to parse widths, treating "0", "255" or missing as None
            let parse_width = |val: Option<&std::ffi::OsStr>| -> Option<u8> {
                let w = val?.to_str()?.parse::<u8>().ok()?;
                if w == 0 || w == 255 { None } else { Some(w) }
            };

            let details = PcieProperties {
                vendor_id: dev
                    .attribute_value("vendor")
                    .and_then(|v| v.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                device_id: dev
                    .attribute_value("device")
                    .and_then(|v| v.to_str())
                    .unwrap_or("unknown")
                    .to_string(),

                vendor_name: dev
                    .property_value("ID_VENDOR_FROM_DATABASE")
                    .and_then(|v| v.to_str())
                    .unwrap_or("Unknown Vendor")
                    .to_string(),

                device_name: dev
                    .property_value("ID_MODEL_FROM_DATABASE")
                    .and_then(|v| v.to_str())
                    .unwrap_or("Unknown Device")
                    .to_string(),

                class_name: dev
                    .property_value("ID_PCI_CLASS_FROM_DATABASE")
                    .and_then(|v| v.to_str())
                    .unwrap_or("Unknown Class")
                    .to_string(),

                revision: dev
                    .attribute_value("revision")
                    .and_then(|v| v.to_str())
                    .unwrap_or("unknown")
                    .to_string(),

                // Link stats
                max_link_speed: parse_speed(dev.attribute_value("max_link_speed")),
                cur_link_speed: parse_speed(dev.attribute_value("current_link_speed")),
                max_link_width: parse_width(dev.attribute_value("max_link_width")),
                cur_link_width: parse_width(dev.attribute_value("current_link_width")),
            };

            // Ensure the hardware responding status is tied to the driver being loaded
            // (If a PCI device has no driver, it might just be a dummy stub or uninitialized)
            tux_dev.status.hw_responding = Some(tux_dev.status.driver_bound.is_some());
            tux_dev.details = DeviceDetails::Pci(details);

            pci_devices.push(tux_dev);
        }
    }

    if pci_devices.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![TuxBus {
            name: "PCI Express Bus".into(),
            subsystem: Subsystem::Pci,
            id: "0".into(),
            devices: pci_devices,
            status: BusStatus::Active,
            metadata: HashMap::new(),
        }])
    }
}

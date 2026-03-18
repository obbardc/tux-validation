use crate::config::{I2cExpectation, NetworkExpectation, PciExpectation, UsbExpectation};
use crate::device::{DeviceAddress, DeviceDetails, Subsystem, TuxBus, TuxDevice};
use std::time::{Duration as CoreDuration, Instant};

/// A generic way to identify what hardware a particular test was looking for
#[derive(Debug, Clone, PartialEq)]
pub enum TargetId {
    Usb { vid: String, pid: String },
    I2c { bus: u8, address: u16 },
    Network { interface: String },
    Pci { address: String },
}

/// The outcome of a single expectation check
#[derive(Debug, Clone)]
pub enum AuditStatus {
    Pass,
    Fail {
        reason: String,
        actual_value: String,
    },
    Missing {
        reason: String,
    }, // Hardware wasn't found at all
}

/// Information about each tested field
#[derive(Debug, Clone)]
pub struct FieldCheck {
    pub name: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
}

/// The complete record of a test case
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub subsystem: String, // e.g., "USB" or "I2C"
    pub item_name: String, // e.g., "Mule CAN Adapter"
    pub target_id: TargetId,
    pub location: String, // e.g., "Bus 3, Port 3-1.4"
    pub status: AuditStatus,
    pub checks: Vec<FieldCheck>,
    pub duration: CoreDuration,
}

/// Evaluates provided USB configuration against detected hardware
pub fn evaluate_usb_blueprint(
    buses: &[TuxBus],
    blueprint: &[UsbExpectation],
) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    for exp in blueprint {
        let start_time = Instant::now();
        // Search the TuxBus tree for a particular device
        let found_device = find_usb_device(buses, &exp.vid, &exp.pid);

        let (status, checks) = match found_device {
            Some(dev) => verify_usb_constraints(dev, exp),
            None => (
                AuditStatus::Missing {
                    reason: format!("Device [{}:{}] not found on system", exp.vid, exp.pid),
                },
                Vec::new(),
            ),
        };

        let elapsed_time = start_time.elapsed();

        results.push(ValidationResult {
            subsystem: "USB".to_string(),
            item_name: exp.name.clone(),
            target_id: TargetId::Usb {
                vid: exp.vid.clone(),
                pid: exp.pid.clone(),
            },
            location: exp.expected_port.clone(),
            status,
            checks,
            duration: elapsed_time,
        });
    }

    results
}

/// Searches all buses for a specific USB device
pub fn find_usb_device<'a>(buses: &'a [TuxBus], vid: &str, pid: &str) -> Option<&'a TuxDevice> {
    for bus in buses {
        for dev in &bus.devices {
            if let Some(found) = search_tree(dev, vid, pid) {
                return Some(found);
            }
        }
    }
    None
}

/// Recursive device searcher
fn search_tree<'a>(dev: &'a TuxDevice, vid: &str, pid: &str) -> Option<&'a TuxDevice> {
    if let DeviceAddress::Usb {
        vid: dev_vid,
        pid: dev_pid,
        ..
    } = &dev.address
        && dev_vid == vid
        && dev_pid == pid
    {
        return Some(dev);
    }

    // Check children
    for child in &dev.children {
        if let Some(found) = search_tree(child, vid, pid) {
            return Some(found);
        }
    }

    None
}

/// Verifies whether (and to what extent) detected device corresponds to requested configuration
/// Returns AuditStatus and vector of FieldCheck objects.
pub fn verify_usb_constraints(
    dev: &TuxDevice,
    exp: &UsbExpectation,
) -> (AuditStatus, Vec<FieldCheck>) {
    let mut checks = Vec::new();

    let dev_port = match &dev.address {
        DeviceAddress::Usb { port_path, .. } => port_path,
        _ => {
            return (
                AuditStatus::Fail {
                    reason: "Device is not a USB device".into(),
                    actual_value: "".into(),
                },
                checks,
            );
        }
    };

    let props = match &dev.details {
        DeviceDetails::Usb(p) => p,
        _ => {
            return (
                AuditStatus::Fail {
                    reason: "Missing USB properties".into(),
                    actual_value: "".into(),
                },
                checks,
            );
        }
    };

    // Check physical port
    let port_match = *dev_port == exp.expected_port;
    checks.push(FieldCheck {
        name: "Port".to_string(),
        passed: port_match,
        expected: exp.expected_port.clone(),
        actual: dev_port.to_string(),
    });

    // Check speed
    if let Some(expected_speed) = &exp.min_speed {
        let speed_match = verify_speed(&props.speed, expected_speed);
        checks.push(FieldCheck {
            name: "Speed".to_string(),
            passed: speed_match,
            expected: expected_speed.clone(),
            actual: props.speed.clone(),
        });
    }

    // Check driver binding
    // Check if at least one interface is bound to the required driver.
    if let Some(relevant_iface) = props
        .interfaces
        .iter()
        .find(|iface| iface.driver.as_deref().unwrap_or("None") == exp.required_driver)
    {
        checks.push(FieldCheck {
            name: "Driver".to_string(),
            passed: true,
            expected: exp.required_driver.clone(),
            actual: relevant_iface
                .driver
                .as_deref()
                .unwrap_or("None")
                .to_string(),
        });
    } else {
        // If none matching the requirement, collect the names of all drivers currently bound to this device's interfaces
        let bound_drivers: Vec<String> = props
            .interfaces
            .iter()
            .map(|iface| iface.driver.as_deref().unwrap_or("None").to_string())
            .collect();

        // Join them with a comma, or provide a fallback if there are 0 interfaces
        let actual_str = if bound_drivers.is_empty() {
            "No interfaces found".to_string()
        } else {
            bound_drivers.join(", ")
        };
        checks.push(FieldCheck {
            name: "Driver".to_string(),
            passed: false,
            expected: exp.required_driver.clone(),
            actual: actual_str,
        });
    }

    // If ANY check failed, the whole test fails.
    let all_passed = checks.iter().all(|c| c.passed);

    let status = if all_passed {
        AuditStatus::Pass
    } else {
        // Build a summary of the failed checks for the XML
        let failed_msgs: Vec<String> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| {
                format!(
                    "{} mismatch (Expected: {}, Got: {})",
                    c.name, c.expected, c.actual
                )
            })
            .collect();

        AuditStatus::Fail {
            reason: failed_msgs.join(" | "),
            actual_value: "See reason".to_string(),
        }
    };

    (status, checks)
}

/// Checks if expected USB speed is equal or larger than the actual one.
pub fn verify_speed(actual: &str, expected_min: &str) -> bool {
    let speed_to_val = |s: &str| -> u32 {
        // Strip everything that isn't a digit (like "M" or "Mbps")
        let cleaned = s.trim_end_matches(|c: char| !c.is_numeric());

        // Parse directly to u32. If it fails, return 0.
        cleaned.parse::<u32>().unwrap_or(0)
    };

    speed_to_val(actual) >= speed_to_val(expected_min)
}

/// Evaluates provided I2C configuration against detected hardware
pub fn evaluate_i2c_blueprint(
    buses: &[TuxBus],
    blueprint: &[I2cExpectation],
) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    for exp in blueprint {
        let start_time = Instant::now();
        let expected_addr = exp.parsed_address();
        // Search the TuxBus tree for a particular device
        let found_device =
            if let Some(found_bus) = buses.iter().find(|bus| bus.id == exp.bus.to_string()) {
                found_bus.devices.iter().find(|dev| {
                    if let DeviceAddress::I2c { address, .. } = &dev.address {
                        Some(*address) == expected_addr
                    } else {
                        false
                    }
                })
            } else {
                None
            };

        let (status, checks) = match found_device {
            Some(dev) => verify_i2c_constraints(dev, exp),
            None => (
                AuditStatus::Missing {
                    reason: format!("Device [{}:{}] not found on system", exp.bus, exp.address),
                },
                Vec::new(),
            ),
        };

        let elapsed_time = start_time.elapsed();

        results.push(ValidationResult {
            subsystem: "I2C".to_string(),
            item_name: exp.name.clone(),
            target_id: TargetId::I2c {
                bus: exp.bus,
                address: expected_addr.unwrap_or(0),
            },
            location: format!("{}-{}", exp.bus, exp.address),
            status,
            checks,
            duration: elapsed_time,
        });
    }

    results
}

/// Verifies whether (and to what extent) detected I2C device corresponds to requested configuration
/// Returns AuditStatus and vector of FieldCheck objects.
/// TODO: There is some boilerplate here and in verify_usb_constraints - need to refactor
pub fn verify_i2c_constraints(
    dev: &TuxDevice,
    exp: &I2cExpectation,
) -> (AuditStatus, Vec<FieldCheck>) {
    let mut checks = Vec::new();

    if let Some(hw_ack) = dev.status.hw_responding {
        checks.push(FieldCheck {
            name: "Hardware Probe".to_string(),
            passed: hw_ack, // Fails if NACK
            expected: "true".to_string(),
            actual: hw_ack.to_string(),
        });
    }

    let driver = dev.status.driver_bound.as_deref().unwrap_or("None");
    if let Some(expected_driver) = &exp.required_driver {
        checks.push(FieldCheck {
            name: "Driver".to_string(),
            passed: driver == expected_driver,
            expected: expected_driver.clone(),
            actual: driver.to_string(),
        });
    }

    // If ANY check failed, the whole test fails.
    let all_passed = checks.iter().all(|c| c.passed);

    let status = if all_passed {
        AuditStatus::Pass
    } else {
        // Build a summary of the failed checks for the XML
        let failed_msgs: Vec<String> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| {
                format!(
                    "{} mismatch (Expected: {}, Got: {})",
                    c.name, c.expected, c.actual
                )
            })
            .collect();

        AuditStatus::Fail {
            reason: failed_msgs.join(" | "),
            actual_value: "See reason".to_string(),
        }
    };

    (status, checks)
}

pub fn evaluate_network_blueprint(
    buses: &[TuxBus],
    blueprint: &[NetworkExpectation],
) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    for exp in blueprint {
        let start_time = std::time::Instant::now();

        // Find the device in the Net subsystem
        let found_device = buses
            .iter()
            .filter(|bus| bus.subsystem == Subsystem::Net)
            .flat_map(|bus| &bus.devices)
            .find(|dev| dev.name == exp.interface_name);

        let (status, checks) = match found_device {
            Some(dev) => verify_network_constraints(dev, exp),
            None => (
                AuditStatus::Missing {
                    reason: format!("Interface {} not found", exp.interface_name),
                },
                Vec::new(),
            ),
        };

        results.push(ValidationResult {
            subsystem: "Network".to_string(),
            item_name: exp.interface_name.clone(),
            target_id: TargetId::Network {
                interface: exp.interface_name.clone(),
            },
            location: exp.interface_name.clone(),
            status,
            checks,
            duration: start_time.elapsed(),
        });
    }
    results
}

fn verify_network_constraints(
    dev: &TuxDevice,
    exp: &NetworkExpectation,
) -> (AuditStatus, Vec<FieldCheck>) {
    let mut checks = Vec::new();

    let (ipv4_list, link_detected, current_speed) = match &dev.details {
        DeviceDetails::Ethernet(p) => (&p.ipv4_address, p.link_detected, Some(p.speed)),
        DeviceDetails::Wifi(p) => (&p.ipv4_address, p.link_detected, None), // Wifi speed is variable
        _ => {
            return (
                AuditStatus::Fail {
                    reason: "Not a network device".into(),
                    actual_value: "".into(),
                },
                checks,
            );
        }
    };

    //MAC address
    if let Some(expected_mac) = &exp.mac_address
        && let DeviceAddress::Network { mac, .. } = &dev.address
    {
        checks.push(FieldCheck {
            name: "MAC Address".into(),
            passed: mac.to_lowercase() == expected_mac.to_lowercase(),
            expected: expected_mac.clone(),
            actual: mac.clone(),
        });
    }

    // Physical Link
    checks.push(FieldCheck {
        name: "Link Status".into(),
        passed: link_detected == exp.link_status,
        expected: exp.link_status.to_string(),
        actual: link_detected.to_string(),
    });

    // Speed
    if let Some(expected_speed) = exp.speed
        && let Some(actual_speed) = current_speed
    {
        checks.push(FieldCheck {
            name: "Speed".into(),
            passed: actual_speed >= expected_speed,
            expected: format!("{}+ Mbps", expected_speed),
            actual: format!("{} Mbps", actual_speed),
        });
    }

    // IPv4 Presence/DHCP
    let has_any_v4 = !ipv4_list.is_empty();
    let (ip_passed, expected_str, actual_str) = if let Some(target_ip) = &exp.expected_ip {
        // We want a specific IP
        let found = ipv4_list.iter().any(|addr| addr == target_ip);
        (
            found,
            target_ip.clone(),
            if has_any_v4 {
                ipv4_list.join(", ")
            } else {
                "None".into()
            },
        )
    } else {
        (
            has_any_v4,
            "Any valid IPv4".into(),
            if has_any_v4 {
                ipv4_list.join(", ")
            } else {
                "None".into()
            },
        )
    };
    checks.push(FieldCheck {
        name: "IPv4 Check".into(),
        passed: ip_passed,
        expected: expected_str,
        actual: actual_str,
    });

    // Driver Check
    if let Some(expected_driver) = &exp.driver {
        let actual_driver = dev.status.driver_bound.as_deref().unwrap_or("None");
        checks.push(FieldCheck {
            name: "Driver".into(),
            passed: actual_driver == expected_driver,
            expected: expected_driver.clone(),
            actual: actual_driver.into(),
        });
    }

    if let DeviceDetails::Wifi(wifi_props) = &dev.details
        && let Some(expected_ssid) = &exp.expected_ssid
    {
        checks.push(FieldCheck {
            name: "SSID".into(),
            passed: wifi_props.ssid.as_ref() == Some(expected_ssid),
            expected: expected_ssid.clone(),
            actual: wifi_props.ssid.clone().unwrap_or("None".into()),
        });
    }

    let all_passed = checks.iter().all(|c| c.passed);
    let status = if all_passed {
        AuditStatus::Pass
    } else {
        let failed_msgs: Vec<String> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| {
                format!(
                    "{} mismatch (Expected: {}, Got: {})",
                    c.name, c.expected, c.actual
                )
            })
            .collect();
        AuditStatus::Fail {
            reason: failed_msgs.join(" | "),
            actual_value: "See reason".to_string(),
        }
    };

    (status, checks)
}

// Evaluates found PCI devices against expected blueprint.
// Structured slightly different from previous validators for a change.
// TODO: should probably make evaluation functions more uniform and look into reducing boilerplate
pub fn evaluate_pci_blueprint(
    buses: &[TuxBus],
    blueprint: &[PciExpectation],
) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    // Flatten all PCI devices from all buses for easy searching
    let all_pci_devices: Vec<_> = buses
        .iter()
        .filter(|b| b.subsystem == Subsystem::Pci)
        .flat_map(|b| &b.devices)
        .collect();

    for expected in blueprint {
        let start_time = Instant::now();
        let target_id = TargetId::Pci {
            address: expected.address.clone(),
        };
        let item_name = expected
            .device
            .clone()
            .unwrap_or_else(|| "Unknown PCIe Device".into());

        // Check Presence by parsing the BDF address back to a string
        let found_device = all_pci_devices.iter().find(|dev| {
            if let DeviceAddress::Pci {
                domain,
                bus,
                device,
                function,
            } = &dev.address
            {
                let dev_bdf = format!("{:04x}:{:02x}:{:02x}.{:x}", domain, bus, device, function);
                dev_bdf == expected.address
            } else {
                false
            }
        });

        match found_device {
            None => {
                // MISSING CASE
                results.push(ValidationResult {
                    subsystem: "PCIe".into(),
                    item_name,
                    target_id,
                    location: expected.address.clone(),
                    status: AuditStatus::Missing {
                        reason: format!(
                            "No PCI device detected at BDF address {}",
                            expected.address
                        ),
                    },
                    checks: Vec::new(),
                    duration: start_time.elapsed(),
                });
            }
            Some(dev) => {
                // FOUND CASE: Evaluate fields
                let mut checks = Vec::new();

                // Check Device Name (Hardware Model)
                if let Some(expected_name) = &expected.device {
                    let passed = dev.name.contains(expected_name);
                    checks.push(FieldCheck {
                        name: "Hardware Model".into(),
                        passed,
                        expected: expected_name.clone(),
                        actual: dev.name.clone(),
                    });
                }

                // Check Driver Bound
                if let Some(expected_driver) = &expected.driver {
                    let actual_driver = dev.status.driver_bound.as_deref().unwrap_or("None");
                    checks.push(FieldCheck {
                        name: "Driver".into(),
                        passed: actual_driver == expected_driver,
                        expected: expected_driver.clone(),
                        actual: actual_driver.into(),
                    });
                }

                // Check Link Width and Speed
                if let DeviceDetails::Pci(props) = &dev.details {
                    // Width Check
                    if let Some(min_width) = expected.min_link_width {
                        let actual_width_str = props
                            .cur_link_width
                            .map_or_else(|| "None".to_string(), |w| format!("x{}", w));
                        let passed = props.cur_link_width.is_some_and(|w| w >= min_width);

                        checks.push(FieldCheck {
                            name: "PCIe Link Width".into(),
                            passed,
                            expected: format!(">= x{}", min_width),
                            actual: actual_width_str,
                        });
                    }

                    // Speed Check
                    if let Some(min_speed) = expected.min_link_speed {
                        let actual_speed_str = props
                            .cur_link_speed
                            .clone()
                            .unwrap_or_else(|| "None".into());

                        // Extract just the float part from the string (e.g., "16.0" from "16.0 GT/s PCIe")
                        let cur_speed_val: f32 = props
                            .cur_link_speed
                            .as_ref()
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0.0);

                        let passed = cur_speed_val >= min_speed;
                        checks.push(FieldCheck {
                            name: "PCIe Link Speed".into(),
                            passed,
                            expected: format!(">= {} GT/s", min_speed),
                            actual: actual_speed_str,
                        });
                    }
                }

                let all_passed = checks.iter().all(|c| c.passed);
                let status = if all_passed {
                    AuditStatus::Pass
                } else {
                    let failed_msgs: Vec<String> = checks
                        .iter()
                        .filter(|c| !c.passed)
                        .map(|c| {
                            format!(
                                "{} mismatch (Expected: {}, Got: {})",
                                c.name, c.expected, c.actual
                            )
                        })
                        .collect();
                    AuditStatus::Fail {
                        reason: failed_msgs.join(" | "),
                        actual_value: "See reason".to_string(),
                    }
                };
                results.push(ValidationResult {
                    subsystem: "PCIe".into(),
                    item_name: dev.name.clone(), // Use the actual detected name for the report
                    target_id,
                    location: expected.address.clone(), // BDF acts as the physical location
                    status,
                    checks,
                    duration: start_time.elapsed(),
                });
            }
        }
    }

    results
}

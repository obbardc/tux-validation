use crate::device::{TuxBus, TuxDevice, DeviceAddress, DeviceDetails};
use crate::config::UsbExpectation;
use crate::usb::verify_speed;
use junit_report::{TestCase, TestSuite, Report, Duration as JunitDuration};
use std::time::{Duration as CoreDuration, Instant};

/// A generic way to identify what hardware a particular test was looking for
#[derive(Debug, Clone, PartialEq)]
pub enum TargetId {
    Usb { vid: String, pid: String },
    I2c { bus: u8, address: u16 },
}

/// The outcome of a single expectation check
#[derive(Debug, Clone)]
pub enum AuditStatus {
    Pass,
    Fail { reason: String, actual_value: String },
    Missing { reason: String }, // Hardware wasn't found at all
}

/// The complete record of a test case
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub subsystem: String,      // e.g., "USB" or "I2C"
    pub item_name: String,      // e.g., "Mule CAN Adapter"
    pub target_id: TargetId,
    pub location: String,       // e.g., "Bus 3, Port 3-1.4"
    pub status: AuditStatus,
    pub duration: CoreDuration,
}

/// Evaluates provided USB configuration against detected hardware
pub fn evaluate_usb_blueprint(
    buses: &[TuxBus],
    blueprint: &[UsbExpectation]
) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    for exp in blueprint {
        let start_time = Instant::now();
        // Search the TuxBus tree for a particular device
        let found_device = find_usb_device(buses, &exp.vid, &exp.pid);

        let status = match found_device {
            Some(dev) => {
                verify_usb_constraints(dev, exp) 
            },
            None => {
                AuditStatus::Missing { 
                    reason: format!("Device [{}:{}] not found on system", exp.vid, exp.pid) 
                }
            }
        };

        let elapsed_time = start_time.elapsed();

        results.push(ValidationResult {
            subsystem: "USB".to_string(),
            item_name: exp.name.clone(),
            target_id: TargetId::Usb { 
                vid: exp.vid.clone(), 
                pid: exp.pid.clone() 
            },
            location: exp.expected_port.clone(),
            status,
            duration: elapsed_time
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
    if let DeviceAddress::Usb { vid: dev_vid, pid: dev_pid, .. } = &dev.address {
        if dev_vid == vid && dev_pid == pid {
            return Some(dev);
        }
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
pub fn verify_usb_constraints(dev: &TuxDevice, exp: &UsbExpectation) -> AuditStatus {
    let mut errors = Vec::new();

    let dev_port = match &dev.address {
        DeviceAddress::Usb { port_path, .. } => port_path,
        _ => return AuditStatus::Fail { 
            reason: "Device is not a USB device".into(), actual_value: "".into() 
        },
    };

    let props = match &dev.details {
        DeviceDetails::Usb(p) => p,
        _ => return AuditStatus::Fail { 
            reason: "Missing USB properties".into(), actual_value: "".into() 
        },
    };

    // Check physical port
    if *dev_port != exp.expected_port {
        errors.push(format!("Port mismatch (Expected: {}, Got: {})", exp.expected_port, dev_port));
    }

    // Check speed
    if let Some(expected_speed) = &exp.min_speed {
        if !verify_speed(&props.speed, expected_speed) {
            errors.push(format!("Speed too low (Expected: {}M, Got: {}M)", expected_speed, props.speed));
        }
    }

    // Check driver binding
    // Check if at least one interface is bound to the required driver.
    let driver_found = props.interfaces.iter().any(|iface| {
        iface.driver.as_deref().unwrap_or("None") == exp.required_driver
    });

    if !driver_found {
        errors.push(format!("Driver '{}' not bound to any interface", exp.required_driver));
    }

    if errors.is_empty() {
        AuditStatus::Pass
    } else {
        AuditStatus::Fail {
            // Join all errors with a separator
            reason: errors.join(" | "), 
            actual_value: format!("Port: {}, Speed: {}", dev_port, props.speed),
        }
    }
}

/// Generates a junit report object and writes it in XML format.
pub fn generate_junit_xml(results: &[ValidationResult], filepath: &str) -> anyhow::Result<()> {
    let mut report = Report::new();
    let mut suite = TestSuite::new("Hardware Audit");

    for res in results {
        let test_name = format!("[{}] {}", res.subsystem, res.item_name);
        
        let test_time = JunitDuration::try_from(res.duration)
            .unwrap_or_else(|_| JunitDuration::milliseconds(1));

        let case = match &res.status {
            AuditStatus::Pass => TestCase::success(&test_name, test_time),
            AuditStatus::Fail { reason, .. } | AuditStatus::Missing { reason } => {
                TestCase::failure(&test_name, test_time, "HardwareError", reason)
            }
        };
        suite.add_testcase(case);
    }

    report.add_testsuite(suite);
    let mut file = std::fs::File::create(filepath)?;
    report.write_xml(&mut file)?;
    Ok(())
}
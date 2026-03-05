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
    pub subsystem: String,      // e.g., "USB" or "I2C"
    pub item_name: String,      // e.g., "Mule CAN Adapter"
    pub target_id: TargetId,
    pub location: String,       // e.g., "Bus 3, Port 3-1.4"
    pub status: AuditStatus,
    pub checks: Vec<FieldCheck>,
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

        let (status, checks) = match found_device {
            Some(dev) => {
                verify_usb_constraints(dev, exp) 
            },
            None => {
                (AuditStatus::Missing { 
                    reason: format!("Device [{}:{}] not found on system", exp.vid, exp.pid) 
                }, Vec::new())
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
            checks,
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
/// Returns AuditStatus and vector of FieldCheck objects.
pub fn verify_usb_constraints(dev: &TuxDevice, exp: &UsbExpectation) -> (AuditStatus, Vec<FieldCheck>) {
    let mut checks = Vec::new();

    let dev_port = match &dev.address {
        DeviceAddress::Usb { port_path, .. } => port_path,
        _ => return (AuditStatus::Fail { 
            reason: "Device is not a USB device".into(), actual_value: "".into() 
        }, checks),
    };

    let props = match &dev.details {
        DeviceDetails::Usb(p) => p,
        _ => return (AuditStatus::Fail { 
            reason: "Missing USB properties".into(), actual_value: "".into() 
        }, checks),
    };

    // Check physical port
    let port_match = *dev_port != exp.expected_port;
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
    if let Some(relevant_iface) = props.interfaces.iter().find(|iface| {
        iface.driver.as_deref().unwrap_or("None") == exp.required_driver
    }){
        checks.push(FieldCheck {
            name: "Driver".to_string(),
            passed: true,
            expected: exp.required_driver.clone(),
            actual: relevant_iface.driver.as_deref().unwrap_or("None").to_string(),
        });
    } else {
        // If none matching the requirement, collect the names of all drivers currently bound to this device's interfaces
        let bound_drivers: Vec<String> = props.interfaces.iter()
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
        let failed_msgs: Vec<String> = checks.iter()
            .filter(|c| !c.passed)
            .map(|c| format!("{} mismatch (Expected: {}, Got: {})", c.name, c.expected, c.actual))
            .collect();
            
        AuditStatus::Fail { 
            reason: failed_msgs.join(" | "), 
            actual_value: "See reason".to_string() 
        }
    };

    (status, checks)

}

/// Generates a junit report object and writes it in XML format.
pub fn generate_junit_xml(results: &[ValidationResult], scan_duration: Option<CoreDuration>, filepath: &str) -> anyhow::Result<()> {
    let mut report = Report::new();
    let mut suite = TestSuite::new("Hardware Audit");

    if let Some(duration) = scan_duration {
        let scan_time = JunitDuration::try_from(duration).unwrap_or_else(|_| JunitDuration::milliseconds(1));
        suite.add_testcase(TestCase::success("System: Hardware Discovery (Scan)", scan_time));
    }

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

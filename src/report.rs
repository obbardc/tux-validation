use crate::device::{TuxBus, TuxDevice, DeviceAddress, DeviceDetails, UsbInterface};
use crate::config::UsbExpectation;
use crate::usb::verify_speed;
use colored::Colorize;
use junit_report::{TestCase, TestSuite, Report, Duration as JunitDuration};
use std::time::{Duration as CoreDuration, Instant};
use roxmltree::Document;
use std::fs;

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
pub fn generate_junit_xml(results: &[ValidationResult], filepath: &str, scan_duration: Option<CoreDuration>) -> anyhow::Result<()> {
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

///Prints annotated USB tree with detected devices and interfaces
pub fn print_annotated_usb_tree(buses: &[TuxBus], results: &[ValidationResult], print_serial: bool) {
    println!("\n{}", "=== USB SUBSYSTEM ===".bold().cyan());
    for bus in buses {
        println!("\n{} (Bus {})", "Bus Controller".bold(), bus.id.yellow());
        for dev in &bus.devices {
            print_recursive_node(dev, results, 0, print_serial); 
        }
    }
}

fn print_recursive_node(dev: &TuxDevice, results: &[ValidationResult], depth: usize, print_serial: bool) {
    let indent = "  ".repeat(depth);
    // Check dev is a USB device and extract necessary parameters
    if let DeviceDetails::Usb(dev_props) = &dev.details
        && let DeviceAddress::Usb {
            vid,
            pid,
            port_path,
            ..
        } = &dev.address
    {
        let test_result = results.iter().find(|res| {
            match &res.target_id {
                TargetId::Usb{vid:res_vid, pid:res_pid} => {
                    res_vid == vid && res_pid == pid 
                },
                _ => false,
            }
        });
    
        let mut speed_colored = dev_props.speed.blue().bold();
        let mut port_colored = port_path.blue().dimmed();
        let mut newline_colored = "•".white();
        if let Some(res) = test_result {
            // Select PASS/FAIL/DEFAULT colors based on checks results
            let speed_check = res.checks.iter().find(|check| check.name == "Speed");
            speed_colored = match speed_check {
                Some(speed_checked) => if speed_checked.passed {
                    dev_props.speed.green().bold()
                } else {
                    dev_props.speed.red().bold()
                },
                None => dev_props.speed.blue().bold()
            };
            let port_check = res.checks.iter().find(|check| check.name == "Port");
            port_colored = match port_check {
                Some(port_checked) => if port_checked.passed {
                    port_path.green().dimmed()
                } else {
                    port_path.red().dimmed()
                },
                None => port_path.blue().dimmed()
            };
            newline_colored = "★".yellow();
        }
        println!(
            "{}{} {} [{}:{}] at {} ({}M)",
            indent,
            newline_colored,
            dev.name.cyan(),
            vid,
            pid,
            port_colored,
            speed_colored,
        );

        // Optionally print ID_SERIAL property
        if print_serial {
            println!(
                "{}    {} {}",
                indent,
                "ID:".dimmed(),
                dev_props.serial_id.dimmed()
            );
        }
        // Check and print interfaces data
        for iface in &dev_props.interfaces {
            print_usb_interface(iface, &indent, test_result);
        }
    }

    // Recurse into hub children
    for child in &dev.children {
        print_recursive_node(child, results, depth + 1, print_serial);
    }

}

fn print_usb_interface(iface: &UsbInterface, indent: &str, result: Option<&ValidationResult>) {
    let class_name = match iface.class.as_str() {
        "01" => "Audio",
        "09" => "Hub",
        "0e" => "Video",
        "03" => "HID",
        "ff" => "Vendor-Specific",
        _ => &iface.class,
    };
    let dev_driver = iface.driver.as_deref().unwrap_or("none");
    let mut driver_colored = dev_driver.blue().bold();
    if let Some(res) = result {
        let driver_check = res.checks.iter().find(|check| check.name == "Driver");
        driver_colored = match driver_check {
            Some(driver_checked) => if driver_checked.passed {
                dev_driver.green().bold()
            } else {
                dev_driver.red().bold()
            },
            None => dev_driver.blue().bold()
        };
    }

    println!(
        "{}  ┗━ If {:02} [{}]: Driver {}",
        indent,
        iface.if_num,
        class_name,
        driver_colored,
    );
}

/// Reads a JUnit XML file and prints a high-level summary to the terminal.
pub fn print_xml_summary(filepath: &str) -> anyhow::Result<()> {
    // Read the file into a string
    let xml_str = fs::read_to_string(filepath)
        .map_err(|e| anyhow::anyhow!("Failed to read XML file '{}': {}", filepath, e))?;

    // Parse the XML stringm into a DOM tree representation
    let doc = Document::parse(&xml_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse XML: {}", e))?;

    let mut total_tests = 0;
    let mut total_failures = 0;

    // Walk the tree looking for <testcase> nodes
    for node in doc.descendants() {
        if node.has_tag_name("testcase") {
            total_tests += 1;
            
            // Check if this testcase has a <failure> or <error> child
            let has_failure = node.children().any(|c| {
                c.has_tag_name("failure") || c.has_tag_name("error")
            });

            if has_failure {
                total_failures += 1;
            }
        }
    }

    let passed = total_tests - total_failures;

    // Print the condensed summary
    println!("\n{}", "=== XML REPORT SUMMARY ===".bold().cyan());
    println!("File:  {}", filepath.dimmed());
    println!("Total: {}", total_tests);
    
    if passed > 0 {
        println!("Pass:  {}", passed.to_string().green().bold());
    } else {
        println!("Pass:  {}", passed);
    }

    if total_failures > 0 {
        println!("Fail:  {}", total_failures.to_string().red().bold());
    } else {
        println!("Fail:  {}", total_failures);
    }

    // Add a final status banner
    if total_failures == 0 && total_tests > 0 {
        println!("\n{}", "ALL TESTS PASSED".green().bold());
    } else if total_failures > 0 {
        println!("\n{}", "SOME TESTS FAILED".red().bold());
    }

    Ok(())
}
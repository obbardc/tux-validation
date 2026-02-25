use clap::Parser;
use tux_validation::device::{TuxDevice, DeviceDetails, DeviceAddress, UsbInterface};
use tux_validation::usb::audit_usb_subsystem;
use colored::*;

#[derive(Parser)]
#[command(author, version, about = "Performs USB subsystem audit.")]
struct Args {
    /// Print serial ID
    #[arg(long)]
    serial: bool,

    /// Print debug info
    #[arg(long)]
    verbose: bool,
}
/// Configuration for a specific board
pub struct UsbExpectation {
    pub name: &'static str,
    pub vid: &'static str,
    pub pid: &'static str,
    pub expected_port: &'static str,
    pub required_driver: &'static str,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // Define expectations
    let blueprint = vec![
        UsbExpectation {
            name: "Mule CAN Adapter",
            vid: "2294",
            pid: "425a",
            expected_port: "3-1.4",
            required_driver: "ucan",
        },
        UsbExpectation {
            name: "Onboard Hub",
            vid: "05e3",
            pid: "0610",
            expected_port: "3-1",
            required_driver: "hub",
        },
    ];

    // Perform the system scan
    let buses = audit_usb_subsystem()?;

    // For now, verbose ouotput in the beginning
    if args.verbose {
        for bus in &buses {
            println!("--- BUS {} ---", bus.id);
            for dev in &bus.devices {
                dev.print_json()?;
            }
        }
        println!("");
    }

    println!("{}", "\n=== USB SUBSYSTEM ===".bold().cyan());

    for bus in &buses {
        println!("\n{} (Bus {})", "Bus Controller".bold(), bus.id.yellow());
        for device in &bus.devices {
            audit_recursive(device, 0, &blueprint, args.serial);
        }
    }

    Ok(())
}

fn audit_recursive(dev: &TuxDevice, depth: usize, blueprint: &[UsbExpectation], serial: bool) {
    let indent = "  ".repeat(depth);
    
    // Attempt to match device against blueprint
    if let DeviceDetails::Usb(props) = &dev.details && let DeviceAddress::Usb { vid, pid, port_path, .. } = &dev.address {
        let expectation = blueprint.iter().find(|e| e.vid == vid && e.pid == pid);
        
        // Print Device Header
        let icon = if expectation.is_some() { "★".yellow() } else { "•".white() };
        println!("{}{} {} [{}:{}] at {} ({}M)", 
            indent, icon, dev.name.green(), vid, pid, port_path.dimmed(), props.speed.blue().bold()
        );

        if serial {
            println!("{}    {} {}", indent, "ID:".dimmed(), props.serial_id.dimmed());
        }

        // Check Interfaces
        for iface in &props.interfaces {
            verify_interface(iface, &indent, expectation);
        }
    }

    // Recurse into hub children
    for child in &dev.children {
        audit_recursive(child, depth + 1, blueprint, serial);
    }
}

fn verify_interface(iface: &UsbInterface, indent: &str, expectation: Option<&UsbExpectation>) {
    let class_name = match iface.class.as_str() {
        "01" => "Audio",
        "09" => "Hub",
        "0e" => "Video",
        "03" => "HID",
        "ff" => "Vendor-Specific",
        _ => &iface.class,
    };
    let driver = iface.driver.as_deref().unwrap_or("none");
    
    match expectation {
        Some(exp) if exp.required_driver != driver => {
            println!("{}  ┗━ {} If {:02} [{}]: Driver is {}, but blueprint requires {}!", 
                indent, "FAIL".red().bold(), iface.if_num, class_name, driver.red(), exp.required_driver.cyan());
        }
        Some(_) => {
            println!("{}  ┗━ {} If {:02} [{}]: Driver {} verified", 
                indent, "PASS".green(), iface.if_num, class_name, driver.green());
        }
        None => {
            println!("{}  ┗━ If {:02} [{}]: Driver {}", indent, iface.if_num, class_name, driver);
        }
    }
}

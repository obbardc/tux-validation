use clap::Parser;
use tux_validation::usb::{UsbExpectation, audit_usb_subsystem, print_and_verify_usb};

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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // Define expectations
    let blueprint = vec![
        UsbExpectation {
            name: "Mule CAN Adapter".to_string(),
            vid: "2294".to_string(),
            pid: "425a".to_string(),
            expected_port: "3-1.4".to_string(),
            required_driver: "ucan".to_string(),
            min_speed: None,
        },
        UsbExpectation {
            name: "Onboard Hub".to_string(),
            vid: "05e3".to_string(),
            pid: "0610".to_string(),
            expected_port: "3-1".to_string(),
            required_driver: "hub".to_string(),
            min_speed: None,
        },
    ];

    // Perform the system scan
    let buses = audit_usb_subsystem()?;

    // For now, verbose ouotput in the beginning
    if args.verbose {
        println!("DEBUG INFORMATION");
        for bus in &buses {
            println!("--- BUS {} ---", bus.id);
            for dev in &bus.devices {
                dev.print_json()?;
            }
        }
        println!("");
    }

    print_and_verify_usb(&buses, &blueprint, args.serial);

    Ok(())
}

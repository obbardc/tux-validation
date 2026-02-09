use clap::Parser;
use tux_validation::i2c::full_system_scan;

#[derive(Parser)]
#[command(author, version, about = "Performs full I2C subsystem scan.")]
struct Args {
    /// Perform hardware probe (smbus_quick_write)
    #[arg(long)]
    hw_probe: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!(
        "{:<12} | {:<20} | {:<20}",
        "Bus", "Kernel Detected", "Responding Addresses"
    );
    println!("{:-<60}", "");

    let reports = full_system_scan(args.hw_probe)?;
    for report in reports {
        let sysfs_addrs: Vec<String> = report
            .kernel_detected
            .iter()
            .map(|a| format!("0x{:02x}", a))
            .collect();

        let mut hw_unbound: Vec<String> = report
            .hardware_unbound
            .iter()
            .map(|a| format!("U0x{:02x}", a))
            .collect();

        let mut hw_bound: Vec<String> = report
            .hardware_bound
            .iter()
            .map(|a| format!("B0x{:02x}", a))
            .collect();

        hw_unbound.append(&mut hw_bound);

        println!(
            "{:<12} | {:<20} | {:<20}",
            report.bus_path,
            sysfs_addrs.join(", "),
            hw_unbound.join(", ")
        );
    }
    Ok(())
}

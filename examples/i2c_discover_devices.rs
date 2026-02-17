use clap::Parser;
use tux_validation::i2c::{audit_all_i2c_buses};

#[derive(Parser)]
#[command(author, version, about = "Performs full I2C subsystem scan.")]
struct Args {
    /// Perform hardware probe (smbus_quick_write)
    #[arg(long)]
    hw_probe: bool,

    /// Print debug info
    #[arg(long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let i2c_busses = audit_all_i2c_buses(args.hw_probe)?;

    if args.verbose {
        println!("DEBUG INFORMATION");
        for bus in &i2c_busses {
            println!("--- BUS {} ---", bus.id);
            for dev in &bus.devices {
                dev.print_json()?;
            }
        }
        println!("");
    }

    println!("{:-<74}", "");
    println!("{:<6} | {:<7} | {:<15} | {:<15} | {:<17}", "Bus ID", "Address", "Name", "Driver", "SMBus Write Quick");
    println!("{:-<74}", "");

    for bus in i2c_busses {
        if let Some((first, rest)) = bus.devices.split_first(){
            println!("{:<6} | {:<7} | {:<15} | {:<15} | {:<17}", bus.id, format!("0x{:02x}", first.address.as_i2c_address().unwrap()), first.name, first.status.driver_bound.as_deref().unwrap_or("none"), first.status.hw_responding);
            for dev in rest {
                println!("{:<6} | {:<7} | {:<15} | {:<15} | {:<17}", "", format!("0x{:02x}", dev.address.as_i2c_address().unwrap()), dev.name, dev.status.driver_bound.as_deref().unwrap_or("none"), dev.status.hw_responding);
            }
            println!("{:-<74}", "");
        }
    }

    //let reports = full_system_scan(args.hw_probe)?;
    //for report in reports {
    //    let sysfs_addrs: Vec<String> = report
    //        .kernel_detected
    //        .iter()
    //        .map(|a| format!("0x{:02x}", a))
    //        .collect();

    //    let mut hw_unbound: Vec<String> = report
    //        .hardware_unbound
    //        .iter()
    //        .map(|a| format!("U0x{:02x}", a))
    //        .collect();

    //    let mut hw_bound: Vec<String> = report
    //        .hardware_bound
    //        .iter()
    //        .map(|a| format!("B0x{:02x}", a))
    //        .collect();

    //    hw_unbound.append(&mut hw_bound);

    //    println!(
    //        "{:<12} | {:<20} | {:<20}",
    //        report.bus_path,
    //        sysfs_addrs.join(", "),
    //        hw_unbound.join(", ")
    //    );
    //}
    Ok(())
}

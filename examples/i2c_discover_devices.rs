use clap::Parser;
use tux_validation::i2c::audit_all_i2c_buses;

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

    println!("{:-<80}", "");
    println!(
        "{:<6} | {:<7} | {:<15} | {:<20} | {:<17}",
        "Bus ID", "Address", "Name", "Driver", "SMBus Write Quick"
    );
    println!("{:-<80}", "");

    for bus in i2c_busses {
        if let Some((first, rest)) = bus.devices.split_first() {
            let hw_resp = match first.status.hw_responding {
                Some(true) => "ACK",
                Some(false) => "NACK",
                None => "NA",
            };
            println!(
                "{:<6} | {:<7} | {:<15} | {:<20} | {:<17}",
                bus.id,
                format!("0x{:02x}", first.address.as_i2c_address().unwrap()),
                first.name,
                first.status.driver_bound.as_deref().unwrap_or("None"),
                hw_resp
            );
            for dev in rest {
                let hw_resp = match first.status.hw_responding {
                    Some(true) => "ACK",
                    Some(false) => "NACK",
                    None => "NA",
                };
                println!(
                    "{:<6} | {:<7} | {:<15} | {:<20} | {:<17}",
                    "",
                    format!("0x{:02x}", dev.address.as_i2c_address().unwrap()),
                    dev.name,
                    dev.status.driver_bound.as_deref().unwrap_or("None"),
                    hw_resp
                );
            }
            println!("{:-<80}", "");
        }
    }

    Ok(())
}

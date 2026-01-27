use tux_validation::i2c;

fn main() -> anyhow::Result<()> {
    println!(
        "{:<12} | {:<20} | {:<20}",
        "Bus", "Kernel Detected", "Responding Addresses"
    );
    println!("{:-<60}", "");

    let reports = i2c::full_system_scan()?;
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

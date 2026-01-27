use tux_validation::i2c;

fn main() -> anyhow::Result<()> {
    println!("{:<12} | {:<20} | {:<20}", "Bus", "Kernel Detected", "Responding Addresses");
    println!("{:-<60}", "");

    let reports = i2c::full_system_scan()?;
    for report in reports {
        let sysfs_addrs: Vec<String> = report.kernel_detected
            .iter()
            .map(|a| format!("0x{:02x}", a))
            .collect();
        let hw_addrs: Vec<String> = report.hardware_responding
            .iter()
            .map(|a| format!("0x{:02x}", a))
            .collect();
            
        println!("{:<12} | {:<20} | {:<20}", report.bus_path, sysfs_addrs.join(", "), hw_addrs.join(", "));
    }
    Ok(())
}
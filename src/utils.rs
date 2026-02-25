use anyhow::Result;
use udev::Enumerator;

pub fn subsystem_info_udev(subsystem: String) -> Result<()> {
    let mut enumerator = Enumerator::new()?;

    // Filter for subsystem
    enumerator.match_subsystem(subsystem)?;

    for device in enumerator.scan_devices()? {
        println!();
        println!("{:#?}", device);

        println!("  [properties]");
        for property in device.properties() {
            println!("    - {:?} {:?}", property.name(), property.value());
        }

        println!("  [attributes]");
        for attribute in device.attributes() {
            println!("    - {:?} {:?}", attribute.name(), attribute.value());
        }
    }

    Ok(())
}

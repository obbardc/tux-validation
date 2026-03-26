use tux_validation::device::{DeviceAddress, TuxDevice};

#[test]
fn test_udev_usb_extraction_does_not_panic() {
    let mut enumerator = udev::Enumerator::new().expect("Failed to create udev enumerator");
    enumerator.match_subsystem("usb").unwrap();

    let devices: Vec<_> = enumerator.scan_devices().unwrap().collect();

    // CI environment fix - test passes silently
    if devices.is_empty() {
        println!("No USB devices found. Skipping extraction test.");
        return;
    }

    for dev in devices {
        // Only test extraction if it's an actual device (skip interfaces)
        if dev.devtype().is_some_and(|t| t == "usb_device") {
            let tux_dev = TuxDevice::from_udev(&dev);

            // If parser successfully parsed it, verify the invariants
            if let Some(t_dev) = tux_dev {
                assert!(!t_dev.name.is_empty());

                // Assert the enum type matches the subsystem
                if let DeviceAddress::Usb { vid, pid, .. } = t_dev.address {
                    assert!(!vid.is_empty(), "VID should not be empty");
                    assert!(!pid.is_empty(), "PID should not be empty");
                } else {
                    panic!("USB udev device was parsed into wrong DeviceAddress enum variant");
                }
            }
        }
    }
}

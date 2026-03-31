use std::collections::HashMap;
use tux_validation::config::UsbExpectation;
use tux_validation::device::{
    BusStatus, DeviceAddress, DeviceDetails, DeviceStatus, Subsystem, TuxBus, TuxDevice,
    UsbInterface, UsbProperties,
};
use tux_validation::validation::{
    AuditStatus, evaluate_usb_blueprint, find_usb_device, verify_usb_constraints,
};

// USB tests

/// Helper function to spin up a fake USB device in memory
fn create_mock_usb_device(
    vid: &str,
    pid: &str,
    port: &str,
    speed: &str,
    driver: Option<&str>,
    children: Vec<TuxDevice>,
) -> TuxDevice {
    TuxDevice {
        name: format!("Mock USB Device {}:{}", vid, pid),
        address: DeviceAddress::Usb {
            bus: 1,
            port_path: port.to_string(),
            vid: vid.to_string(),
            pid: pid.to_string(),
        },
        status: DeviceStatus {
            in_udev: true,
            hw_responding: Some(true),
            driver_bound: driver.map(|s| s.to_string()),
        },
        details: DeviceDetails::Usb(UsbProperties {
            speed: speed.to_string(),
            dev_num: 2,
            serial_id: "ABCDEF".to_string(),
            interfaces: vec![UsbInterface {
                if_num: 0,
                class: "09".to_string(), // Fake class
                driver: driver.map(|s| s.to_string()),
            }],
        }),
        children,
        attributes: HashMap::new(),
    }
}

/// Builds a realistic TuxBus with a nested topology
fn create_mock_usb_bus() -> TuxBus {
    // Child Device (e.g., a mouse plugged into the hub)
    let mouse = create_mock_usb_device("046d", "c077", "1-1", "12", Some("usbhid"), vec![]);

    // Root Hub (containing the mouse)
    let root_hub = create_mock_usb_device("1d6b", "0002", "1", "480", Some("hub"), vec![mouse]);

    TuxBus {
        name: "Mock USB Controller".into(),
        subsystem: Subsystem::Usb,
        id: "1".into(),
        devices: vec![root_hub],
        status: BusStatus::Active,
        metadata: HashMap::new(),
    }
}

#[test]
fn test_usb_speed_verification_passes() {
    let mock_device =
        create_mock_usb_device("1234", "5678", "1-1.2", "480", Some("usbhub"), Vec::new());

    let expectation = UsbExpectation {
        name: "Internal Hub".into(),
        vid: "1234".into(),
        pid: "5678".into(),
        expected_port: "1-1.2".into(),
        required_driver: "usbhub".into(),
        min_speed: Some("480".into()),
    };

    let (status, checks) = verify_usb_constraints(&mock_device, &expectation);

    assert!(matches!(status, AuditStatus::Pass));

    let speed_check = checks.iter().find(|c| c.name == "Speed").unwrap();
    assert!(speed_check.passed);
    assert_eq!(speed_check.actual, "480");
}

#[test]
fn test_usb_speed_verification_fails_on_bottleneck() {
    let mock_device =
        create_mock_usb_device("1234", "5678", "1-1.2", "12", Some("usbhub"), Vec::new());

    let expectation = UsbExpectation {
        name: "Internal Hub".into(),
        vid: "1234".into(),
        pid: "5678".into(),
        expected_port: "1-1.2".into(),
        required_driver: "usbhub".into(),
        min_speed: Some("480".into()),
    };

    let (status, checks) = verify_usb_constraints(&mock_device, &expectation);

    assert!(matches!(status, AuditStatus::Fail { .. }));
    let speed_check = checks.iter().find(|c| c.name == "Speed").unwrap();
    assert!(!speed_check.passed);
}

#[test]
fn test_find_usb_device_tree_traversal() {
    let buses = vec![create_mock_usb_bus()];

    let root = find_usb_device(&buses, "1d6b", "0002");
    assert!(root.is_some(), "Failed to find top-level root hub");

    let child = find_usb_device(&buses, "046d", "c077");
    assert!(child.is_some(), "Failed to find nested child device");

    let missing = find_usb_device(&buses, "dead", "beef");
    assert!(missing.is_none(), "Falsely found a non-existent device");
}

#[test]
fn test_evaluate_usb_blueprint() {
    let buses = vec![create_mock_usb_bus()];

    // A blueprint asking for one real device and one fake device
    let blueprint = vec![
        UsbExpectation {
            name: "Logitech Mouse".into(),
            vid: "046d".into(),
            pid: "c077".into(),
            expected_port: "1-1".into(),
            required_driver: "usbhid".into(),
            min_speed: Some("12".into()),
        },
        UsbExpectation {
            name: "Missing Webcam".into(),
            vid: "ffff".into(),
            pid: "ffff".into(),
            expected_port: "1-2".into(),
            required_driver: "uvcvideo".into(),
            min_speed: None,
        },
    ];

    let results = evaluate_usb_blueprint(&buses, &blueprint);

    assert_eq!(results.len(), 2);

    let mouse_res = &results[0];
    assert_eq!(mouse_res.item_name, "Logitech Mouse");
    assert!(
        matches!(mouse_res.status, AuditStatus::Pass),
        "Valid mouse failed the audit"
    );

    let cam_res = &results[1];
    assert_eq!(cam_res.item_name, "Missing Webcam");
    assert!(
        matches!(cam_res.status, AuditStatus::Missing { .. }),
        "Missing webcam was not flagged as Missing"
    );
}

// PCIe tests

use tux_validation::config::PciExpectation;
use tux_validation::device::PcieProperties;
use tux_validation::validation::evaluate_pci_blueprint;

/// Helper to spin up a fake PCIe device
fn create_mock_pci_device(
    domain: u16,
    bus: u8,
    device: u8,
    function: u8,
    name: &str,
    driver: Option<&str>,
    cur_speed: Option<&str>,
    cur_width: Option<u8>,
) -> TuxDevice {
    TuxDevice {
        name: name.to_string(),
        address: DeviceAddress::Pci {
            domain,
            bus,
            device,
            function,
        },
        status: DeviceStatus {
            in_udev: true,
            hw_responding: Some(driver.is_some()),
            driver_bound: driver.map(|s| s.to_string()),
        },
        details: DeviceDetails::Pci(PcieProperties {
            vendor_id: "0x10de".into(),
            device_id: "0x1b80".into(),
            vendor_name: "Mock Vendor".into(),
            device_name: "Mock Device".into(),
            class_name: "VGA compatible controller".into(),
            revision: "0xa1".into(),
            max_link_speed: Some("16.0 GT/s PCIe".into()),
            cur_link_speed: cur_speed.map(|s| s.to_string()),
            max_link_width: Some(16),
            cur_link_width: cur_width,
        }),
        children: vec![],
        attributes: HashMap::new(),
    }
}

#[test]
fn test_evaluate_pci_blueprint_passes() {
    // A healthy GPU running at full x16 Gen4 speed
    let gpu = create_mock_pci_device(
        0x0000,
        0x01,
        0x00,
        0x0,
        "NVIDIA GeForce GTX 1080",
        Some("nouveau"),
        Some("16.0 GT/s PCIe"),
        Some(16),
    );

    let pci_bus = TuxBus {
        name: "PCIe Bus".into(),
        subsystem: Subsystem::Pci,
        id: "0".into(),
        devices: vec![gpu],
        status: BusStatus::Active,
        metadata: HashMap::new(),
    };

    let expectation = vec![PciExpectation {
        address: "0000:01:00.0".into(),
        device: Some("NVIDIA".into()),
        driver: Some("nouveau".into()),
        min_link_width: Some(16),
        min_link_speed: Some(16.0),
    }];

    let results = evaluate_pci_blueprint(&[pci_bus], &expectation);

    assert_eq!(results.len(), 1);
    assert!(matches!(results[0].status, AuditStatus::Pass));
}

#[test]
fn test_evaluate_pci_blueprint_fails_and_misses() {
    // GPU is seated poorly or sharing lanes: Negotiated at x8 instead of x16
    let bottlenecked_gpu = create_mock_pci_device(
        0x0000,
        0x01,
        0x00,
        0x0,
        "NVIDIA GeForce GTX 1080",
        Some("nouveau"),
        Some("16.0 GT/s PCIe"),
        Some(8), // <-- Only 8 lanes active!
    );

    let pci_bus = TuxBus {
        name: "PCIe Bus".into(),
        subsystem: Subsystem::Pci,
        id: "0".into(),
        devices: vec![bottlenecked_gpu],
        status: BusStatus::Active,
        metadata: HashMap::new(),
    };

    let expectations = vec![
        PciExpectation {
            address: "0000:01:00.0".into(),
            device: None,
            driver: None,
            min_link_width: Some(16), // Expecting full x16 bandwidth
            min_link_speed: None,
        },
        PciExpectation {
            address: "0000:04:00.0".into(), // A missing NVMe drive
            device: Some("Samsung NVMe".into()),
            driver: None,
            min_link_width: None,
            min_link_speed: None,
        },
    ];

    let results = evaluate_pci_blueprint(&[pci_bus], &expectations);

    assert_eq!(results.len(), 2);

    // Assert the first test failed
    assert!(matches!(results[0].status, AuditStatus::Fail { .. }));

    // Assert it failed specifically because of the Link Width
    let width_check = results[0]
        .checks
        .iter()
        .find(|c| c.name == "PCIe Link Width")
        .unwrap();
    assert!(!width_check.passed);
    assert_eq!(width_check.actual, "x8");

    // Assert second test on NVMe is flagged as Missing
    assert!(matches!(results[1].status, AuditStatus::Missing { .. }));
}

// Network tests

use tux_validation::config::NetworkExpectation;
use tux_validation::device::EthernetProperties;
use tux_validation::validation::evaluate_network_blueprint;

/// Helper to spin up a fake Ethernet device
fn create_mock_eth_device(
    name: &str,
    mac: &str,
    driver: Option<&str>,
    link_up: bool,
    speed: u32,
    ips: Vec<&str>,
) -> TuxDevice {
    TuxDevice {
        name: name.to_string(),
        address: DeviceAddress::Network {
            interface: name.to_string(),
            mac: mac.to_string(),
        },
        status: DeviceStatus {
            in_udev: true,
            hw_responding: Some(link_up), // Carrier
            driver_bound: driver.map(|s| s.to_string()),
        },
        details: DeviceDetails::Ethernet(EthernetProperties {
            speed,
            duplex: "full".into(),
            link_detected: link_up,
            pci_bus_id: Some("0000:02:00.0".into()),
            operstate: if link_up { "up".into() } else { "down".into() },
            ipv4_address: ips.iter().map(|s| s.to_string()).collect(),
            ipv6_address: vec![],
            dhcp_enabled: !ips.is_empty(),
            firmware_version: None,
        }),
        children: vec![],
        attributes: HashMap::new(),
    }
}

#[test]
fn test_evaluate_network_blueprint_ethernet_fails_on_speed_drop() {
    // eth0 is physically plugged in, got an IP, but negotiated a bad 100Mbps link
    let eth0 = create_mock_eth_device(
        "eth0",
        "aa:bb:cc:dd:ee:ff",
        Some("igb"),
        true,
        100,
        vec!["192.168.1.50"],
    );

    let net_bus = TuxBus {
        name: "Network".into(),
        subsystem: Subsystem::Net,
        id: "0".into(),
        devices: vec![eth0],
        status: BusStatus::Active,
        metadata: HashMap::new(),
    };

    let expectation = vec![NetworkExpectation {
        interface_name: "eth0".into(),
        link_status: true, // Expecting carrier
        speed: Some(1000), // Expecting Gigabit! (This should fail)
        driver: Some("igb".into()),
        mac_address: Some("aa:bb:cc:dd:ee:ff".into()),
        expected_ip: None,
        expected_ssid: None,
    }];

    let results = evaluate_network_blueprint(&[net_bus], &expectation);

    assert_eq!(results.len(), 1);

    // It should fail specifically because 100 < 1000
    if let AuditStatus::Fail { reason, .. } = &results[0].status {
        assert!(reason.contains("Speed mismatch"));
    } else {
        panic!("Network test passed when it should have failed on speed bottleneck");
    }

    // Check that the MAC address test specifically passed despite the speed failure
    let mac_check = results[0]
        .checks
        .iter()
        .find(|c| c.name == "MAC Address")
        .unwrap();
    assert!(mac_check.passed);
}

use tux_validation::device::WifiProperties;

// Helper to spin up a fake Wi-Fi device
fn create_mock_wifi_device(
    name: &str,
    mac: &str,
    driver: Option<&str>,
    link_up: bool,
    ssid: Option<&str>,
    signal: i32,
    freq: u32,
    ips: Vec<&str>,
) -> TuxDevice {
    TuxDevice {
        name: name.to_string(),
        address: DeviceAddress::Network {
            interface: name.to_string(),
            mac: mac.to_string(),
        },
        status: DeviceStatus {
            in_udev: true,
            hw_responding: Some(link_up), // Carrier / Associated
            driver_bound: driver.map(|s| s.to_string()),
        },
        details: DeviceDetails::Wifi(WifiProperties {
            ssid: ssid.map(|s| s.to_string()),
            signal_level: signal,
            frequency: freq,
            link_detected: link_up,
            ipv4_address: ips.iter().map(|s| s.to_string()).collect(),
            ipv6_address: vec![],
        }),
        children: vec![],
        attributes: HashMap::new(),
    }
}

#[test]
fn test_evaluate_network_blueprint_wifi_passes() {
    // A healthy Intel Wi-Fi card connected to the corporate network
    let wlan0 = create_mock_wifi_device(
        "wlan0",
        "11:22:33:44:55:66",
        Some("iwlwifi"),
        true,
        Some("Corp_IoT_Net"),
        -45,
        5240,
        vec!["10.0.0.15"],
    );

    let net_bus = TuxBus {
        name: "Network".into(),
        subsystem: Subsystem::Net,
        id: "0".into(),
        devices: vec![wlan0],
        status: BusStatus::Active,
        metadata: HashMap::new(),
    };

    let expectation = vec![NetworkExpectation {
        interface_name: "wlan0".into(),
        link_status: true,
        speed: None, // We don't assert speed on Wi-Fi
        driver: Some("iwlwifi".into()),
        mac_address: None,
        expected_ip: None,                          // Any valid IPv4 is fine
        expected_ssid: Some("Corp_IoT_Net".into()), // Must be on this exact network
    }];

    let results = evaluate_network_blueprint(&[net_bus], &expectation);

    assert_eq!(results.len(), 1);
    assert!(matches!(results[0].status, AuditStatus::Pass));

    // Verify the SSID check specifically passed
    let ssid_check = results[0].checks.iter().find(|c| c.name == "SSID").unwrap();
    assert!(ssid_check.passed);
    assert_eq!(ssid_check.actual, "Corp_IoT_Net");
}

#[test]
fn test_evaluate_network_blueprint_wifi_fails_wrong_ssid() {
    // The device successfully connected to Wi-Fi and got an IP,
    // but it connected to the wrong network (e.g., a guest network instead of production)
    let wlan0 = create_mock_wifi_device(
        "wlan0",
        "11:22:33:44:55:66",
        Some("iwlwifi"),
        true,
        Some("Public_Guest_Wifi"),
        -60,
        2412,
        vec!["192.168.1.100"],
    );

    let net_bus = TuxBus {
        name: "Network".into(),
        subsystem: Subsystem::Net,
        id: "0".into(),
        devices: vec![wlan0],
        status: BusStatus::Active,
        metadata: HashMap::new(),
    };

    let expectation = vec![NetworkExpectation {
        interface_name: "wlan0".into(),
        link_status: true,
        speed: None,
        driver: Some("iwlwifi".into()),
        mac_address: None,
        expected_ip: None,
        expected_ssid: Some("Corp_IoT_Net".into()), // Blueprint demands the secure network
    }];

    let results = evaluate_network_blueprint(&[net_bus], &expectation);

    assert_eq!(results.len(), 1);

    // The overall audit must fail
    assert!(matches!(results[0].status, AuditStatus::Fail { .. }));

    // Ensure it failed specifically because of the SSID mismatch
    let ssid_check = results[0].checks.iter().find(|c| c.name == "SSID").unwrap();
    assert!(!ssid_check.passed);
    assert_eq!(ssid_check.expected, "Corp_IoT_Net");
    assert_eq!(ssid_check.actual, "Public_Guest_Wifi");
}

use tux_validation::config::I2cExpectation;
use tux_validation::device::I2cProperties;
use tux_validation::validation::evaluate_i2c_blueprint;

// Helper to spin up a fake I2C device
fn create_mock_i2c_device(
    bus: u8,
    address: u16,
    name: &str,
    driver: Option<&str>,
    hw_ack: Option<bool>,
) -> TuxDevice {
    TuxDevice {
        name: name.to_string(),
        address: DeviceAddress::I2c { bus, address },
        status: DeviceStatus {
            in_udev: true,
            hw_responding: hw_ack,
            driver_bound: driver.map(|s| s.to_string()),
        },
        details: DeviceDetails::I2c(I2cProperties),
        children: vec![],
        attributes: HashMap::new(),
    }
}

// Helper to wrap I2C devices into a bus
fn create_mock_i2c_bus(bus_id: u8, devices: Vec<TuxDevice>) -> TuxBus {
    TuxBus {
        name: format!("i2c-{}", bus_id),
        subsystem: Subsystem::I2c,
        id: bus_id.to_string(),
        devices,
        status: BusStatus::Active,
        metadata: HashMap::new(),
    }
}

#[test]
fn test_evaluate_i2c_blueprint_passes() {
    // A healthy RTC chip at 0x32 on Bus 7
    let rtc = create_mock_i2c_device(7, 0x32, "rx8010", Some("rtc-rx8010"), Some(true));
    let bus7 = create_mock_i2c_bus(7, vec![rtc]);

    let expectation = vec![I2cExpectation {
        name: "Real-Time Clock".into(),
        bus: 7,
        address: "0x32".into(), // Blueprint uses hex strings
        required_driver: Some("rtc-rx8010".into()),
    }];

    let results = evaluate_i2c_blueprint(&[bus7], &expectation);

    assert_eq!(results.len(), 1);
    assert!(matches!(results[0].status, AuditStatus::Pass));

    // Check that the hardware probe field specifically passed
    let hw_check = results[0]
        .checks
        .iter()
        .find(|c| c.name == "Hardware Probe")
        .unwrap();
    assert!(hw_check.passed);
}

#[test]
fn test_evaluate_i2c_blueprint_fails_hw_nack() {
    // A power management IC that is bound to a driver, but the physical chip is dead/unseated (NACK)
    let pmic = create_mock_i2c_device(0, 0x1b, "rk808", Some("rk808"), Some(false));
    let bus0 = create_mock_i2c_bus(0, vec![pmic]);

    let expectation = vec![I2cExpectation {
        name: "Power Management IC".into(),
        bus: 0,
        address: "0x1b".into(),
        required_driver: Some("rk808".into()),
    }];

    let results = evaluate_i2c_blueprint(&[bus0], &expectation);

    assert_eq!(results.len(), 1);
    assert!(matches!(results[0].status, AuditStatus::Fail { .. }));

    // Verify it failed specifically because of the Hardware Probe (NACK)
    let hw_check = results[0]
        .checks
        .iter()
        .find(|c| c.name == "Hardware Probe")
        .unwrap();
    assert!(!hw_check.passed);
    assert_eq!(hw_check.actual, "false");
}

#[test]
fn test_evaluate_i2c_blueprint_handles_missing_device() {
    // An empty I2C bus
    let bus2 = create_mock_i2c_bus(2, vec![]);

    let expectation = vec![I2cExpectation {
        name: "Missing Temp Sensor".into(),
        bus: 2,
        address: "0x4a".into(),
        required_driver: None,
    }];

    let results = evaluate_i2c_blueprint(&[bus2], &expectation);

    assert_eq!(results.len(), 1);
    // Should immediately flag as Missing without attempting to check fields
    assert!(matches!(results[0].status, AuditStatus::Missing { .. }));
}

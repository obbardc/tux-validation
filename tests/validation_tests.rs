use std::collections::HashMap;
use tux_validation::config::UsbExpectation;
use tux_validation::device::{
    BusStatus, DeviceAddress, DeviceDetails, DeviceStatus, Subsystem, TuxBus, TuxDevice,
    UsbInterface, UsbProperties,
};
use tux_validation::validation::{
    AuditStatus, evaluate_usb_blueprint, find_usb_device, verify_usb_constraints,
};

// Helper function to spin up a fake USB device in memory
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

// Builds a realistic TuxBus with a nested topology
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
fn test_evaluate_usb_blueprint_pipeline() {
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

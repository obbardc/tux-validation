use std::collections::HashMap;
use tux_validation::config::UsbExpectation;
use tux_validation::device::{
    DeviceAddress, DeviceDetails, DeviceStatus, TuxDevice, UsbInterface, UsbProperties,
};
use tux_validation::validation::{AuditStatus, verify_usb_constraints};

// Helper function to spin up a fake USB device in memory
fn create_mock_usb_device(port: &str, speed: &str, driver: Option<&str>) -> TuxDevice {
    TuxDevice {
        name: "Mock USB Hub".to_string(),
        address: DeviceAddress::Usb {
            bus: 1,
            port_path: port.to_string(),
            vid: "1234".to_string(),
            pid: "5678".to_string(),
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
                class: "09".to_string(),
                driver: driver.map(|s| s.to_string()),
            }],
        }),
        children: vec![],
        attributes: HashMap::new(),
    }
}

#[test]
fn test_usb_speed_verification_passes() {
    let mock_device = create_mock_usb_device("1-1.2", "480", Some("usbhub"));

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
    let mock_device = create_mock_usb_device("1-1.2", "12", Some("usbhub"));

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

use std::time::Duration;
use tux_validation::report::{generate_junit_xml, print_xml_summary};
use tux_validation::validation::{AuditStatus, FieldCheck, TargetId, ValidationResult};

#[test]
fn test_junit_xml_generation_and_parsing() {
    // Create a safe temporary file path for the test
    let temp_dir = std::env::temp_dir();
    let xml_path = temp_dir.join("tux_test_report.xml");
    let xml_path_str = xml_path.to_str().unwrap();

    // Create mock validation results (One Pass, One Fail)
    let mock_results = vec![
        ValidationResult {
            subsystem: "PCIe".into(),
            item_name: "NVIDIA GPU".into(),
            target_id: TargetId::Pci {
                address: "0000:01:00.0".into(),
            },
            location: "0000:01:00.0".into(),
            status: AuditStatus::Pass,
            checks: vec![],
            duration: Duration::from_millis(15),
        },
        ValidationResult {
            subsystem: "Systemd".into(),
            item_name: "sshd.service".into(),
            target_id: TargetId::Systemd {
                service: "sshd.service".into(),
            },
            location: "D-Bus".into(),
            status: AuditStatus::Fail {
                reason: "ActiveState mismatch".into(),
                actual_value: "See reason".into(),
            },
            checks: vec![FieldCheck {
                name: "ActiveState".into(),
                passed: false,
                expected: "active".into(),
                actual: "inactive".into(),
            }],
            duration: Duration::from_millis(5),
        },
    ];

    // Test XML Generation
    let gen_result = generate_junit_xml(
        &mock_results,
        xml_path_str,
        Some(Duration::from_millis(50)),
        "Test Suite",
        "Test Scan Phase",
    );
    assert!(gen_result.is_ok(), "Failed to generate XML file");

    // Verify file contents explicitly
    let xml_content = std::fs::read_to_string(&xml_path).expect("Failed to read generated XML");

    // Check that the dynamic error type logic works
    assert!(xml_content.contains("name=\"[PCIe] NVIDIA GPU\""));
    assert!(xml_content.contains("name=\"[Systemd] sshd.service\""));
    assert!(xml_content.contains("type=\"ServiceError\"")); // Systemd should trigger this

    // Verify tests counts using the same parser as in print_xml_summary()
    let doc = roxmltree::Document::parse(&xml_content).expect("Failed to parse XML into DOM");

    let mut total_tests = 0;
    let mut total_failures = 0;

    for node in doc.descendants() {
        if node.has_tag_name("testcase") {
            total_tests += 1;

            // Check if this testcase failed
            let has_failure = node
                .children()
                .any(|c| c.has_tag_name("failure") || c.has_tag_name("error"));

            if has_failure {
                total_failures += 1;
            }
        }
    }

    // 2 mock results + 1 scan phase = 3 total tests
    assert_eq!(
        total_tests, 3,
        "XML reported the wrong number of total tests"
    );

    // Only the Systemd service failed
    assert_eq!(
        total_failures, 1,
        "XML reported the wrong number of failures"
    );

    // Test XML parsing.
    // If it doesn't return an Err(), it means the XML is structurally valid
    let parse_result = print_xml_summary(xml_path_str);
    assert!(
        parse_result.is_ok(),
        "Failed to parse the generated XML file"
    );

    // Clean up the temp file
    let _ = std::fs::remove_file(xml_path);
}

use clap::Parser;
use tux_validation::systemd::audit_systemd_services;

fn main() -> anyhow::Result<()> {
    let test_services = vec![
        "dbus.service".to_string(),                // Almost certainly active
        "systemd-journald.service".to_string(),    // Almost certainly active
        "sshd.service".to_string(),                // Might be active, inactive, or missing
        "NetworkManager.service".to_string(),      // Common on desktop/embedded
        "totally-fake-daemon.service".to_string(), // Guaranteed to fail (NoSuchUnit)
    ];
    let tested_services = audit_systemd_services(&test_services);
    dbg!(tested_services);
    Ok(())
}

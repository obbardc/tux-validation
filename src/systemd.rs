//! Systemd user-space daemon validation via D-Bus.
//!
//! This module communicates directly with the Linux system D-Bus to query the
//! `org.freedesktop.systemd1` interface. It provides a synchronous mechanism to
//! fetch the real-time lifecycle states (Load, Active, Sub) of requested systemd
//! units without needing to parse the output of CLI commands like `systemctl`.

use zbus::{blocking::Connection, proxy};

/// A real-time snapshot of a systemd unit's state scraped from D-Bus.
#[derive(Debug, Clone)]
pub struct SystemdService {
    /// The exact name of the unit (e.g., "NetworkManager.service").
    pub name: String,
    /// `true` if systemd has a record of this unit, `false` if it returned `NoSuchUnit`.
    pub exists: bool,
    /// The human-readable description of the unit (purely for logging/UI).
    pub description: Option<String>,
    /// The unit's configuration load state (e.g., "loaded", "not-found", "masked").
    pub load_state: Option<String>,
    /// The unit's high-level state (e.g., "active", "inactive", "failed").
    pub active_state: Option<String>,
    /// The unit's low-level, type-specific state (e.g., "running", "exited", "dead").
    pub sub_state: Option<String>,
}

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait SystemdManager {
    fn get_unit(&self, name: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait SystemdUnit {
    #[zbus(property)]
    fn description(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn load_state(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn active_state(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn sub_state(&self) -> zbus::Result<String>;
}

/// Synchronously connects to the system D-Bus and scrapes the current state of requested services.
///
/// This function queries the `Systemd1.Manager` interface to find the object path of each
/// requested unit. If the unit exists, it queries the `Systemd1.Unit` interface to extract
/// its exact lifecycle state. It gracefully intercepts `NoSuchUnit` D-Bus errors, marking
/// those specific services as `exists = false` rather than failing the entire audit.
///
/// # Arguments
/// * `service_names` - A slice of systemd unit names (including suffixes, e.g., "sshd.service")
///   to query.
///
/// # Returns
/// A `Result` containing a list of `SystemdService` snapshots reflecting the real-time state
/// of the daemon.
pub fn audit_systemd_services(service_names: &[String]) -> anyhow::Result<Vec<SystemdService>> {
    let connection = Connection::system()?;
    let manager = SystemdManagerProxyBlocking::new(&connection)?;

    let mut scanned_services = Vec::new();

    for name in service_names {
        match manager.get_unit(name) {
            Ok(path) => {
                // Unit exists, fetch its properties
                let unit_proxy = SystemdUnitProxyBlocking::builder(&connection)
                    .path(path)?
                    .build()?;

                let description = unit_proxy.description().ok();
                let load_state = unit_proxy.load_state().ok();
                let active_state = unit_proxy.active_state().ok();
                let sub_state = unit_proxy.sub_state().ok();

                scanned_services.push(SystemdService {
                    name: name.clone(),
                    exists: true,
                    description,
                    load_state,
                    active_state,
                    sub_state,
                });
            }
            Err(zbus::Error::MethodError(err_name, _, _))
                if err_name.as_str() == "org.freedesktop.systemd1.NoSuchUnit" =>
            {
                // Unit does not exist
                scanned_services.push(SystemdService {
                    name: name.clone(),
                    exists: false,
                    description: None,
                    load_state: None,
                    active_state: None,
                    sub_state: None,
                });
            }
            Err(e) => return Err(e.into()), // Bubble up real D-Bus connection errors
        }
    }

    Ok(scanned_services)
}

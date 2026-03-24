use zbus::{blocking::Connection, proxy};

#[derive(Debug, Clone)]
pub struct SystemdService {
    pub name: String,
    pub exists: bool,
    pub description: Option<String>,
    pub load_state: Option<String>,
    pub active_state: Option<String>,
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

/// Synchronously connects to D-Bus and scrapes the current state of requested services.
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

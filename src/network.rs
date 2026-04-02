//! Network subsystem discovery and auditing.
//!
//! This module is responsible for enumerating both wired (Ethernet) and wireless (Wi-Fi)
//! network interfaces. It combines `udev` sysfs data with `nix` (for IP address extraction)
//! and Netlink (for real-time Wi-Fi metadata like SSID and signal strength) to build
//! a complete picture of the device's network state.

use crate::device::{
    BusStatus, DeviceDetails, EthernetProperties, Subsystem, TuxBus, TuxDevice, WifiProperties,
};
use anyhow::Result;
use neli_wifi::Socket;
use nix::ifaddrs::getifaddrs;
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use udev::Enumerator;

/// Scans the system for network interfaces and extracts their hardware and link properties.
///
/// This function performs a comprehensive audit by:
/// 1. Enumerating all network devices via `udev` (ignoring the `lo` loopback interface).
/// 2. Querying the OS for assigned IPv4 and IPv6 addresses (filtering out link-local/APIPA addresses).
/// 3. Communicating with the kernel via Netlink to fetch active Wi-Fi connection details.
///
/// # Returns
/// A `Result` containing a single `TuxBus` (representing the unified "Network Subsystem")
/// populated with all discovered `TuxDevice` network interfaces.
pub fn audit_network_subsystem() -> Result<Vec<TuxBus>> {
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("net")?;

    let mut net_devices = Vec::new();

    // Get IP addresses from the system
    let mut ip_map = get_interface_ips()?;

    // Open Wifi socket
    let mut wifi_socket = Socket::connect().ok();

    for dev in enumerator.scan_devices()? {
        if dev.sysname() == "lo" {
            continue;
        }

        // Check if it's a Wireless device
        let is_wifi =
            dev.devtype().is_some_and(|t| t == "wlan") || dev.attribute_value("phy80211").is_some();

        if let Some(mut tux_dev) = TuxDevice::from_udev(&dev) {
            // General properties
            // IP addresses
            let (v4_addrs, v6_addrs) = ip_map
                .remove(&tux_dev.name)
                .unwrap_or((Vec::new(), Vec::new()));

            // Extract carrier state and treat it as hw probe
            let carrier = dev.attribute_value("carrier").and_then(|v| v.to_str()) == Some("1");
            tux_dev.status.hw_responding = Some(carrier);

            if is_wifi {
                // Wifi
                let (ssid, signal, freq) = wifi_socket
                    .as_mut()
                    .and_then(|sock| {
                        // Query the specific interface by its name (e.g., "wlp0s20f3")
                        get_wifi_meta(sock, &tux_dev.name).ok()
                    })
                    .unwrap_or((None, 0, 0)); // Fallback if query fails
                tux_dev.details = DeviceDetails::Wifi(WifiProperties {
                    ssid,
                    signal_level: signal,
                    frequency: freq,
                    link_detected: carrier,
                    ipv4_address: v4_addrs,
                    ipv6_address: v6_addrs,
                });
            } else {
                // Ethernet
                // Extract extended properties from sysfs via udev attributes
                let speed = dev
                    .attribute_value("speed")
                    .and_then(|v| v.to_str()?.parse::<u32>().ok())
                    .unwrap_or(0);

                let duplex = dev
                    .attribute_value("duplex")
                    .and_then(|v| v.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let operstate = dev
                    .attribute_value("operstate")
                    .and_then(|v| v.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let dhcp_on = !v4_addrs.is_empty(); // This is heuristic: got a non-APIPA IP; TODO: always works?

                tux_dev.details = DeviceDetails::Ethernet(EthernetProperties {
                    speed,
                    duplex,
                    link_detected: carrier,
                    pci_bus_id: dev
                        .property_value("ID_PATH")
                        .and_then(|v| v.to_str().map(|s| s.to_string())),
                    operstate,
                    ipv4_address: v4_addrs,
                    ipv6_address: v6_addrs,
                    dhcp_enabled: dhcp_on,
                    firmware_version: None,
                });
            }

            net_devices.push(tux_dev);
        }
    }

    Ok(vec![TuxBus {
        name: "Network Subsystem".to_string(),
        subsystem: Subsystem::Net,
        id: "0".to_string(),
        devices: net_devices,
        status: BusStatus::Active,
        metadata: HashMap::new(),
    }])
}

/// Uses nix to find IP addresses for all interfaces
#[allow(clippy::type_complexity)]
fn get_interface_ips() -> Result<HashMap<String, (Vec<String>, Vec<String>)>> {
    let mut map = HashMap::new();
    let addrs = getifaddrs()?;

    for ifaddr in addrs {
        let (v4_list, v6_list) = map
            .entry(ifaddr.interface_name.clone())
            .or_insert((Vec::new(), Vec::new()));

        if let Some(address) = ifaddr.address {
            if let Some(sockaddr) = address.as_sockaddr_in() {
                let ip: Ipv4Addr = sockaddr.ip().into();
                let ip_str = ip.to_string();
                // Filter out APIPA (Link-Local) IPv4: 169.254.0.0/16; TODO: correct?
                if !ip_str.starts_with("169.254.") {
                    v4_list.push(ip_str);
                }
            } else if let Some(sockaddr_v6) = address.as_sockaddr_in6() {
                let ip: Ipv6Addr = sockaddr_v6.ip();
                let ip_str = ip.to_string();
                // Filter out Link-Local IPv6: starts with fe80; TODO: correct?
                if !ip_str.starts_with("fe80:") {
                    v6_list.push(ip_str);
                }
            }
        }
    }
    Ok(map)
}

/// Helper to fetch real-time WiFi metadata via Netlink (nl80211)
fn get_wifi_meta(sock: &mut Socket, iface_name: &str) -> Result<(Option<String>, i32, u32)> {
    let iface_index = match nix::net::if_::if_nametoindex(iface_name) {
        Ok(idx) => idx as i32,
        Err(e) => anyhow::bail!("Interface {} not found: {}", iface_name, e),
    };

    // 1. Get SSID & Frequency from Interface Info
    let interfaces = sock.get_interfaces_info().unwrap_or_default();
    let (ssid, freq) = interfaces
        .iter()
        .find(|iface| iface.index == Some(iface_index))
        .map(|iface| {
            let s = iface
                .ssid
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string());
            let f = iface.frequency.unwrap_or(0);
            (s, f)
        })
        .unwrap_or((None, 0));

    // 2. Get Signal Strength from Station Info
    let stations = sock.get_station_info(iface_index).unwrap_or_default();
    let signal = stations
        .first()
        .and_then(|s| s.signal.or(s.average_signal))
        .unwrap_or(0) as i32;

    Ok((ssid, signal, freq))
}

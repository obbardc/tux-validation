use crate::device::{
    BusStatus, DeviceDetails, EthernetProperties, WifiProperties, Subsystem, TuxBus, TuxDevice,
};
use anyhow::Result;
use nix::ifaddrs::getifaddrs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::collections::HashMap;
use udev::Enumerator;

pub fn audit_network_subsystem() -> Result<Vec<TuxBus>> {
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("net")?;

    let mut net_devices = Vec::new();

    // Get IP addresses from the system
    let mut ip_map = get_interface_ips()?;

    for dev in enumerator.scan_devices()? {
        if dev.sysname() == "lo" { continue; }

        // Check if it's a Wireless device
        let is_wifi = dev.devtype().is_some_and(|t| t == "wlan") 
               || dev.attribute_value("phy80211").is_some();

        if let Some(mut tux_dev) = TuxDevice::from_udev(&dev) {
            // General properties
            // IP addresses
            let (v4_addrs, v6_addrs) = ip_map.remove(&tux_dev.name)
            .unwrap_or((Vec::new(), Vec::new()));

            // Extract carrier state and treat it as hw probe
            let carrier = dev.attribute_value("carrier")
                .and_then(|v| v.to_str()) == Some("1");
            tux_dev.status.hw_responding = Some(carrier);

            if is_wifi {
                // Wifi
                tux_dev.details = DeviceDetails::Wifi(WifiProperties {
                    ssid: dev.attribute_value("ssid").map(|s| s.to_string_lossy().into()), // Needs real Wifi tools for full depth, but udev sometimes has this
                    signal_level: 0, // Placeholder: requires nl80211 for accuracy
                    frequency: 0,    // Placeholder
                    link_detected: carrier,
                    ipv4_address: v4_addrs,
                    ipv6_address: v6_addrs,
                });
            } else {
                // Ethernet
                // Extract extended properties from sysfs via udev attributes
                let speed = dev.attribute_value("speed")
                    .and_then(|v| v.to_str()?.parse::<u32>().ok())
                    .unwrap_or(0);
            
                let duplex = dev.attribute_value("duplex")
                    .and_then(|v| v.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let operstate = dev.attribute_value("operstate")
                    .and_then(|v| v.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let dhcp_on  = !v4_addrs.is_empty(); // This is heuristic: got a non-APIPA IP; TODO: always works?

                tux_dev.details = DeviceDetails::Ethernet(EthernetProperties {
                    speed,
                    duplex,
                    link_detected: carrier,
                    pci_bus_id: dev.property_value("ID_PATH").and_then(|v| v.to_str().map(|s| s.to_string())),
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
fn get_interface_ips() -> Result<HashMap<String, (Vec<String>, Vec<String>)>> {
    let mut map = HashMap::new();
    let addrs = getifaddrs()?;

    for ifaddr in addrs {
        let (v4_list, v6_list) = map.entry(ifaddr.interface_name.clone()).or_insert((Vec::new(), Vec::new()));
        
        if let Some(address) = ifaddr.address {
            if let Some(sockaddr) = address.as_sockaddr_in() {
                let ip: Ipv4Addr = sockaddr.ip().into();
                let ip_str = ip.to_string();
                // Filter out APIPA (Link-Local) IPv4: 169.254.0.0/16; TODO: correct?
                if !ip_str.starts_with("169.254.") {
                    v4_list.push(ip_str);
                }
            } else if let Some(sockaddr_v6) = address.as_sockaddr_in6() {
                let ip: Ipv6Addr = sockaddr_v6.ip().into();
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
use crate::device::{
    BusStatus, DeviceDetails, EthernetProperties, Subsystem, TuxBus, TuxDevice,
};
use anyhow::Result;
use nix::ifaddrs::getifaddrs;
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
        // TODO: correct?
        let is_wifi = dev.attribute_value("wireless").is_some() 
            || dev.syspath().to_string_lossy().contains("phy80211");
        if is_wifi { continue };

        if let Some(mut tux_dev) = TuxDevice::from_udev(&dev) {
            // Extract carrier state and treat it as hw probe
            let carrier = dev.attribute_value("carrier")
                .and_then(|v| v.to_str()) == Some("1");
            tux_dev.status.hw_responding = Some(carrier);

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

            let (v4_addrs, v6_addrs) = ip_map.remove(&tux_dev.name)
            .unwrap_or((Vec::new(), Vec::new()));

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
                v4_list.push(sockaddr.ip().to_string());
            } else if let Some(sockaddr_v6) = address.as_sockaddr_in6() {
                v6_list.push(sockaddr_v6.ip().to_string());
            }
        }
    }
    Ok(map)
}
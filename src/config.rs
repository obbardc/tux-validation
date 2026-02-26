use serde::Deserialize;

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    #[serde(default)]
    pub usb_devices: Vec<UsbExpectation>,
    #[serde(default)]
    pub i2c_devices: Vec<I2cExpectation>,
}

#[derive(Deserialize, Debug)]
pub struct UsbExpectation {
    pub name: String,
    pub vid: String,
    pub pid: String,
    pub expected_port: String,
    pub required_driver: String,
    pub min_speed: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct I2cExpectation {
    pub name: String,
    pub bus: u8,
    pub address: String, // e.g., "0x1b"
    pub required_driver: Option<String>,
}

impl I2cExpectation {
    /// Helper to safely parse the hex string from TOML
    pub fn parsed_address(&self) -> Option<u16> {
        let clean = self.address.trim_start_matches("0x");
        u16::from_str_radix(clean, 16).ok()
    }
}
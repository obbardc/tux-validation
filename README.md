# tux-validation

A practical, blueprint-driven system validation framework for Embedded Linux.

`tux-validation` allows hardware engineers and embedded developers to define the expected state of their system in a TOML file and automatically verify it. It is designed to be run during and/or after board bring-up, QA validation, or continuous integration (CI) pipelines.

*Note: This project is under active development. Subsystems and API surfaces may evolve.*

## Currently Supported Subsystems

The framework currently supports auditing the following components:

* **USB:** Verify vendor/product IDs, port topologies, expected drivers, and link speeds.
* **I2C:** Verify bus topologies, device addresses, driver bindings, and live hardware ACK/NACK status.
* **PCIe:** Verify BDF addresses, link widths (e.g., x4 vs x16), negotiated link speeds, and bound drivers.
* **Networking:** Validate Ethernet/Wi-Fi interfaces, MAC addresses, link states, negotiated speeds, and SSID associations.
* **Systemd:** Query D-Bus to ensure critical user-space daemons are loaded, active, and running.

## Dependencies

The framework interacts directly with the Linux kernel via `libudev` and `zbus`. If building natively on a Linux host or devboard, install the required development headers:

```bash
sudo apt-get update
sudo apt-get install libudev-dev pkg-config
```

> **For Embedded Targets**: Linking complex D-Bus macros can cause out-of-memory (OOM) errors on constrained boards. Cross-compiling from a host machine is recommended, e.g.:
> ```bash
> cross build --release --target aarch64-unknown-linux-gnu
> ```

> **Note on I2C Hardware Probing:** If you are using the live hardware probe feature to verify physical I2C ACK/NACK status, your kernel must expose the user-space I2C character devices (`/dev/i2c-*`). You may need to load the module manually before running the tool:
> ```bash
> sudo modprobe i2c-dev
> ```

## Usage: Command-Line Tool
The primary way to use tux-validation is via the unified executable.

### 1. Create a Blueprint
Define your expected hardware topology and software state in a TOML file (e.g., `board_config.toml`):
```toml
[[systemd_services]]
name = "sshd.service"
active_state = "active"
sub_state = "running"

[[usb_devices]]
name = "External Camera"
vid = "046d"
pid = "082d"
required_driver = "uvcvideo"
min_speed = "480"

[[i2c_devices]]
name = "rk808 PMIC"
bus = 0
address = "0x1b"
required_driver = "rk8xx-i2c"

[[network_interfaces]]
interface_name = "wlan0"
link_status = true
expected_ssid = "Corporate_IoT_Net"
```

### 2. Run the Validator
Run the audit against your blueprint. To generate a JUnit XML report for CI systems (like Jenkins or GitHub Actions), use the `-x` flag.
```bash
tux-validation --config board_config.toml -x report.xml --xml-summary
```
If you are interacting directly with hardware (for example, using the `--i2c-hw-probe` flag to perform an `smbus_write_quick`), root privileges are required to access the local device nodes. Also, remember to load the `i2c-dev` module as noted above:
```bash
sudo tux-validation --config board_config.toml --i2c-hw-probe
```

## Usage: As a Rust Library

Because `tux-validation` exposes its core evaluation engine, you can use it as a framework to build your own custom testing tools. 

Since the project is currently in active development, it is not yet published to crates.io. You can add it to your `Cargo.toml` by pointing directly to this repository:

```toml
[dependencies]
# Pull directly from the repository
tux-validation = { git = "https://github.com/obbardc/tux-validation.git", branch = "main" }
```
*(Note: If you are working locally or in a monorepo, you can also use a relative path: `tux-validation = { path = "../tux-validation" }`)*

You can view the `examples/` directory in this repository for reference implementations on how to query specific subsystems (like `udev` or `systemd`) directly from Rust code.

## Testing
The framework includes a functional test suite to verify the core pass/fail logic and OS-level extractors.
```bash
cargo test
```
*(Note: Hardware extraction integration tests will safely skip if run in an empty cloud CI environment).*

## Documentation

Since this framework is currently used as an internal library and not published to `crates.io`, you can generate and view the API documentation locally. 

Run the following command in the root of the repository:

```bash
cargo doc --no-deps --open
```

This will parse all source code docstrings, generate a searchable HTML API reference, and automatically open it in your default web browser.


# tux-validation

System validation framework for Embedded Linux.

## Dependencies

Due to relying on `udev-rs`, requires the following packages installed:
```
sudo apt-get install libudev-dev pkg-config
```

## Usage

### Build examples
Run
```
cargo build --examples
```

### Run examples

<details>
<summary><b>udev_audit</b></summary>
This will be fancy-colored in a terminal:

<pre>
$ ./target/debug/examples/udev_audit

===== UDEV-AUDIT =====

=== I2C SUBSYSTEM ===

I2C Bus (Bus 0)
  • rk808 [0x1b]
    ┗━ Driver: rk8xx-i2c
  • fan53555 [0x60]
    ┗━ Driver: fan53555-regulator

I2C Bus (Bus 4)
  • sgtl5000 [0x0a]
    ┗━ Driver: sgtl5000

I2C Bus (Bus 7)
  • amc6821 [0x18]
    ┗━ Driver: amc6821
  • isl1208 [0x6f]
    ┗━ Driver: none

I2C Bus (Bus 8)
  • fan53555 [0x60]
    ┗━ Driver: fan53555-regulator

=== USB SUBSYSTEM ===

Bus Controller (Bus 1)
• EHCI Host Controller [1d6b:0002] at usb1 (480M)
  ┗━ If 00 [Hub]: Driver hub

Bus Controller (Bus 2)
• Generic Platform OHCI controller [1d6b:0001] at usb2 (12M)
  ┗━ If 00 [Hub]: Driver hub

Bus Controller (Bus 3)
• xHCI Host Controller [1d6b:0002] at usb3 (480M)
  ┗━ If 00 [Hub]: Driver hub
  • USB2.0 Hub [05e3:0610] at 3-1 (480M)
    ┗━ If 00 [Hub]: Driver hub
    • Mule USB/CAN Adapter [2294:425a] at 3-1.4 (12M)
      ┗━ If 00 [Vendor-Specific]: Driver none

</pre>
<pre>
$ sudo ./target/debug/examples/udev_audit --hw-probe ./examples/puma-rk3399.toml 

===== UDEV-AUDIT =====

=== I2C SUBSYSTEM ===

I2C Bus (Bus 0)
  ★ rk808 [0x1b]
    ┗━ Driver rk8xx-i2c - expected (HW: ACK)
  • fan53555 [0x60]
    ┗━ Driver: fan53555-regulator (HW: ACK)

I2C Bus (Bus 4)
  ★ sgtl5000 [0x0a]
    ┗━ Driver sgtl5000 - expected (HW: ACK)

I2C Bus (Bus 7)
  • amc6821 [0x18]
    ┗━ Driver: amc6821 (HW: ACK)
  • isl1208 [0x6f]
    ┗━ Driver: none (HW: ACK)
  ★ Unknown [0x50]
    ┗━ Driver none - expected fan53555-regulator (HW: ACK)

I2C Bus (Bus 8)
  • fan53555 [0x60]
    ┗━ Driver: fan53555-regulator (HW: ACK)

=== USB SUBSYSTEM ===

Bus Controller (Bus 1)
• EHCI Host Controller [1d6b:0002] at usb1 (480M)
  ┗━ If 00 [Hub]: Driver hub

Bus Controller (Bus 2)
• Generic Platform OHCI controller [1d6b:0001] at usb2 (12M)
  ┗━ If 00 [Hub]: Driver hub

Bus Controller (Bus 3)
• xHCI Host Controller [1d6b:0002] at usb3 (480M)
  ┗━ If 00 [Hub]: Driver hub
  ★ USB2.0 Hub [05e3:0610] at 3-1 (480M - expected)
    ┗━ If 00 [Hub]: Driver hub - expected
    ★ Mule USB/CAN Adapter [2294:425a] at 3-1.4 (12M - expected)
      ┗━ If 00 [Vendor-Specific]: Driver none - expected ucan

Bus Controller (Bus 4)
• xHCI Host Controller [1d6b:0003] at usb4 (5000M)
  ┗━ If 00 [Hub]: Driver hub
  • USB3.0 Hub [05e3:0620] at 4-1 (5000M)
    ┗━ If 00 [Hub]: Driver hub
</pre>

where `puma-rk3399.toml` contains
```
[[usb_devices]]
name = "Mule CAN Adapter"
vid = "2294"
pid = "425a"
expected_port = "3-1.4"
required_driver = "ucan"
min_speed = "12M"

[[usb_devices]]
name = "Onboard Hub"
vid = "05e3"
pid = "0610"
expected_port = "3-1"
required_driver = "hub"
min_speed = "480M"

[[i2c_devices]]
name = "rk808 PMIC"
bus = 0
address = "0x1b"
required_driver = "rk8xx-i2c"

[[i2c_devices]]
name = "sgtl5000 Audio Codec"
bus = 4
address = "0x0a"
required_driver = "sgtl5000"

[[i2c_devices]]
name = "Some"
bus = 7
address = "0x50"
required_driver = "fan53555-regulator"
```
</details>

### Runt unit tests
```
$ cargo test
```

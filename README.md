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
<summary><b>i2c_discover_devices</b></summary>
<pre>
$ ./target/debug/examples/i2c_discover_devices                     
--------------------------------------------------------------------------------
Bus ID | Address | Name            | Driver               | SMBus Write Quick
--------------------------------------------------------------------------------
0      | 0x1b    | rk808           | rk8xx-i2c            | false            
       | 0x60    | fan53555        | fan53555-regulator   | false            
--------------------------------------------------------------------------------
4      | 0x0a    | sgtl5000        | sgtl5000             | false            
--------------------------------------------------------------------------------
7      | 0x18    | amc6821         | amc6821              | false            
       | 0x6f    | isl1208         | None                 | false            
--------------------------------------------------------------------------------
8      | 0x60    | fan53555        | fan53555-regulator   | false            
--------------------------------------------------------------------------------
</pre>
<pre>
$ sudo ./target/debug/examples/i2c_discover_devices --hw-probe
--------------------------------------------------------------------------------
Bus ID | Address | Name            | Driver               | SMBus Write Quick
--------------------------------------------------------------------------------
0      | 0x1b    | rk808           | rk8xx-i2c            | true             
       | 0x60    | fan53555        | fan53555-regulator   | true             
--------------------------------------------------------------------------------
4      | 0x0a    | sgtl5000        | sgtl5000             | true             
--------------------------------------------------------------------------------
7      | 0x18    | amc6821         | amc6821              | true             
       | 0x6f    | isl1208         | None                 | true             
       | 0x50    | Unknown         | None                 | true             
--------------------------------------------------------------------------------
8      | 0x60    | fan53555        | fan53555-regulator   | true             
--------------------------------------------------------------------------------
</pre>
</details>

<details>
<summary><b>usb_audit</b></summary>
This will be fancy-colored in a terminal:

<pre>
$ ./target/debug/examples/usb_audit

=== USB SUBSYSTEM ===

Bus Controller (Bus 1)
• Generic Platform OHCI controller [1d6b:0001] at usb1 (12M)
  ┗━ If 00 [Hub]: Driver hub

Bus Controller (Bus 2)
• EHCI Host Controller [1d6b:0002] at usb2 (480M)
  ┗━ If 00 [Hub]: Driver hub

Bus Controller (Bus 3)
• xHCI Host Controller [1d6b:0002] at usb3 (480M)
  ┗━ If 00 [Hub]: Driver hub
  • USB2.0 Hub [05e3:0610] at 3-1 (480M)
    ┗━ If 00 [Hub]: Driver hub
    • Mule USB/CAN Adapter [2294:425a] at 3-1.4 (12M)
      ┗━ If 00 [Vendor-Specific]: Driver none

Bus Controller (Bus 4)
• xHCI Host Controller [1d6b:0003] at usb4 (5000M)
  ┗━ If 00 [Hub]: Driver hub
  • USB3.0 Hub [05e3:0620] at 4-1 (5000M)
    ┗━ If 00 [Hub]: Driver hub
</pre>
<pre>
$ ./target/debug/examples/usb_audit ./examples/puma-rk3399.toml 

=== USB SUBSYSTEM ===

Bus Controller (Bus 1)
• Generic Platform OHCI controller [1d6b:0001] at usb1 (12M)
  ┗━ If 00 [Hub]: Driver hub

Bus Controller (Bus 2)
• EHCI Host Controller [1d6b:0002] at usb2 (480M)
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
```
</details>

### Runt unit tests
```
$ cargo test
```

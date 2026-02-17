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

#### i2c_discover_devices
```
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
```

### Runt unit tests
```
$ cargo test
```

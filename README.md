# Smart Coaster Firmware

This is the code repository for the smart coaster firmware. The hardware design for the device can be found at
https://github.com/paulhampson/smart-coaster-hw

## Install via USB

For devices with the bootloader installed the firmware can be loaded using
the [Web Interface](https://paulhampson.github.io/smart-coaster-fw/).

To enter firmware update mode, press and hold the encoder button for more than 2 seconds when powering on the device.

To load firmware using the CLI follow the steps in [Running from CLI](#running-from-cli).

## Install the latest release to hardware via debugger

Assuming you have `probe-rs` installed ([instructions](https://probe.rs/docs/getting-started/installation/)) and the
debugger attached follow these steps in a directory where you are happy
to download the firmware to.

```aiignore
curl -L https://github.com/paulhampson/smart-coaster-fw/releases/latest/download/smartcoaster-application > smartcoaster-application && \
curl -L https://github.com/paulhampson/smart-coaster-fw/releases/latest/download/smartcoaster-bootloader > smartcoaster-bootloader && \
probe-rs download --chip RP2040 --speed 10000 smartcoaster-bootloader && \
probe-rs reset --chip RP2040 && \
probe-rs download --chip RP2040 --speed 10000 smartcoaster-application
```

## Setting up for development

### Rust

Steps:

1. Rust installation as per [official documentation](https://doc.rust-lang.org/book/ch01-01-installation.html).

2. Install support for ARMv6 targets

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
rustup target add thumbv6m-none-eabi
```

### JLink debugger

Steps:

1. Install tools from https://www.segger.com/downloads/jlink/

### Clion & GDB

1. Setup toolchain
    1. Install gdb-multiarch `sudo apt install gdb-multiarch`
    2. In Clion, goto Settings -> Build, Execution, Deployment -> Toolchains
    3. Create 'Rust Embedded', leave all as defaults except GDB, set this to `/usr/bin/gdb-multiarch`
    4. Targets should be loaded from project.
2. Setup embedded GDB target run configuration (if not already present)
    1. Add an Embedded GDB Server configuration
    2. Remote args = `tcp:localhost:2331`
    3. GDB server = `/usr/bin/JLinkGDBServer` (assuming package was used for JLink install, otherwise the path to
       JLinkGDBServer is required)
    4. GDB server args = `-if swd -device RP2040_M0_0`
    5. Apply, then build, then set the Executable binary (
       `$ProjectFileDir$/target/thumbv6m-none-eabi/debug/rp2040-embassy`)

### Running from CLI

All operations are managed through `xtask`. To get a full list of commands run `cargo xtask help`. A summary of useful
commands is below.

Flash the bootloader:

```aiignore
cargo xtask flash bootloader
```

Flash the application:

```aiignore
cargo xtask flash application
```

Attach to the application:

```aiignore
cargo xtask attach [bootloader|application]
```

Running CLI firmware loader (replace <SERIAL_PORT> with the serial port your device is connected to):

```aiignore
cargo xtask run firmware-loader-cli --port <SERIAL_PORT> target/thumbv6m-none-eabi/release/smartcoaster-application.bin
cargo xtask run firmware-loader-cli --log-level DEBUG --port <SERIAL_PORT> target/thumbv6m-none-eabi/release/smartcoaster-application.bin
```

Standalone firmware loader can be obtained
from the [latest release](https://github.com/paulhampson/smart-coaster-fw/releases/latest/).

# Design

See [DESIGN_NOTES.md](docs/DESIGN_NOTES.md)

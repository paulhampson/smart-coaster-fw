# Smart Coaster - Firmware

This is the code repository for the smart coaster firmware. The hardware design for the device can be found at
https://github.com/paulhampson/smart-coaster-hw

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

```aiignore
cargo run --release
```

# Design

See [DESIGN_NOTES.md](DESIGN_NOTES.md)

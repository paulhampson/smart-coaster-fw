// Copyright (C) 2025 Paul Hampson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License version 3 as  published by the
// Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.

use std::process::Command;
use std::env;
use std::path::PathBuf;

#[derive(Debug)]
enum BuildTarget {
    Bootloader,
    Application,
    FirmwareLoaderCli,
    HostCore,
    Both,
}

#[derive(Debug)]
enum Command_ {
    Build(BuildTarget),
    Flash(BuildTarget),
    Run(BuildTarget, Vec<String>),
    Help,
    Attach(BuildTarget),
    Wasm { release: bool, output: PathBuf },
    WasmWatch,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let command = parse_command(&args[1..]);

    if let Err(e) = command {
        eprintln!("Error parsing command: {}", e);
        print_usage();
        std::process::exit(1);
    }

    if let Err(e) = execute_command(command.unwrap()) {
        eprintln!("Error executing command: {}", e);
        std::process::exit(1);
    }
}

fn parse_command(args: &[String]) -> Result<Command_, String> {
    if args.is_empty() {
        return Err("No command provided".to_string());
    }

    match args[0].as_str() {
        "attach" => {
            let target = if args.len() > 1 {
                parse_build_target(&args[1])?
            } else {
                return Err("run command requires a target (bootloader or application)".to_string());
            };
            match target {
                BuildTarget::Bootloader | BuildTarget::Application => {
                    Ok(Command_::Attach(target))
                }
                _ => {
                    Err("run command only supports bootloader, application".to_string())
                }
            }
        }
        "build" => {
            let target = if args.len() > 1 {
                parse_build_target(&args[1])?
            } else {
                BuildTarget::Both
            };
            Ok(Command_::Build(target))
        }
        "flash" => {
            let target = if args.len() > 1 {
                parse_build_target(&args[1])?
            } else {
                BuildTarget::Both
            };
            Ok(Command_::Flash(target))
        }
        "run" => {
            let target = if args.len() > 1 {
                parse_build_target(&args[1])?
            } else {
                return Err("run command requires a target (bootloader, application, or firmware-loader-cli)".to_string());
            };
            match target {
                BuildTarget::Both => {
                    Err("run command only supports bootloader, application, or firmware-loader-cli, not both".to_string())
                }
                _ => {
                    // Collect any additional arguments after the target
                    let extra_args = if args.len() > 2 {
                        args[2..].to_vec()
                    } else {
                        Vec::new()
                    };
                    Ok(Command_::Run(target, extra_args))
                }
            }
        }
        "wasm" => {
            let mut release = false;
            let mut output = PathBuf::from("web-interface/pkg");

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--release" | "-r" => release = true,
                    "--output" | "-o" => {
                        if i + 1 < args.len() {
                            output = PathBuf::from(&args[i + 1]);
                            i += 1;
                        } else {
                            return Err("--output requires a path argument".to_string());
                        }
                    }
                    _ => return Err(format!("Unknown wasm flag: {}", args[i])),
                }
                i += 1;
            }

            Ok(Command_::Wasm { release, output })
        }
        "wasm-watch" => Ok(Command_::WasmWatch),
        "help" => Ok(Command_::Help),
        _ => Err(format!("Unknown command: {}", args[0])),
    }
}

fn parse_build_target(target: &str) -> Result<BuildTarget, String> {
    match target {
        "bootloader" => Ok(BuildTarget::Bootloader),
        "application" => Ok(BuildTarget::Application),
        "firmware-loader-cli" => Ok(BuildTarget::FirmwareLoaderCli),
        "host-core" => Ok(BuildTarget::HostCore),
        "both" => Ok(BuildTarget::Both),
        _ => Err(format!("Unknown target: {}", target)),
    }
}

fn execute_command(cmd: Command_) -> Result<(), String> {
    match cmd {
        Command_::Build(target) => build(&target),
        Command_::Flash(target) => flash(&target),
        Command_::Run(target, extra_args) => run(&target, extra_args),
        Command_::Attach(target) => attach(&target),
        Command_::Wasm { release, output } => build_wasm(release, &output),
        Command_::WasmWatch => watch_wasm(),
        Command_::Help => {
            print_usage();
            Ok(())
        }
    }
}

fn build(target: &BuildTarget) -> Result<(), String> {
    match target {
        BuildTarget::Bootloader => {
            println!("Building bootloader...");
            run_cargo_build("smartcoaster-bootloader", "thumbv6m-none-eabi")?;
            generate_bin("smartcoaster-bootloader")?;
            println!("✓ Bootloader built successfully");
        }
        BuildTarget::Application => {
            println!("Building application...");
            run_cargo_build("smartcoaster-application", "thumbv6m-none-eabi")?;
            generate_bin("smartcoaster-application")?;
            println!("✓ Application built successfully");
        }
        BuildTarget::FirmwareLoaderCli => {
            println!("Building firmware-loader-cli...");
            run_cargo_build("firmware-loader-cli", "x86_64-unknown-linux-gnu")?;
            println!("✓ Firmware-loader-cli built successfully");
        }
        BuildTarget::HostCore => {
            println!("Building host-core...");
            run_cargo_build("smartcoaster-host-core", "x86_64-unknown-linux-gnu")?;
            println!("✓ Host-core built successfully");
        }
        BuildTarget::Both => {
            println!("Building bootloader...");
            run_cargo_build("smartcoaster-bootloader", "thumbv6m-none-eabi")?;
            generate_bin("smartcoaster-bootloader")?;
            println!("✓ Bootloader built successfully");
            println!("Building application...");
            run_cargo_build("smartcoaster-application", "thumbv6m-none-eabi")?;
            generate_bin("smartcoaster-application")?;
            println!("✓ Application built successfully");
        }
    }
    Ok(())
}

fn flash(target: &BuildTarget) -> Result<(), String> {
    match target {
        BuildTarget::Bootloader => {
            println!("Building and flashing bootloader...");
            run_cargo_flash("smartcoaster-bootloader")?;
            println!("✓ Bootloader flashed successfully");
        }
        BuildTarget::Application => {
            println!("Building and flashing application...");
            run_cargo_flash("smartcoaster-application")?;
            println!("✓ Application flashed successfully");
        }
        BuildTarget::FirmwareLoaderCli => {
            println!("Error: Cannot flash firmware-loader-cli (it's a host tool, not firmware)");
            return Err("firmware-loader-cli cannot be flashed".to_string());
        }
        BuildTarget::HostCore => {
            println!("Error: Cannot flash host-core (it's a library, not firmware)");
            return Err("host-core cannot be flashed".to_string());
        }
        BuildTarget::Both => {
            println!("Building and flashing bootloader...");
            run_cargo_flash("smartcoaster-bootloader")?;
            println!("✓ Bootloader flashed successfully");
            println!("Resetting device...");
            run_probe_rs_reset()?;
            println!("Building and flashing application...");
            run_cargo_flash("smartcoaster-application")?;
            println!("✓ Application flashed successfully");
        }
    }
    Ok(())
}

fn run(target: &BuildTarget, extra_args: Vec<String>) -> Result<(), String> {
    match target {
        BuildTarget::Bootloader => {
            println!("Building and running bootloader...");
            run_cargo_run("smartcoaster-bootloader", "thumbv6m-none-eabi", &[])?;
            println!("✓ Bootloader run completed");
        }
        BuildTarget::Application => {
            println!("Building and running application...");
            run_cargo_run("smartcoaster-application", "thumbv6m-none-eabi", &[])?;
            println!("✓ Application run completed");
        }
        BuildTarget::FirmwareLoaderCli => {
            println!("Building and running firmware-loader-cli...");
            if extra_args.is_empty() {
                println!("Note: Consider providing a serial port as an argument");
                println!("Usage: cargo xtask run firmware-loader-cli -- <SERIAL_PORT>");
            }
            run_cargo_run("firmware-loader-cli", "x86_64-unknown-linux-gnu", &extra_args)?;
            println!("✓ Firmware-loader-cli run completed");
        }
        BuildTarget::HostCore => {
            println!("Error: Cannot run host-core (it's a library, not an executable)");
            return Err("host-core cannot be run directly".to_string());
        }
        BuildTarget::Both => {
            unreachable!("Both target should have been rejected in parse_command")
        }
    }
    Ok(())
}

fn attach(target: &BuildTarget) -> Result<(), String> {
    match target {
        BuildTarget::Bootloader => {
            println!("Attaching to bootloader...");
            run_probe_rs_attach("smartcoaster-bootloader")?;
            println!("✓ Bootloader attach completed");
        }
        BuildTarget::Application => {
            println!("Attaching to application...");
            run_probe_rs_attach("smartcoaster-application")?;
            println!("✓ Application attach completed");
        }
        BuildTarget::FirmwareLoaderCli => {
            unreachable!("FirmwareLoaderCli should have been rejected in parse_command")
        }
        BuildTarget::HostCore => {
            unreachable!("HostCore should have been rejected in parse_command")
        }
        BuildTarget::Both => {
            unreachable!("Both target should have been rejected in parse_command")
        }
    }
    Ok(())
}

fn run_cargo_build(package: &str, target: &str) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(&[
            "build",
            "--release",
            "--package",
            package,
            "--target",
            target,
        ])
        .status()
        .map_err(|e| format!("Failed to run cargo build: {}", e))?;

    if !output.success() {
        return Err(format!(
            "Build failed for {}",
            package
        ));
    }

    Ok(())
}

fn run_cargo_flash(package: &str) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(&[
            "flash",
            "--release",
            "--package",
            package,
            "--target",
            "thumbv6m-none-eabi",
            "--chip",
            "RP2040",
        ])
        .status()
        .map_err(|e| format!("Failed to run cargo flash: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Flash failed for {}",
            package
        ));
    }

    Ok(())
}

fn run_cargo_run(package: &str, target: &str, extra_args: &[String]) -> Result<(), String> {
    let mut cmd = Command::new("cargo");
    cmd.args(&[
        "run",
        "--release",
        "--package",
        package,
        "--target",
        target,
    ]);

    // Add separator and extra arguments if provided
    if !extra_args.is_empty() {
        cmd.arg("--");
        for arg in extra_args {
            cmd.arg(arg);
        }
    }

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to run cargo run: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Run failed for {}",
            package
        ));
    }

    Ok(())
}

fn generate_bin(package: &str) -> Result<(), String> {
    let elf_path = PathBuf::from(format!(
        "target/thumbv6m-none-eabi/release/{}",
        package
    ));
    let bin_path = PathBuf::from(format!(
        "target/thumbv6m-none-eabi/release/{}.bin",
        package
    ));

    if !elf_path.exists() {
        return Err(format!(
            "ELF binary not found at {}",
            elf_path.display()
        ));
    }

    println!(
        "Generating .bin file for {}...",
        package
    );

    let output = Command::new("arm-none-eabi-objcopy")
        .args(&[
            "-O",
            "binary",
            elf_path.to_str().unwrap(),
            bin_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| {
            format!(
                "Failed to run arm-none-eabi-objcopy: {}. Make sure arm-none-eabi-objcopy is installed.",
                e
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "Failed to generate .bin file for {}:\n{}",
            package,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    println!("✓ Generated {}", bin_path.display());
    Ok(())
}

fn run_probe_rs_reset() -> Result<(), String> {
    let output = Command::new("probe-rs")
        .args(&["reset", "--chip", "RP2040"])
        .output()
        .map_err(|e| format!("Failed to run probe-rs reset: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Reset failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn run_probe_rs_attach(package: &str) -> Result<(), String> {
    let elf_path = PathBuf::from(format!(
        "target/thumbv6m-none-eabi/release/{}",
        package
    ));

    if !elf_path.exists() {
        return Err(format!(
            "ELF binary not found at {}. Build the project first using 'cargo xtask build'.",
            elf_path.display()
        ));
    }

    let status = Command::new("probe-rs")
        .args(&[
            "attach",
            "--chip",
            "RP2040",
            elf_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("Failed to run probe-rs attach: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Attach failed for {}",
            package
        ));
    }

    Ok(())
}

fn build_wasm(release: bool, output: &PathBuf) -> Result<(), String> {
    println!("🔨 Building WASM package for smartcoaster-host-core...");

    ensure_wasm_pack_installed()?;

    let smartcoaster_host_core = find_crate_dir("smartcoaster-host-core")?;

    let mut cmd = Command::new("wasm-pack");
    cmd.arg("build")
        .arg(&smartcoaster_host_core)
        .arg("--target")
        .arg("web")
        .arg("--out-dir")
        .arg(output);

    if release {
        cmd.arg("--release");
        println!("📦 Building in release mode (optimized)...");
    } else {
        println!("📦 Building in dev mode (faster compilation)...");
    }

    let status = cmd.status()
        .map_err(|e| format!("Failed to build WASM package: {}", e))?;

    if !status.success() {
        return Err("WASM build failed".to_string());
    }

    println!("✓ WASM package built successfully at: {}", output.display());
    println!("\n📝 Next steps:");
    println!("  1. Serve the web interface: python3 -m http.server --directory web-interface");
    println!("  2. Open http://localhost:8000 in your browser");

    Ok(())
}

fn watch_wasm() -> Result<(), String> {
    println!("👀 Watching smartcoaster-host-core for changes...");

    ensure_wasm_pack_installed()?;

    let smartcoaster_host_core = find_crate_dir("smartcoaster-host-core")?;

    let status = Command::new("wasm-pack")
        .arg("build")
        .arg(&smartcoaster_host_core)
        .arg("--target")
        .arg("web")
        .arg("--watch")
        .status()
        .map_err(|e| format!("Failed to watch WASM package: {}", e))?;

    if !status.success() {
        return Err("WASM watch failed".to_string());
    }

    Ok(())
}

fn ensure_wasm_pack_installed() -> Result<(), String> {
    let output = Command::new("wasm-pack")
        .arg("--version")
        .output();

    if output.is_err() {
        return Err(
            "wasm-pack is not installed. Install it with:\n\
                cargo install wasm-pack\n".to_string()
        );
    }

    Ok(())
}


fn find_crate_dir(crate_name: &str) -> Result<PathBuf, String> {
    let manifest_path = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| ".".to_string());

    let root = PathBuf::from(&manifest_path)
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .to_path_buf();

    let crate_path = root.join(crate_name);

    if !crate_path.exists() {
        return Err(format!(
            "Crate directory not found: {}",
            crate_path.display()
        ));
    }

    Ok(crate_path)
}

fn print_usage() {
    eprintln!(
        "Usage: cargo xtask <COMMAND> [TARGET]\n\
         \n\
         Commands:\n\
         \tbuild       Build the specified target or both (generates .bin files)\n\
         \tflash       Build and flash the specified target or both\n\
         \trun         Build and run the specified target (bootloader, application, or firmware-loader-cli)\n\
         \tattach      Attach to the specified target with probe-rs (bootloader or application)\n\
         \thelp        Show this help message\n\
         \n\
         Targets:\n\
         \tbootloader              Build/flash/run/attach the bootloader\n\
         \tapplication             Build/flash/run/attach the application\n\
         \tfirmware-loader-cli     Build/run the firmware-loader-cli (x86_64)\n\
         \tboth                    Build/flash both (bootloader and application, default if target not specified for build/flash)\n\
         \n\
         Examples:\n\
         \tcargo xtask build                                    # Build both with .bin generation\n\
         \tcargo xtask build bootloader                         # Build bootloader only with .bin generation\n\
         \tcargo xtask build application                        # Build application only with .bin generation\n\
         \tcargo xtask build firmware-loader-cli                # Build firmware-loader-cli for x86_64\n\
         \tcargo xtask flash                                    # Flash both\n\
         \tcargo xtask flash bootloader                         # Flash bootloader only\n\
         \tcargo xtask flash application                        # Flash application only\n\
         \tcargo xtask run bootloader                           # Run bootloader with probe-rs\n\
         \tcargo xtask run application                          # Run application with probe-rs\n\
         \tcargo xtask run firmware-loader-cli /dev/ttyUSB0     # Run firmware-loader-cli with serial port\n\
         \tcargo xtask run firmware-loader-cli --log-level DEBUG /dev/ttyUSB0  # Run with DEBUG log level\n\
         \tcargo xtask attach                                   # Attach to application with probe-rs\n\
         \tcargo xtask attach bootloader                        # Attach to bootloader with probe-rs\n\
         \tcargo xtask attach application                       # Attach to application with probe-rs\n\
         \nWASM/WEB EXAMPLES:\n\
         \tcargo xtask wasm                                     # Build WASM (dev mode)\n\
         \tcargo xtask wasm --release                           # Build WASM (release/optimized)\n\
         \tcargo xtask wasm -o ./dist                           # Build WASM to custom directory\n\
         \tcargo xtask wasm-watch                               # Watch WASM sources and rebuild\n\
         \n\
         \tcargo xtask help                                     # Show this help message"
    );
}

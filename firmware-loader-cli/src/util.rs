use std::fs::File;
use log::LevelFilter;
use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult};

/// Calculates the Ascon-Hash256 of the given data
pub(crate) fn calculate_ascon_hash256(data: &[u8]) -> [u8; 32] {
    use ascon_hash::digest::Digest;
    use ascon_hash::AsconHash256;

    let mut hasher = AsconHash256::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(&result[..32]);
    hash_bytes
}

/// Reads a binary file and returns its contents as a Vec<u8>
pub(crate) fn read_binary_file(path: &str) -> IoResult<Vec<u8>> {
    let mut file = File::open(path)
        .map_err(|e| IoError::new(ErrorKind::NotFound, format!("Failed to open file: {}", e)))?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    log::trace!("Read {} bytes from {}", contents.len(), path);
    Ok(contents)
}

/// Extract the firmware file path from command-line arguments
/// Skips --log-level and its value, and --port and its value
pub(crate) fn extract_firmware_file_path(args: &[String]) -> IoResult<String> {
    let mut i = 1; // Skip program name

    while i < args.len() {
        match args[i].as_str() {
            "--log-level" => {
                i += 2; // Skip flag and value
            }
            "--port" => {
                i += 2; // Skip flag and value
            }
            _ => {
                // First non-flag argument is the firmware file path
                return Ok(args[i].clone());
            }
        }
    }

    Err(IoError::new(
        ErrorKind::InvalidInput,
        "No firmware file path provided. Usage: firmware-loader-cli [--log-level LEVEL] [--port PORT] <firmware.bin>",
    ))
}

/// Parse log level from command-line arguments
/// Supports: --log-level <LEVEL> or RUST_LOG environment variable
/// Defaults to INFO if neither is provided
pub(crate) fn parse_log_level() -> LevelFilter {
    let args: Vec<String> = std::env::args().collect();

    // Check for --log-level argument
    for i in 0..args.len() {
        if args[i] == "--log-level" && i + 1 < args.len() {
            return match args[i + 1].to_uppercase().as_str() {
                "OFF" => LevelFilter::Off,
                "ERROR" => LevelFilter::Error,
                "WARN" => LevelFilter::Warn,
                "INFO" => LevelFilter::Info,
                "DEBUG" => LevelFilter::Debug,
                "TRACE" => LevelFilter::Trace,
                _ => {
                    eprintln!("Unknown log level: {}. Using INFO", args[i + 1]);
                    LevelFilter::Info
                }
            };
        }
    }

    // Default to INFO
    LevelFilter::Info
}
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

use log::LevelFilter;
use std::fs;
use std::io::{Error as IoError, ErrorKind, Result as IoResult};

pub(crate) fn parse_log_level() -> LevelFilter {
    std::env::args()
        .position(|arg| arg == "--log-level")
        .and_then(|i| std::env::args().nth(i + 1))
        .as_deref()
        .map(|level_str| match level_str.to_uppercase().as_str() {
            "TRACE" => LevelFilter::Trace,
            "DEBUG" => LevelFilter::Debug,
            "INFO" => LevelFilter::Info,
            "WARN" => LevelFilter::Warn,
            "ERROR" => LevelFilter::Error,
            _ => LevelFilter::Info,
        })
        .unwrap_or(LevelFilter::Info)
}

pub(crate) fn extract_firmware_file_path(args: &[String]) -> IoResult<String> {
    // Find the last argument that doesn't start with '--'
    args.iter()
        .rfind(|arg| !arg.starts_with("--"))
        .and_then(|arg| {
            // Make sure it's not a value for another flag
            let prev_idx = args.iter().position(|a| a == arg)?;
            if prev_idx > 0 {
                let prev = &args[prev_idx - 1];
                if prev == "--log-level" || prev == "--port" {
                    return Some(arg.clone());
                }
            }
            Some(arg.clone())
        })
        .ok_or_else(|| IoError::new(ErrorKind::InvalidInput, "No firmware file path provided"))
}

pub(crate) fn read_binary_file(path: &str) -> IoResult<Vec<u8>> {
    fs::read(path)
        .map_err(|e| IoError::new(ErrorKind::Other, format!("Failed to read firmware file: {}", e)))
}
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

use crate::GeneralMessages;
use crate::custom_data_types::VersionNumber;
use crate::general::hello::{Hello, HelloResp, SystemMode};

/// A builder for creating `GeneralMessages`.
pub struct GeneralMessagesBuilder;

impl GeneralMessagesBuilder {
    /// Creates a new `GeneralMessagesBuilder`.
    pub fn new() -> Self {
        Self
    }

    /// Builds a `GeneralMessages::Hello` message.
    pub fn hello(self) -> GeneralMessages {
        GeneralMessages::Hello(Hello {})
    }

    /// Begins building a `GeneralMessages::HelloResp` message.
    pub fn hello_resp(self) -> HelloRespBuilder {
        HelloRespBuilder::new()
    }
}

impl Default for GeneralMessagesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A builder for creating a `GeneralMessages::HelloResp` message.
pub struct HelloRespBuilder {
    mode: Option<SystemMode>,
    version: Option<VersionNumber>,
}

impl HelloRespBuilder {
    fn new() -> Self {
        Self {
            mode: None,
            version: None,
        }
    }

    /// Sets the system mode for the `HelloResp` message.
    pub fn mode(mut self, mode: SystemMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Sets the version number for the `HelloResp` message.
    pub fn version(mut self, version: VersionNumber) -> Self {
        self.version = Some(version);
        self
    }

    /// Builds the `GeneralMessages::HelloResp` message.
    ///
    /// # Panics
    ///
    /// Panics if `mode` or `version` have not been set.
    pub fn build(self) -> GeneralMessages {
        GeneralMessages::HelloResp(HelloResp {
            mode: self.mode.expect("mode must be set"),
            version: self.version.expect("version must be set"),
        })
    }
}

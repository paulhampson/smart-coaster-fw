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

use crate::storage::storage_manager::StoredLogConfig;
use crate::ACTIVITY_LOG_NVM_FLASH_OFFSET_RANGE;

pub enum Logs {
    ConsumptionLog,
    ErrorLog,
}

impl Logs {
    pub fn get_config(self) -> StoredLogConfig {
        match self {
            Logs::ConsumptionLog => StoredLogConfig {
                storage_range: ACTIVITY_LOG_NVM_FLASH_OFFSET_RANGE,
                allow_overwrite_old: true,
            },
            Logs::ErrorLog => {
                todo!("Error log not configured yet")
            }
        }
    }
}

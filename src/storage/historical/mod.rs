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

pub mod accessor;
pub mod log_config;
pub mod manager;

use crate::storage::StoredDataValue;
use chrono::NaiveDateTime;

#[derive(Debug)]
pub struct LogEntry {
    pub timestamp: NaiveDateTime,
    pub data: StoredDataValue,
}

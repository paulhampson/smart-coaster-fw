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

use crate::storage::settings::SettingsAccessorId;
use defmt::Format;

#[derive(Clone, Copy, Debug, PartialEq, Format)]
pub enum ConfirmationId {
    ClearHistoricalConsumptionLog,
}

#[derive(Debug, Format, Clone, Copy, PartialEq)]
pub enum ApplicationState {
    Startup,
    TestScreen,
    ErrorScreenWithMessage(&'static str),
    Settings,
    Monitoring,
    HeapStatus,
    Calibration,
    SetSystemDateTime,
    DateTimeEntry(SettingsAccessorId),
    NumberEntry(SettingsAccessorId),
    TimeEntry(SettingsAccessorId),
    AboutScreen,
    ConfirmationScreen(ConfirmationId),
}

#[derive(Clone, PartialEq, Debug)]
pub enum CalibrationStateSubstates {
    Tare,
    Wait,
    Calibration(u32),
    CalibrationDone,
}

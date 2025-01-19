use defmt::Format;

#[derive(Debug, Format, Clone, Copy, PartialEq)]
pub enum ApplicationState {
    Startup,
    TestScreen,
    Tare,
    Calibration(u32),
    CalibrationDone,
    Wait,
    ErrorScreenWithMessage(&'static str),
    Settings,
    Monitoring,
    HeapStatus,
}

#[derive(Clone)]
pub enum MonitoringStateSubstates {
    WaitingForActivity,
    VesselRemoved,
    VesselPlaced,
}

#[derive(Clone)]
pub enum CalibrationStateSubstates {
    Tare,
    Wait,
    Calibration(u32),
    CalibrationDone,
}

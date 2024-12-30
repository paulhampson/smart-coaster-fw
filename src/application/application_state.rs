use defmt::Format;

#[derive(Debug, Format, Clone, Copy, PartialEq)]
pub enum ApplicationState {
    Startup,
    WaitingForActivity,
    TestScreen,
    Tare,
    Calibration(u32),
    CalibrationDone,
    Wait,
    ErrorScreenWithMessage(&'static str),
}
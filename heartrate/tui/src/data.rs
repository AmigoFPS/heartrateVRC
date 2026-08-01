use heartrate_core::hrv::HrvMetrics;

#[derive(Debug, Clone, Default)]
pub struct HeartRateData {
    pub bpm: u16,
    pub hrv: Option<HrvMetrics>,
}

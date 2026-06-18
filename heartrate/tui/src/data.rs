use crate::hrv::HrvMetrics;

#[derive(Debug, Clone, Default)]
pub struct HeartRateData {
    pub bpm: u16,
    pub hrv: HrvMetrics,
    pub battery: u8,
}

#[derive(Debug, Clone, Default)]
pub struct HeartRateData {
    pub bpm: u16,
    pub intervals: Vec<u16>,
    pub battery: u8,
}

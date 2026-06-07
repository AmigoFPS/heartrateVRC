#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub enum Page {
    #[default]
    Heartrate,
    Rmssd,
    Sdnn,
    Pnn50,
    Logs,
}

impl From<Page> for usize {
    fn from(value: Page) -> Self {
        match value {
            Page::Heartrate => 0,
            Page::Rmssd => 1,
            Page::Sdnn => 2,
            Page::Pnn50 => 3,
            Page::Logs => 4,
        }
    }
}

impl From<Page> for Option<usize> {
    fn from(value: Page) -> Self {
        match value {
            Page::Heartrate => Some(0),
            Page::Rmssd => Some(1),
            Page::Sdnn => Some(2),
            Page::Pnn50 => Some(3),
            Page::Logs => Some(4),
        }
    }
}

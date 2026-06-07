use crate::page::Page;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct App {
    pub heartrate_data: Vec<(f64, f64)>,
    pub current_page: Page,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn quit(&mut self) {
        self.should_quit = true
    }
}

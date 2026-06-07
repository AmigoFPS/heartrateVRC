#[derive(Default, Debug, Clone)]
pub struct App {
    counter: u8,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn counter(&self) -> u8 {
        self.counter
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub(crate) fn quit(&mut self) {
        self.should_quit = true
    }

    pub(crate) fn increment_counter(&mut self) {
        if let Some(res) = self.counter.checked_add(1) {
            self.counter = res
        }
    }

    pub(crate) fn decrement_counter(&mut self) {
        if let Some(res) = self.counter.checked_sub(1) {
            self.counter = res
        }
    }
}

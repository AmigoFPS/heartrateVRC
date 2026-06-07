use crate::page::Page;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct App {
    current_page: Page,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_page(&self) -> Page {
        self.current_page
    }

    pub fn set_current_page(&mut self, page: Page) {
        self.current_page = page;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub(crate) fn quit(&mut self) {
        self.should_quit = true
    }
}

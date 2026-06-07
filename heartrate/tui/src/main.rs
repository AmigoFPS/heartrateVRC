use std::io;

use heartrate_tui::app::App;

fn main() -> io::Result<()> {
    ratatui::run(|terminal| App::default().run(terminal))
}

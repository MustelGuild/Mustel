pub mod app;
pub mod components;

use crate::error::Result;
use app::MustelApp;

pub fn run_gui() -> Result<()> {
    iced::application(
        || (MustelApp::default(), iced::Task::none()),
        MustelApp::update,
        MustelApp::view,
    )
    .title("Mustel - Database Toolkit")
    .theme(MustelApp::theme)
    .run()
    .map_err(|e| crate::error::MustelError::Config(format!("GUI error: {}", e)))?;

    Ok(())
}

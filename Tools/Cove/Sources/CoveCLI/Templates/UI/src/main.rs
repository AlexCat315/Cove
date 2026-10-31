//! Application entry point. Every platform host runs the same shared root.

mod app;
mod resources;

fn main() -> Result<(), String> {
    cove_ui::hosts::Application::run(
        cove_ui::hosts::ApplicationConfiguration::default(),
        None,
        cove_ui::composition::AnyView::new(app::make_application_content()),
        cove_ui::hosts::ApplicationCallbacks::default(),
        None,
    )
}

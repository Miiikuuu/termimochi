mod color_picker;
mod layout;
mod preview;
mod preview_context;
mod preview_inspect;
mod prompt;
mod prompt_diagnostics;
mod ptyxis;
mod starship_draft;
mod starship_editor;
mod starship_file;
mod starship_import;
mod starship_modules;
mod starship_scene;
mod style;
mod typography;
mod typography_apply;
mod typography_preset;
mod window;

use adw::prelude::*;
use gtk::{gdk, gio};

pub(crate) const APPLICATION_ID: &str = "io.github.miiikuuu.termimochi";
pub(crate) const RESOURCE_BASE: &str = "/io/github/miiikuuu/termimochi";

fn main() -> gtk::glib::ExitCode {
    gio::resources_register_include!("termimochi.gresource")
        .expect("TermiMochi resources must be embedded in the application");

    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    application.connect_startup(|_| {
        if let Some(display) = gdk::Display::default() {
            gtk::IconTheme::for_display(&display)
                .add_resource_path(&format!("{RESOURCE_BASE}/icons"));
        }
        gtk::Window::set_default_icon_name(APPLICATION_ID);
    });

    application.connect_activate(|application| {
        if let Some(window) = application.active_window() {
            window.present();
        } else {
            window::present(application, None);
        }
    });

    application.connect_open(|application, files, _hint| {
        let path = files.first().and_then(gio::File::path);
        window::present(application, path);
    });

    application.set_accels_for_action("win.open", &["<Control>o"]);
    application.set_accels_for_action("win.save", &["<Control>s"]);
    application.set_accels_for_action("win.save-as", &["<Control><Shift>s"]);
    application.set_accels_for_action("win.undo", &["<Control>z"]);
    application.set_accels_for_action("win.redo", &["<Control><Shift>z", "<Control>y"]);
    application.set_accels_for_action("win.show-palette", &["<Control>1"]);
    application.set_accels_for_action("win.show-typography", &["<Control>2"]);
    application.set_accels_for_action("win.show-layout", &["<Control>3"]);
    application.set_accels_for_action("win.show-prompt", &["<Control>4"]);

    application.run()
}

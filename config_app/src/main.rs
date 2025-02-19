use eframe::{epaint::Vec2, IconData, NativeOptions};
use ui::launcher::Launcher;

#[cfg(windows)]
use eframe::Renderer;

mod plot_backend;
mod ui;
mod window;
mod ghapi;

// IMPORTANT. On windows, only the i686-pc-windows-msvc target is supported (Due to limitations with J2534 and D-PDU!
#[cfg(all(target_arch = "x86_64", target_os = "windows"))]
compile_error!("Windows can ONLY be built using the i686-pc-windows-msvc target!");

fn main() {
    env_logger::init();

    let icon = image::load_from_memory(include_bytes!("../icon.png"))
        .unwrap()
        .to_rgba8();
    let (icon_w, icon_h) = icon.dimensions();

    #[cfg(unix)]
    std::env::set_var("WINIT_UNIX_BACKEND", "x11");

    let mut app = window::MainWindow::new();
    app.add_new_page(Box::new(Launcher::new()));
    let mut native_options = NativeOptions::default();
    native_options.vsync = true;
    native_options.icon_data = Some(IconData {
        rgba: icon.into_raw(),
        width: icon_w,
        height: icon_h,
    });
    native_options.initial_window_size = Some(Vec2::new(1280.0, 720.0));
    #[cfg(windows)]
    {
        native_options.renderer = Renderer::Wgpu;
    }
    eframe::run_native(
        "Ultimate NAG52 config suite",
        native_options,
        Box::new(|cc| Box::new(app)),
    );
}

#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
//! Riomat: simulador 1D de transporte de trazador en ríos a partir de datos de HEC-RAS.
mod app;
mod plot;
mod theme;

use gpui_kit::component::Root;
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::*;

fn main() {
    let args: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    gpui_kit::application().run(move |cx| {
        app::registrar_atajos(cx);
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        // La ventana se adapta a la pantalla (en Windows con escala del 125-150 % el espacio lógico es menor).
        let (mut ancho, mut alto) = (1380.0_f32, 860.0_f32);
        if let Some(d) = cx.primary_display() {
            let b = d.bounds();
            ancho = ancho.min(f32::from(b.size.width) * 0.94);
            alto = alto.min(f32::from(b.size.height) * 0.88);
        }
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(ancho), px(alto)), cx))),
            window_min_size: Some(size(px(860.), px(520.))),
            titlebar: Some(TitlebarOptions { title: Some("Riomat".into()), ..Default::default() }),
            app_id: Some("riomat".into()),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| {
            let view = cx.new(|cx| app::Riomat::new(args, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("no pude abrir la ventana");
        cx.activate(true);
    });
}

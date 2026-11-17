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
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1380.), px(860.)), cx))),
            window_min_size: Some(size(px(1000.), px(600.))),
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

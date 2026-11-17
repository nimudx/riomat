//! Gráficas de líneas propias: varias series, ejes lineales o logarítmicos, rellenos y marcas.
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::theme::*;

pub use riomat_plot::{Ejes, Marca, Serie, num, techo_bonito, ticks_lineales, tr};

pub fn grafica(series: Vec<Serie>, marcas: Vec<Marca>, ejes: Ejes, alto: f32) -> impl IntoElement {
    let xt: Vec<String> = ejes.x_ticks.iter().map(|&v| num(v)).collect();
    let yt: Vec<String> = ejes.y_ticks.iter().rev().map(|&v| num(v)).collect();
    let (xr, yr, xl, yl) = (ejes.x, ejes.y, ejes.x_log, ejes.y_log);
    let xticks = ejes.x_ticks.clone();
    let yticks = ejes.y_ticks.clone();
    let lienzo = canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            let pos = |x: f64, y: f64| {
                point(b.origin.x + px((tr(x, xr, xl).clamp(-0.05, 1.05) as f32) * w), b.origin.y + px((1.0 - tr(y, yr, yl).clamp(-0.05, 1.05) as f32) * h))
            };
            for &y in &yticks {
                let p = pos(xr.0, y);
                window.paint_quad(fill(Bounds::new(point(b.origin.x, p.y), size(px(w), px(1.))), rgba((GRID << 8) | 0xff)));
            }
            for &x in &xticks {
                let p = pos(x, yr.0);
                window.paint_quad(fill(Bounds::new(point(p.x, b.origin.y), size(px(1.), px(h))), rgba((GRID << 8) | 0xff)));
            }
            for s in &series {
                if s.puntos.len() < 2 {
                    continue;
                }
                let color: Hsla = rgba((s.color << 8) | if s.opaca { 0xff } else { 0xb0 }).into();
                if s.relleno {
                    let mut pb = PathBuilder::fill();
                    pb.move_to(pos(s.puntos[0].0, yr.0));
                    for &(x, y) in &s.puntos {
                        pb.line_to(pos(x, y));
                    }
                    pb.line_to(pos(s.puntos[s.puntos.len() - 1].0, yr.0));
                    pb.close();
                    if let Ok(path) = pb.build() {
                        let f: Hsla = rgba((s.color << 8) | 0x30).into();
                        window.paint_path(path, f);
                    }
                }
                let mut pb = PathBuilder::stroke(px(s.grosor));
                pb.move_to(pos(s.puntos[0].0, s.puntos[0].1));
                for &(x, y) in &s.puntos[1..] {
                    pb.line_to(pos(x, y));
                }
                if let Ok(path) = pb.build() {
                    window.paint_path(path, color);
                }
            }
            for m in &marcas {
                let p = pos(m.x, m.y);
                let r = 5.0;
                window.paint_quad(
                    fill(Bounds::new(point(p.x - px(r + 2.), p.y - px(r + 2.)), size(px(2. * r + 4.), px(2. * r + 4.))), rgba((PANEL << 8) | 0xff)).corner_radii(px(r + 2.)),
                );
                window.paint_quad(fill(Bounds::new(point(p.x - px(r), p.y - px(r)), size(px(2. * r), px(2. * r))), rgba((m.color << 8) | 0xff)).corner_radii(px(r)));
            }
        },
    )
    .size_full();

    let etiqueta = |t: String| div().text_size(px(11.)).text_color(rgb(MUTED)).child(t);
    v_flex()
        .w_full()
        .gap_1()
        .child(div().text_size(px(11.)).text_color(rgb(DIM)).child(ejes.y_titulo))
        .child(
            h_flex()
                .h(px(alto))
                .gap_2()
                .child(v_flex().w(px(44.)).h_full().justify_between().items_end().children(yt.into_iter().map(etiqueta)))
                .child(div().flex_1().h_full().rounded_md().bg(rgb(PLOT_BG)).border_1().border_color(rgb(BORDER)).child(lienzo)),
        )
        .child(
            h_flex().pl(px(52.)).justify_between().children(xt.into_iter().map(etiqueta)),
        )
        .child(div().w_full().text_center().text_size(px(11.)).text_color(rgb(DIM)).child(ejes.x_titulo))
}

/// Mapa de calor (p. ej. espacio-tiempo) con barra de color y ejes.
pub fn mapa_calor(f: riomat_plot::Figura, alto: f32) -> impl IntoElement {
    let ejes = f.ejes.clone();
    let m = f.mapa.clone().expect("figura sin mapa");
    let xt: Vec<String> = ejes.x_ticks.iter().map(|&v| num(v)).collect();
    let yt: Vec<String> = ejes.y_ticks.iter().rev().map(|&v| num(v)).collect();
    let (nx, ny) = (m.nx, m.ny);
    let colores = m.colores.clone();
    let lienzo = canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            let (cw, ch) = (w / nx as f32, h / ny as f32);
            for j in 0..ny {
                for i in 0..nx {
                    let c = colores[j * nx + i];
                    // fila 0 = valor menor de Y, abajo; se solapa 0.5 px para evitar costuras
                    let o = point(b.origin.x + px(i as f32 * cw), b.origin.y + px(h - (j + 1) as f32 * ch));
                    window.paint_quad(fill(Bounds::new(o, size(px(cw + 0.5), px(ch + 0.5))), rgb(c)));
                }
            }
        },
    )
    .size_full();
    let barra = m.barra.clone();
    let nb = barra.len().max(1);
    let lienzo_barra = canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            let ch = h / nb as f32;
            for (k, c) in barra.iter().enumerate() {
                let o = point(b.origin.x, b.origin.y + px(h - (k + 1) as f32 * ch));
                window.paint_quad(fill(Bounds::new(o, size(px(w), px(ch + 0.5))), rgb(*c)));
            }
        },
    )
    .size_full();
    let etiqueta = |t: String| div().text_size(px(11.)).text_color(rgb(MUTED)).child(t);
    v_flex()
        .w_full()
        .gap_1()
        .child(div().text_size(px(11.)).text_color(rgb(DIM)).child(format!("{} · {}", ejes.y_titulo, m.barra_titulo)))
        .child(
            h_flex()
                .h(px(alto))
                .gap_2()
                .child(v_flex().w(px(44.)).h_full().justify_between().items_end().children(yt.into_iter().map(etiqueta)))
                .child(div().flex_1().h_full().rounded_md().overflow_hidden().border_1().border_color(rgb(BORDER)).child(lienzo))
                .child(
                    h_flex().gap_1().h_full().child(div().w(px(14.)).h_full().rounded_sm().overflow_hidden().border_1().border_color(rgb(BORDER)).child(lienzo_barra)).child(
                        v_flex().h_full().justify_between().child(etiqueta(m.barra_max.clone())).child(etiqueta("0".into())),
                    ),
                ),
        )
        .child(h_flex().pl(px(52.)).pr(px(96.)).justify_between().children(xt.into_iter().map(etiqueta)))
        .child(div().w_full().text_center().text_size(px(11.)).text_color(rgb(DIM)).child(ejes.x_titulo))
}

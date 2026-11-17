//! Generador de SVG a partir de una `Figura`. Estilo de publicación: fondo blanco, texto oscuro.
use crate::modelo::*;
use std::fmt::Write;

const FUENTE: &str = "DejaVu Sans, Arial, Helvetica, sans-serif";

fn hex(c: u32) -> String {
    format!("#{c:06x}")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Dibuja la figura en un lienzo de `ancho` × `alto` unidades.
pub fn a_svg(f: &Figura, ancho: f64, alto: f64) -> String {
    let (izq, der, sup, inf) = (66.0, 20.0, 58.0, 54.0);
    let (x0, y0, x1, y1) = (izq, sup, ancho - der, alto - inf);
    let (w, h) = (x1 - x0, y1 - y0);
    let e = &f.ejes;
    let px = |x: f64| x0 + tr(x, e.x, e.x_log).clamp(-0.05, 1.05) * w;
    let py = |y: f64| y1 - tr(y, e.y, e.y_log).clamp(-0.05, 1.05) * h;
    let mut s = String::new();
    let _ = write!(s, r##"<svg xmlns="http://www.w3.org/2000/svg" width="{ancho}" height="{alto}" viewBox="0 0 {ancho} {alto}" font-family="{FUENTE}">"##);
    let _ = write!(s, r##"<rect width="{ancho}" height="{alto}" fill="#ffffff"/>"##);
    let _ = write!(s, r##"<defs><clipPath id="area"><rect x="{x0}" y="{y0}" width="{w}" height="{h}"/></clipPath></defs>"##);
    // Título
    let _ = write!(s, r##"<text x="{x0}" y="22" font-size="14" font-weight="bold" fill="#22272c">{}</text>"##, esc(&f.titulo));
    // Leyenda
    let mut lx = x0;
    for se in f.series.iter().filter(|se| !se.nombre.is_empty()) {
        let _ = write!(s, r##"<rect x="{lx}" y="33" width="11" height="11" rx="2" fill="none" stroke="{}" stroke-width="2.4"/>"##, hex(se.color));
        let _ = write!(s, r##"<text x="{}" y="43" font-size="11" fill="#555">{}</text>"##, lx + 16.0, esc(&se.nombre));
        lx += 24.0 + se.nombre.chars().count() as f64 * 6.2;
    }
    // Fondo y rejilla
    let _ = write!(s, r##"<rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="#fcfbf8" stroke="#cfc9bc"/>"##);
    for &t in &e.y_ticks {
        let y = py(t);
        let _ = write!(s, r##"<line x1="{x0}" y1="{y}" x2="{x1}" y2="{y}" stroke="#e6e1d6" stroke-width="1"/>"##);
        let _ = write!(s, r##"<text x="{}" y="{}" font-size="10.5" fill="#444" text-anchor="end">{}</text>"##, x0 - 7.0, y + 3.5, num(t));
    }
    for &t in &e.x_ticks {
        let x = px(t);
        let _ = write!(s, r##"<line x1="{x}" y1="{y0}" x2="{x}" y2="{y1}" stroke="#e6e1d6" stroke-width="1"/>"##);
        let _ = write!(s, r##"<text x="{x}" y="{}" font-size="10.5" fill="#444" text-anchor="middle">{}</text>"##, y1 + 15.0, num(t));
    }
    // Series
    let _ = write!(s, r##"<g clip-path="url(#area)">"##);
    for se in &f.series {
        if se.puntos.len() < 2 {
            continue;
        }
        let pts: String = se.puntos.iter().map(|&(x, y)| format!("{:.2},{:.2}", px(x), py(y))).collect::<Vec<_>>().join(" ");
        if se.relleno {
            let (a, b) = (se.puntos[0].0, se.puntos[se.puntos.len() - 1].0);
            let _ = write!(s, r##"<polygon points="{:.2},{y1} {pts} {:.2},{y1}" fill="{}" fill-opacity="0.18"/>"##, px(a), px(b), hex(se.color));
        }
        let _ = write!(s, r##"<polyline points="{pts}" fill="none" stroke="{}" stroke-width="{}" stroke-opacity="{}" stroke-linejoin="round" stroke-linecap="round"/>"##, hex(se.color), se.grosor, if se.opaca { 1.0 } else { 0.7 });
    }
    s.push_str("</g>");
    for m in &f.marcas {
        let _ = write!(s, r##"<circle cx="{:.2}" cy="{:.2}" r="5" fill="{}" stroke="#ffffff" stroke-width="2"/>"##, px(m.x), py(m.y), hex(m.color));
    }
    // Títulos de ejes
    let _ = write!(s, r##"<text x="{}" y="{}" font-size="11.5" fill="#333" text-anchor="middle">{}</text>"##, x0 + w / 2.0, alto - 10.0, esc(&e.x_titulo));
    let _ = write!(s, r##"<text transform="translate(16 {}) rotate(-90)" font-size="11.5" fill="#333" text-anchor="middle">{}</text>"##, y0 + h / 2.0, esc(&e.y_titulo));
    s.push_str("</svg>");
    s
}

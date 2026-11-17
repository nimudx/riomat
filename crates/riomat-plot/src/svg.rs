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

/// Documento SVG completo de `ancho` × `alto` unidades.
pub fn a_svg(f: &Figura, ancho: f64, alto: f64) -> String {
    let mut s = String::new();
    let _ = write!(s, r##"<svg xmlns="http://www.w3.org/2000/svg" width="{ancho}" height="{alto}" viewBox="0 0 {ancho} {alto}" font-family="{FUENTE}">"##);
    let _ = write!(s, r##"<rect width="{ancho}" height="{alto}" fill="#ffffff"/>"##);
    s.push_str(&cuerpo(f, ancho, alto, "a"));
    s.push_str("</svg>");
    s
}

/// Elementos de la figura sin el envoltorio `<svg>` (para incrustarla en otros documentos). `id` evita choques de `clipPath`.
pub fn cuerpo(f: &Figura, ancho: f64, alto: f64, id: &str) -> String {
    let (izq, der, inf) = (66.0, if f.mapa.is_some() { 132.0 } else { 20.0 }, 54.0);
    // El título se parte en dos líneas si no cabe (≈ 8.4 unidades por carácter a 13 de tamaño)
    let maxc = (((ancho - izq - 10.0) / 8.4) as usize).max(20);
    let lineas_t = partir(&f.titulo, maxc, 2);
    let extra = 16.0 * (lineas_t.len() as f64 - 1.0);
    let sup = 58.0 + extra;
    let (x0, y0, x1, y1) = (izq, sup, ancho - der, alto - inf);
    let (w, h) = (x1 - x0, y1 - y0);
    let e = &f.ejes;
    let px = |x: f64| x0 + tr(x, e.x, e.x_log).clamp(-0.05, 1.05) * w;
    let py = |y: f64| y1 - tr(y, e.y, e.y_log).clamp(-0.05, 1.05) * h;
    let mut s = String::new();
    let _ = write!(s, r##"<defs><clipPath id="{id}"><rect x="{x0}" y="{y0}" width="{w}" height="{h}"/></clipPath></defs>"##);
    // Título
    for (k, l) in lineas_t.iter().enumerate() {
        let _ = write!(s, r##"<text x="{x0}" y="{}" font-size="13" font-weight="bold" fill="#22272c">{}</text>"##, 22.0 + 16.0 * k as f64, esc(l));
    }
    // Leyenda
    let mut lx = x0;
    for se in f.series.iter().filter(|se| !se.nombre.is_empty()) {
        let _ = write!(s, r##"<rect x="{lx}" y="{}" width="11" height="11" rx="2" fill="none" stroke="{}" stroke-width="2.4"/>"##, 33.0 + extra, hex(se.color));
        let _ = write!(s, r##"<text x="{}" y="{}" font-size="11" fill="#555">{}</text>"##, lx + 16.0, 43.0 + extra, esc(&se.nombre));
        lx += 24.0 + se.nombre.chars().count() as f64 * 6.2;
    }
    // Fondo y rejilla
    let _ = write!(s, r##"<rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="#fcfbf8" stroke="#cfc9bc"/>"##);
    for &t in &e.y_ticks {
        let y = py(t);
        if f.mapa.is_none() {
            let _ = write!(s, r##"<line x1="{x0}" y1="{y}" x2="{x1}" y2="{y}" stroke="#e6e1d6" stroke-width="1"/>"##);
        }
        let _ = write!(s, r##"<text x="{}" y="{}" font-size="10.5" fill="#444" text-anchor="end">{}</text>"##, x0 - 7.0, y + 3.5, num(t));
    }
    for &t in &e.x_ticks {
        let x = px(t);
        if f.mapa.is_none() {
            let _ = write!(s, r##"<line x1="{x}" y1="{y0}" x2="{x}" y2="{y1}" stroke="#e6e1d6" stroke-width="1"/>"##);
        }
        let _ = write!(s, r##"<text x="{x}" y="{}" font-size="10.5" fill="#444" text-anchor="middle">{}</text>"##, y1 + 15.0, num(t));
    }
    // Mapa de calor: celdas y barra de color
    if let Some(m) = &f.mapa {
        let (cw, ch) = (w / m.nx as f64, h / m.ny as f64);
        for j in 0..m.ny {
            for i in 0..m.nx {
                let c = m.colores[j * m.nx + i];
                // la fila 0 (valor menor de Y) va abajo; se solapa 0.4 para evitar costuras
                let _ = write!(s, r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" shape-rendering="crispEdges"/>"##, x0 + i as f64 * cw, y1 - (j + 1) as f64 * ch, cw + 0.4, ch + 0.4, hex(c));
            }
        }
        let _ = write!(s, r##"<rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="none" stroke="#cfc9bc"/>"##);
        let bx = x1 + 16.0;
        let n = m.barra.len().max(1);
        for (k, c) in m.barra.iter().enumerate() {
            let _ = write!(s, r##"<rect x="{bx}" y="{:.2}" width="12" height="{:.2}" fill="{}" shape-rendering="crispEdges"/>"##, y1 - (k + 1) as f64 * h / n as f64, h / n as f64 + 0.4, hex(*c));
        }
        let _ = write!(s, r##"<rect x="{bx}" y="{y0}" width="12" height="{h}" fill="none" stroke="#cfc9bc"/>"##);
        let _ = write!(s, r##"<text x="{}" y="{}" font-size="10" fill="#444">0</text>"##, bx + 16.0, y1 + 3.5);
        let _ = write!(s, r##"<text x="{}" y="{}" font-size="10" fill="#444">{}</text>"##, bx + 16.0, y0 + 9.0, esc(&m.barra_max));
        let _ = write!(s, r##"<text x="{bx}" y="{}" font-size="9.5" fill="#666">{}</text>"##, y0 - 8.0, esc(&m.barra_titulo));
    }
    // Series
    let _ = write!(s, r##"<g clip-path="url(#{id})">"##);
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
    s
}

/// Parte un texto en como máximo `max_lineas` líneas de `n` caracteres (la última se recorta con «…»).
pub(crate) fn partir(texto: &str, n: usize, max_lineas: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut actual = String::new();
    for palabra in texto.split_whitespace() {
        if !actual.is_empty() && actual.chars().count() + 1 + palabra.chars().count() > n {
            out.push(std::mem::take(&mut actual));
        }
        if !actual.is_empty() {
            actual.push(' ');
        }
        actual.push_str(palabra);
    }
    if !actual.is_empty() {
        out.push(actual);
    }
    if out.len() > max_lineas {
        out.truncate(max_lineas);
        if let Some(u) = out.last_mut() {
            let mut t: String = u.chars().take(n.saturating_sub(1)).collect();
            t.push('…');
            *u = t;
        }
    }
    out
}

//! Informe de una página (A4) en SVG, listo para convertir a PDF: parámetros, tabla de escenarios, figuras y avisos.
use crate::modelo::Figura;
use crate::svg::cuerpo;
use std::fmt::Write;

pub struct Informe {
    pub titulo: String,
    pub subtitulo: String,
    pub parametros: Vec<(String, String)>,
    /// Primera fila = cabecera.
    pub tabla: Vec<Vec<String>>,
    /// Hasta dos figuras.
    pub figuras: Vec<Figura>,
    pub avisos: Vec<String>,
    pub alcance: String,
    pub pie: String,
}

const FUENTE: &str = "DejaVu Sans, Arial, Helvetica, sans-serif";
// A4 en puntos
const W: f64 = 595.0;
const H: f64 = 842.0;
const M: f64 = 36.0;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn partir(texto: &str, n: usize) -> Vec<String> {
    crate::svg::partir(texto, n, usize::MAX)
}

pub fn informe_svg(i: &Informe) -> String {
    let mut s = String::new();
    let _ = write!(s, r##"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" font-family="{FUENTE}"><rect width="{W}" height="{H}" fill="#ffffff"/>"##);
    // Cabecera
    let _ = write!(s, r##"<rect x="0" y="0" width="{W}" height="6" fill="#0e7c86"/>"##);
    let _ = write!(s, r##"<text x="{M}" y="38" font-size="14" font-weight="bold" fill="#22272c">{}</text>"##, esc(&i.titulo));
    let _ = write!(s, r##"<text x="{M}" y="54" font-size="9" fill="#66655f">{}</text>"##, esc(&i.subtitulo));
    let mut y = 74.0;

    // Parámetros: los cortos en dos columnas, los largos a todo el ancho
    let _ = write!(s, r##"<text x="{M}" y="{y}" font-size="10" font-weight="bold" fill="#0e7c86">PARÁMETROS Y DATOS</text>"##);
    y += 6.0;
    let (cortos, largos): (Vec<&(String, String)>, Vec<&(String, String)>) = i.parametros.iter().partition(|(_, b)| b.chars().count() <= 34);
    let mitad = cortos.len().div_ceil(2);
    for (k, (a, b)) in cortos.iter().enumerate() {
        let (col, fila) = (k / mitad.max(1), k % mitad.max(1));
        let x = M + col as f64 * (W - 2.0 * M) / 2.0;
        let yy = y + 11.0 * (fila + 1) as f64;
        let _ = write!(s, r##"<text x="{x}" y="{yy}" font-size="8" fill="#66655f">{}</text><text x="{}" y="{yy}" font-size="8" fill="#22272c">{}</text>"##, esc(a), x + 98.0, esc(b));
    }
    y += 11.0 * mitad as f64;
    for (a, b) in largos {
        for (n, l) in partir(b, 100).iter().enumerate() {
            y += 11.0;
            let _ = write!(s, r##"<text x="{M}" y="{y}" font-size="8" fill="#66655f">{}</text><text x="{}" y="{y}" font-size="8" fill="#22272c">{}</text>"##, if n == 0 { esc(a) } else { String::new() }, M + 98.0, esc(l));
        }
    }
    y += 22.0;

    // Tabla de escenarios
    let _ = write!(s, r##"<text x="{M}" y="{y}" font-size="10" font-weight="bold" fill="#0e7c86">ESCENARIOS</text>"##);
    y += 6.0;
    let ncol = i.tabla.first().map(|f| f.len()).unwrap_or(1).max(1);
    let ancho_t = W - 2.0 * M;
    let pesos: Vec<f64> = (0..ncol).map(|c| if c < 2 { 1.6 } else { 1.0 }).collect();
    let total: f64 = pesos.iter().sum();
    for (r, fila) in i.tabla.iter().enumerate() {
        let yy = y + 12.0 * (r + 1) as f64;
        if r == 0 {
            let _ = write!(s, r##"<rect x="{M}" y="{}" width="{ancho_t}" height="13" fill="#e8e3d9"/>"##, yy - 9.5);
        } else if r % 2 == 0 {
            let _ = write!(s, r##"<rect x="{M}" y="{}" width="{ancho_t}" height="12" fill="#f7f5f0"/>"##, yy - 9.0);
        }
        let mut x = M + 3.0;
        for (c, celda) in fila.iter().enumerate() {
            let _ = write!(s, r##"<text x="{x}" y="{yy}" font-size="7.8" {} fill="#22272c">{}</text>"##, if r == 0 { r##"font-weight="bold""## } else { "" }, esc(celda));
            x += ancho_t * pesos[c] / total;
        }
    }
    y += 12.0 * (i.tabla.len() + 1) as f64 + 6.0;

    // Figuras (hasta dos), apiladas
    let ancho_f = W - 2.0 * M;
    let (fw, fh) = (720.0, 280.0);
    let esc_f = ancho_f / fw;
    for (k, f) in i.figuras.iter().take(2).enumerate() {
        let _ = write!(s, r##"<g transform="translate({M} {y}) scale({esc_f})">{}</g>"##, cuerpo(f, fw, fh, &format!("f{k}")));
        y += fh * esc_f + 4.0;
    }

    // Avisos
    y += 10.0;
    let _ = write!(s, r##"<text x="{M}" y="{y}" font-size="10" font-weight="bold" fill="#c9801a">VERIFICACIÓN Y ADVERTENCIAS</text>"##);
    let mut yy = y + 4.0;
    let limite = H - 52.0;
    'avisos: for a in &i.avisos {
        for (n, linea) in partir(a, 130).iter().enumerate() {
            yy += 9.0;
            if yy > limite - 9.0 {
                let _ = write!(s, r##"<text x="{M}" y="{limite}" font-size="7.5" fill="#66655f">… (resto de avisos en verificacion.csv)</text>"##);
                break 'avisos;
            }
            let _ = write!(s, r##"<text x="{}" y="{yy}" font-size="7.5" fill="#444">{}{}</text>"##, M + if n == 0 { 0.0 } else { 6.0 }, if n == 0 { "• " } else { "" }, esc(linea));
        }
    }

    // Alcance y pie
    let lineas = partir(&i.alcance, 150);
    for (n, l) in lineas.iter().enumerate() {
        let _ = write!(s, r##"<text x="{M}" y="{}" font-size="7" fill="#66655f">{}</text>"##, H - 30.0 + 8.0 * n as f64, esc(l));
    }
    let _ = write!(s, r##"<text x="{M}" y="{}" font-size="7" fill="#9b988d">{}</text>"##, H - 12.0, esc(&i.pie));
    s.push_str("</svg>");
    s
}

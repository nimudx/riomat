//! Conversión de SVG a PNG y PDF (Rust puro; sirve igual en Linux y Windows).
use resvg::{tiny_skia, usvg};

use std::sync::{Arc, OnceLock};

/// Las fuentes del sistema se cargan una sola vez (es lo más lento de la exportación).
fn fuentes() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

fn opciones() -> usvg::Options<'static> {
    let mut o = usvg::Options::default();
    o.fontdb = fuentes();
    o
}

/// PNG con `escala` veces el tamaño del SVG (3.0 ≈ 300 ppp para 100 ppp base).
pub fn svg_a_png(svg: &str, escala: f32) -> Result<Vec<u8>, String> {
    let arbol = usvg::Tree::from_str(svg, &opciones()).map_err(|e| format!("SVG no válido: {e}"))?;
    let tam = arbol.size();
    let (w, h) = ((tam.width() * escala).ceil() as u32, (tam.height() * escala).ceil() as u32);
    let mut pix = tiny_skia::Pixmap::new(w, h).ok_or("Tamaño de imagen no válido")?;
    resvg::render(&arbol, tiny_skia::Transform::from_scale(escala, escala), &mut pix.as_mut());
    pix.encode_png().map_err(|e| e.to_string())
}

/// PDF vectorial de una página.
pub fn svg_a_pdf(svg: &str) -> Result<Vec<u8>, String> {
    static DB: OnceLock<Arc<svg2pdf::usvg::fontdb::Database>> = OnceLock::new();
    let db = DB.get_or_init(|| {
        let mut db = svg2pdf::usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    });
    let mut o = svg2pdf::usvg::Options::default();
    o.fontdb = db.clone();
    let arbol = svg2pdf::usvg::Tree::from_str(svg, &o).map_err(|e| format!("SVG no válido: {e}"))?;
    svg2pdf::to_pdf(&arbol, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions::default()).map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ejes, Figura, Marca, Serie, a_svg, ticks_lineales};

    fn ejemplo() -> Figura {
        Figura {
            nombre: "prueba".into(),
            titulo: "Prueba de figura".into(),
            series: vec![Serie { nombre: "Serie A".into(), puntos: (0..20).map(|i| (i as f64, (i as f64 / 3.0).sin().abs())).collect(), color: 0xd55e00, grosor: 2.2, relleno: true, opaca: true }],
            marcas: vec![Marca { x: 10.0, y: 0.5, color: 0x0e7c86 }],
            mapa: None,
            ejes: Ejes { x: (0.0, 20.0), y: (0.0, 1.0), y_log: false, x_log: false, x_ticks: ticks_lineales(0.0, 20.0, 5), y_ticks: ticks_lineales(0.0, 1.0, 5), x_titulo: "Distancia (km)".into(), y_titulo: "Concentración (×10⁻⁶)".into() },
        }
    }

    #[test]
    fn genera_svg_png_y_pdf() {
        let svg = a_svg(&ejemplo(), 640.0, 340.0);
        assert!(svg.starts_with("<svg") && svg.contains("Prueba de figura") && svg.contains("polyline"));
        let png = svg_a_png(&svg, 2.0).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert!(png.len() > 2000);
        let pdf = svg_a_pdf(&svg).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn mapa_de_calor_e_informe_generan_pdf() {
        use crate::{Informe, Mapa, informe_svg};
        let (nx, ny) = (30, 12);
        let colores: Vec<u32> = (0..nx * ny).map(|k| 0x200000 + ((k * 200 / (nx * ny)) as u32) * 0x010100).collect();
        let mut f = ejemplo();
        f.mapa = Some(Mapa { nx, ny, colores, barra: (0..16).map(|k| 0x102030 + k * 0x0a0a0a).collect(), barra_max: "5.0 ×10⁻⁶".into(), barra_titulo: "Concentración".into() });
        let svg = a_svg(&f, 720.0, 380.0);
        assert!(svg.matches("<rect").count() > nx * ny);
        assert!(svg_a_png(&svg, 1.0).is_ok());
        let inf = Informe {
            titulo: "Informe de prueba".into(),
            subtitulo: "Riomat".into(),
            parametros: (0..9).map(|k| (format!("parametro {k}"), format!("valor {k}"))).collect(),
            tabla: vec![vec!["Caudal".into(), "Correlación".into(), "t50".into()], vec!["PF 1".into(), "Fischer".into(), "53.5".into()]],
            figuras: vec![ejemplo(), f],
            avisos: (0..40).map(|k| format!("aviso número {k} con un texto bastante largo para obligar a partir la línea en varias partes y comprobar que no se sale de la página")).collect(),
            alcance: "Trazador pasivo.".into(),
            pie: "pie".into(),
        };
        let pdf = svg_a_pdf(&informe_svg(&inf)).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn escapa_texto_especial() {
        let mut f = ejemplo();
        f.titulo = "A & B <C>".into();
        let svg = a_svg(&f, 600.0, 300.0);
        assert!(svg.contains("A &amp; B &lt;C&gt;"));
        assert!(svg_a_png(&svg, 1.0).is_ok());
    }
}

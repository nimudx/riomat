use riomat_plot::*;
fn main() {
    let f = Figura { nombre: "m".into(), titulo: "Masa evacuada acumulada · PF 2 · Q = 113.7 m³/s".into(),
        series: vec![
            Serie { nombre: "Fischer".into(), puntos: (0..=240).step_by(4).map(|t| (t as f64, 1.0 - (-(t as f64) / 40.0).exp())).collect(), color: 0x1f77b4, grosor: 2.0, relleno: false, opaca: true },
            Serie { nombre: "Seo y Cheong".into(), puntos: (0..=240).step_by(4).map(|t| (t as f64, 1.0 - (-(t as f64) / 60.0).exp())).collect(), color: 0xd55e00, grosor: 2.6, relleno: false, opaca: true }],
        marcas: vec![Marca { x: 60.0, y: 0.63, color: 0xd55e00 }],
        ejes: Ejes { x: (0.0, 240.0), y: (0.0, 1.0), x_log: false, y_log: false, x_ticks: ticks_lineales(0.0, 240.0, 5), y_ticks: ticks_lineales(0.0, 1.0, 5), x_titulo: "Tiempo (min)".into(), y_titulo: "Fracción evacuada".into() } };
    let svg = a_svg(&f, 720.0, 380.0);
    std::fs::write(std::env::args().nth(1).unwrap(), svg_a_png(&svg, 2.0).unwrap()).unwrap();
}

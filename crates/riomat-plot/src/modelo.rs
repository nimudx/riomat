//! Datos de una gráfica de líneas: series, marcas y ejes (lineales o logarítmicos).

#[derive(Clone, Debug)]
pub struct Serie {
    pub nombre: String,
    pub puntos: Vec<(f64, f64)>,
    pub color: u32, // 0xRRGGBB
    pub grosor: f32,
    pub relleno: bool,
    pub opaca: bool,
}

#[derive(Clone, Debug)]
pub struct Marca {
    pub x: f64,
    pub y: f64,
    pub color: u32,
}

#[derive(Clone, Debug)]
pub struct Ejes {
    pub x: (f64, f64),
    pub y: (f64, f64),
    pub y_log: bool,
    pub x_log: bool,
    pub x_ticks: Vec<f64>,
    pub y_ticks: Vec<f64>,
    pub x_titulo: String,
    pub y_titulo: String,
}

/// Una figura completa, lista para dibujar en pantalla o exportar.
#[derive(Clone, Debug)]
pub struct Figura {
    /// Nombre de archivo (sin extensión).
    pub nombre: String,
    pub titulo: String,
    pub series: Vec<Serie>,
    pub marcas: Vec<Marca>,
    pub ejes: Ejes,
}

/// Formato compacto de un número de eje.
pub fn num(v: f64) -> String {
    let a = v.abs();
    if a == 0.0 {
        "0".into()
    } else if (v - v.round()).abs() < 1e-9 && a < 100000.0 {
        format!("{v:.0}")
    } else if !(0.01..100000.0).contains(&a) {
        format!("{v:.0e}")
    } else if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

pub fn ticks_lineales(min: f64, max: f64, n: usize) -> Vec<f64> {
    (0..n).map(|i| min + (max - min) * i as f64 / (n - 1) as f64).collect()
}

/// Valor "bonito" por encima de `v` (1, 2, 5 × 10^k).
pub fn techo_bonito(v: f64) -> f64 {
    if v <= 0.0 {
        return 1.0;
    }
    let e = v.log10().floor();
    let base = 10f64.powf(e);
    let m = v / base;
    let nice = if m <= 1.0 { 1.0 } else if m <= 2.0 { 2.0 } else if m <= 5.0 { 5.0 } else { 10.0 };
    nice * base
}

/// Posición normalizada (0..1) de un valor en un eje.
pub fn tr(v: f64, (lo, hi): (f64, f64), log: bool) -> f64 {
    if log {
        (v.max(1e-300).ln() - lo.max(1e-300).ln()) / (hi.ln() - lo.max(1e-300).ln())
    } else {
        (v - lo) / (hi - lo)
    }
}

//! Coeficiente de dispersión longitudinal D_L por tres correlaciones empíricas.
//! Aproximaciones exploratorias: no calibradas para microplásticos.

pub const G: f64 = 9.81;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Metodo {
    Fischer,
    SeoCheong,
    KashefipourFalconer,
}

pub const METODOS: [Metodo; 3] = [Metodo::Fischer, Metodo::SeoCheong, Metodo::KashefipourFalconer];

impl Metodo {
    pub fn nombre(self) -> &'static str {
        match self {
            Metodo::Fischer => "Fischer",
            Metodo::SeoCheong => "Seo y Cheong",
            Metodo::KashefipourFalconer => "Kashefipour y Falconer",
        }
    }
    /// Nombre usado en los CSV de referencia de Python.
    pub fn clave(self) -> &'static str {
        match self {
            Metodo::Fischer => "Fischer",
            Metodo::SeoCheong => "Seo-Cheong",
            Metodo::KashefipourFalconer => "Kashefipour-Falconer",
        }
    }
}

/// D_L (m²/s) de un solo punto. `u`=Q/A, `a`=área, `b`=ancho superficial,
/// `r`=radio hidráulico aproximado, `sf`=pendiente de energía.
pub fn d_l(metodo: Metodo, u: f64, a: f64, b: f64, r: f64, sf: f64) -> f64 {
    let h = a / b;
    let us = (G * r * sf).sqrt();
    match metodo {
        Metodo::Fischer => 0.011 * u * u * b * b / (h * us),
        Metodo::SeoCheong => 5.915 * h * us * (b / h).powf(0.620) * (u / us).powf(1.428),
        Metodo::KashefipourFalconer => {
            (7.428 + 1.775 * (b / h).powf(0.620) * (us / u).powf(0.572)) * h * u * u / us
        }
    }
}

/// D_L en todas las secciones de un perfil.
pub fn coeficientes(metodo: Metodo, u: &[f64], a: &[f64], b: &[f64], r: &[f64], sf: &[f64]) -> Vec<f64> {
    (0..u.len()).map(|i| d_l(metodo, u[i], a[i], b[i], r[i], sf[i])).collect()
}

/// Velocidad de corte u* (m/s).
pub fn u_estrella(r: f64, sf: f64) -> f64 {
    (G * r * sf).sqrt()
}

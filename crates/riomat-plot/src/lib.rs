//! Modelo de gráficas de Riomat y su exportación (SVG, PNG, PDF).
//! No depende de la interfaz: la pantalla y los archivos usan los mismos datos.
pub mod export;
pub mod modelo;
pub mod svg;

pub use export::{svg_a_pdf, svg_a_png};
pub use modelo::*;
pub use svg::a_svg;

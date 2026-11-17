//! Identidad visual de Riomat (0xRRGGBB): piedra andina, agua de río y trazador como tinta.
pub const BG: u32 = 0xf1eee8; // niebla de piedra
pub const PANEL: u32 = 0xfdfcfa;
pub const HEADER: u32 = 0xe8e3d9;
pub const BORDER: u32 = 0xd8d2c5;
pub const HOVER: u32 = 0xeee9df;
pub const TEXT: u32 = 0x22272c;
pub const MUTED: u32 = 0x66655f;
pub const DIM: u32 = 0x9b988d;
pub const ACCENT: u32 = 0x0e7c86; // agua profunda
pub const ACCENT_SOFT: u32 = 0x0e7c861f;
pub const RUN: u32 = 0x0e7c86;
pub const RESET: u32 = 0x6a6e75;
pub const OK: u32 = 0x2f8f5b;
pub const WARN: u32 = 0xc9801a;
pub const ERR: u32 = 0xc4432f;
pub const GRID: u32 = 0xebe7de;
pub const PLOT_BG: u32 = 0xfcfbf8;
// Río
pub const AGUA: u32 = 0xcfe5e8;
pub const AGUA_LINEA: u32 = 0x0e7c86;
pub const ROCA: u32 = 0xc4b9a4;
pub const ROCA_LINEA: u32 = 0x7a6d58;
pub const ENERGIA: u32 = 0x8a4f7d;
// Trazador: de agua clara a ámbar y bermellón
pub const TRAZ_MEDIO: u32 = 0xf0b13e;
pub const TRAZ_ALTO: u32 = 0xc2411b;
// Correlaciones
pub const C_FISCHER: u32 = 0x0072b2;
pub const C_SEO: u32 = 0xd55e00;
pub const C_KF: u32 = 0x009e73;

/// Escala global de la interfaz. En Windows los elementos se ven demasiado grandes con el mismo diseño, así que se reduce.
pub const ESCALA: f32 = if cfg!(target_os = "windows") { 0.8 } else { 1.0 };

/// `px` con la escala global aplicada: todas las medidas de la interfaz pasan por aquí.
pub fn px(v: f32) -> gpui_kit::Pixels {
    gpui_kit::px(v * ESCALA)
}

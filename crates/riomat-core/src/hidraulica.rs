//! Lectura y validación de los dos Excel exportados de HEC-RAS.
use calamine::{Data, Reader, Xlsx, open_workbook};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub enum Nivel {
    Info,
    Aviso,
    Error,
}

#[derive(Clone, Debug)]
pub struct Aviso {
    pub nivel: Nivel,
    pub mensaje: String,
}

/// Una fila: una sección en un perfil.
#[derive(Clone, Debug)]
pub struct Registro {
    pub seccion: f64,
    pub perfil: String,
    pub q: f64,
    pub p_total: f64,
    pub a: f64,
    pub a_canal_act: f64,
    pub froude_canal: f64,
    pub sf_friccion: Option<f64>,
    pub froude_total: f64,
    pub sf: f64,
    pub u_canal: f64,
    pub p_canal: f64,
    pub a_canal: f64,
    // Del primer Excel
    pub q_original: f64,
    pub sf_original: f64,
    pub a_original: f64,
    pub b: f64,
    // Para el perfil longitudinal (si existen en el Excel de radio hidráulico)
    pub cota_fondo: Option<f64>,
    pub nivel_agua: Option<f64>,
    pub nivel_critico: Option<f64>,
    pub cota_energia: Option<f64>,
}

impl Registro {
    pub fn r_total_aprox(&self) -> f64 {
        self.a / self.p_total
    }
}

/// Un perfil de caudal ordenado de aguas arriba a aguas abajo.
#[derive(Clone, Debug)]
pub struct Perfil {
    pub nombre: String,
    pub q: f64,
    /// Distancia desde la primera sección (m), creciente hacia aguas abajo.
    pub x: Vec<f64>,
    pub secciones: Vec<f64>,
    pub a: Vec<f64>,
    pub b: Vec<f64>,
    pub sf: Vec<f64>,
    pub r: Vec<f64>,
    pub u: Vec<f64>,
    pub registros: Vec<Registro>,
}

#[derive(Clone, Debug)]
pub struct Hidraulica {
    pub registros: Vec<Registro>,
    pub perfiles: Vec<Perfil>,
    pub avisos: Vec<Aviso>,
}

fn num(d: &Data) -> Option<f64> {
    match d {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        _ => None,
    }
}

fn texto(d: &Data) -> String {
    match d {
        Data::String(s) => s.trim().to_string(),
        other => other.to_string(),
    }
}

fn req(fila: &[Data], i: usize, que: &str) -> Result<f64, String> {
    fila.get(i).and_then(num).ok_or_else(|| format!("Falta el valor numérico de '{que}' (columna {})", i + 1))
}

fn es_perfil(fila: &[Data]) -> bool {
    fila.get(1).and_then(num).is_some() && fila.get(2).map(texto).is_some_and(|t| t.starts_with("PF "))
}

/// Lee los dos Excel, los une por (sección, perfil) y valida su consistencia.
pub fn leer_hidraulica(excel_tablas: &Path, excel_radio: &Path) -> Result<Hidraulica, String> {
    for p in [excel_tablas, excel_radio] {
        if !p.exists() {
            return Err(format!("Falta archivo: {}", p.display()));
        }
    }
    let mut wb1: Xlsx<_> = open_workbook(excel_tablas).map_err(|e| format!("{}: {e}", excel_tablas.display()))?;
    let hoja1 = wb1.worksheet_range("Datos originales").map_err(|e| format!("Hoja 'Datos originales': {e}"))?;
    let mut original: HashMap<(i64, String), [f64; 4]> = HashMap::new();
    for fila in hoja1.rows().skip(1) {
        if !es_perfil(fila) {
            continue;
        }
        let clave = ((num(&fila[1]).unwrap() * 1000.0).round() as i64, texto(&fila[2]));
        original.insert(
            clave,
            [req(fila, 3, "Q")?, req(fila, 8, "Pend. energía")?, req(fila, 10, "Área")?, req(fila, 11, "Ancho")?],
        );
    }
    let mut wb2: Xlsx<_> = open_workbook(excel_radio).map_err(|e| format!("{}: {e}", excel_radio.display()))?;
    let hoja2 = wb2
        .worksheet_range_at(0)
        .ok_or("El Excel de radio hidráulico no tiene hojas")?
        .map_err(|e| e.to_string())?;
    let mut registros = Vec::new();
    for fila in hoja2.rows().skip(2) {
        if !es_perfil(fila) {
            continue;
        }
        let seccion = num(&fila[1]).unwrap();
        let perfil = texto(&fila[2]);
        let clave = ((seccion * 1000.0).round() as i64, perfil.clone());
        let o = original
            .get(&clave)
            .ok_or_else(|| format!("Sección/perfil ausente en el primer Excel: {seccion} {perfil}"))?;
        let opt = |i: usize| fila.get(i).and_then(num);
        registros.push(Registro {
            seccion,
            perfil,
            q: req(fila, 3, "Q Total")?,
            p_total: req(fila, 4, "W.P. Total")?,
            a: req(fila, 5, "Flow Area")?,
            a_canal_act: req(fila, 6, "Flow Area Ch")?,
            froude_canal: req(fila, 7, "Froude # Chl")?,
            sf_friccion: opt(8),
            froude_total: req(fila, 9, "Froude # XS")?,
            sf: req(fila, 10, "E.G. Slope")?,
            u_canal: req(fila, 11, "Vel Chnl")?,
            p_canal: req(fila, 12, "W.P. Channel")?,
            a_canal: req(fila, 13, "Area Channel")?,
            q_original: o[0],
            sf_original: o[1],
            a_original: o[2],
            b: o[3],
            cota_fondo: opt(14),
            nivel_agua: opt(15),
            nivel_critico: opt(16),
            cota_energia: opt(17),
        });
    }
    validar(&registros)?;
    let avisos = avisos_de(&registros, "Q, área y pendiente coinciden entre ambos Excel");
    let perfiles = agrupar(&registros)?;
    Ok(Hidraulica { registros, perfiles, avisos })
}

pub(crate) fn validar(r: &[Registro]) -> Result<(), String> {
    if r.is_empty() {
        return Err("No se encontró ningún registro de perfil (PF 1, PF 2…).".into());
    }
    let mut vistos = std::collections::HashSet::new();
    for x in r {
        if !vistos.insert(((x.seccion * 1000.0).round() as i64, x.perfil.clone())) {
            return Err(format!("Registro duplicado: sección {} {}", x.seccion, x.perfil));
        }
        for (v, n) in [(x.q, "Q"), (x.p_total, "perímetro total"), (x.a, "área"), (x.a_canal, "área de canal"),
                       (x.p_canal, "perímetro de canal"), (x.sf, "pendiente"), (x.b, "ancho")] {
            if v.is_nan() && (n == "área de canal" || n == "perímetro de canal") {
                continue; // columnas opcionales que no usa el modelo
            }
            if !(v > 0.0) {
                return Err(format!("{n} debe ser positivo (sección {} {})", x.seccion, x.perfil));
            }
        }
        if (x.q - x.q_original).abs() > 1e-6 * x.q_original.abs() {
            return Err(format!("Los caudales de ambos archivos no coinciden (sección {} {})", x.seccion, x.perfil));
        }
        if (x.sf - x.sf_original).abs() > 5.1e-6 {
            return Err(format!("La pendiente energética supera la tolerancia de redondeo de 5e-6 (sección {} {})", x.seccion, x.perfil));
        }
        if (x.a - x.a_original).abs() > 1e-6 * x.a_original.abs() {
            return Err(format!("El área activa total no coincide con el área del primer Excel (sección {} {})", x.seccion, x.perfil));
        }
    }
    Ok(())
}

pub(crate) fn avisos_de(r: &[Registro], origen: &str) -> Vec<Aviso> {
    let mut v = vec![Aviso { nivel: Nivel::Info, mensaje: format!("{} registros leídos y validados ({origen}).", r.len()) }];
    let faltan: Vec<String> = r.iter().filter(|x| x.sf_friccion.is_none()).map(|x| format!("{} {}", x.seccion, x.perfil)).collect();
    if !faltan.is_empty() {
        v.push(Aviso { nivel: Nivel::Aviso, mensaje: format!("Pendiente de fricción ausente en {} registros ({}). Se usa la pendiente de energía.", faltan.len(), faltan.join(", ")) });
    }
    let difieren = r.iter().filter(|x| x.sf_friccion.is_some_and(|f| (x.sf - f).abs() > 1e-8)).count();
    if difieren > 0 {
        v.push(Aviso { nivel: Nivel::Aviso, mensaje: format!("La pendiente de energía difiere de la de fricción en {difieren} registros: no son intercambiables.") });
    }
    v.push(Aviso { nivel: Nivel::Aviso, mensaje: "R = área / perímetro total es una aproximación; el perímetro puede incluir zonas inefectivas.".into() });
    v.push(Aviso { nivel: Nivel::Aviso, mensaje: "El ancho B es el ancho superficial exportado; puede incluir zonas inefectivas y afecta a D_L.".into() });
    v
}

pub(crate) fn agrupar(r: &[Registro]) -> Result<Vec<Perfil>, String> {
    let mut nombres: Vec<String> = Vec::new();
    for x in r {
        if !nombres.contains(&x.perfil) {
            nombres.push(x.perfil.clone());
        }
    }
    nombres.sort();
    let mut out = Vec::new();
    for n in nombres {
        let mut regs: Vec<Registro> = r.iter().filter(|x| x.perfil == n).cloned().collect();
        regs.sort_by(|a, b| b.seccion.partial_cmp(&a.seccion).unwrap());
        let q = regs[0].q;
        if regs.iter().any(|x| (x.q - q).abs() > 1e-8 * q.abs()) {
            return Err(format!("El caudal cambia entre secciones en {n}."));
        }
        let s0 = regs[0].seccion;
        out.push(Perfil {
            nombre: n,
            q,
            x: regs.iter().map(|x| s0 - x.seccion).collect(),
            secciones: regs.iter().map(|x| x.seccion).collect(),
            a: regs.iter().map(|x| x.a).collect(),
            b: regs.iter().map(|x| x.b).collect(),
            sf: regs.iter().map(|x| x.sf).collect(),
            r: regs.iter().map(|x| x.r_total_aprox()).collect(),
            u: regs.iter().map(|x| q / x.a).collect(),
            registros: regs,
        });
    }
    Ok(out)
}

impl Perfil {
    pub fn longitud(&self) -> f64 {
        self.x.last().copied().unwrap_or(0.0) - self.x[0]
    }
}

/// ¿Este Excel es el de "tablas científicas" (tiene la hoja 'Datos originales')?
pub fn es_excel_tablas(ruta: &Path) -> bool {
    open_workbook::<Xlsx<_>, _>(ruta).map(|wb| wb.sheet_names().iter().any(|n| n == "Datos originales")).unwrap_or(false)
}

/// Recibe los dos Excel en cualquier orden y los identifica por su contenido.
pub fn leer_hidraulica_auto(a: &Path, b: &Path) -> Result<Hidraulica, String> {
    if es_excel_tablas(a) {
        leer_hidraulica(a, b)
    } else if es_excel_tablas(b) {
        leer_hidraulica(b, a)
    } else {
        Err("Ninguno de los dos Excel tiene la hoja 'Datos originales' (tablas científicas).".into())
    }
}

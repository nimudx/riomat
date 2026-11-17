//! Entrada directa desde una tabla de resultados por perfil de HEC-RAS
//! (texto pegado, .txt/.csv/.tsv o .xlsx de una sola tabla), sin preparar Excel a mano.
//!
//! Columnas obligatorias (nombres de HEC-RAS en inglés o equivalentes en español):
//! River Sta, Profile, Q Total, W.P. Total, Flow Area, Top Width, E.G. Slope.
//! Opcionales: Frctn Slope, Vel Chnl, W.P. Channel, Area Channel, Flow Area Ch,
//! Froude # Chl, Froude # XS, Min Ch El, W.S. Elev, Crit W.S., E.G. Elev.
use crate::hidraulica::{Aviso, Hidraulica, Nivel, Registro, agrupar, avisos_de, validar};
use calamine::{Data, Reader, Xlsx, open_workbook};
use std::path::Path;

fn norm(s: &str) -> String {
    s.trim().trim_matches('"').to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Número tolerante: acepta coma decimal, asteriscos de sección interpolada y espacios.
pub fn numero(s: &str) -> Option<f64> {
    let t = s.trim().trim_matches('"').trim_end_matches('*').trim();
    if t.is_empty() {
        return None;
    }
    let t = if t.contains(',') && !t.contains('.') { t.replace(',', ".") } else { t.replace(',', "") };
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

const SINONIMOS: &[(&str, &[&str])] = &[
    ("seccion", &["river sta", "river station", "sección", "seccion", "estación", "estacion", "station"]),
    ("perfil", &["profile", "perfil"]),
    ("q", &["q total", "q (m3/s)", "caudal", "q"]),
    ("p_total", &["w.p. total", "wp total", "perímetro mojado total", "perimetro mojado total"]),
    ("a", &["flow area", "área de flujo", "area de flujo", "área", "area"]),
    ("b", &["top width", "ancho superficial", "ancho"]),
    ("sf", &["e.g. slope", "eg slope", "pendiente de energía", "pendiente de energia", "pend. energía", "pend. energia"]),
    ("sf_fr", &["frctn slope", "pendiente de fricción", "pendiente de friccion"]),
    ("u_canal", &["vel chnl", "velocidad canal"]),
    ("p_canal", &["w.p. channel", "wp channel"]),
    ("a_canal", &["area channel", "área canal", "area canal"]),
    ("a_canal_act", &["flow area ch"]),
    ("fr_canal", &["froude # chl", "froude chl"]),
    ("fr_total", &["froude # xs", "froude xs"]),
    ("cota_fondo", &["min ch el", "cota del fondo", "cota cauce"]),
    ("nivel_agua", &["w.s. elev", "ws elev", "nivel del agua", "nivel agua"]),
    ("nivel_critico", &["crit w.s.", "crit ws", "nivel crítico", "nivel critico"]),
    ("cota_energia", &["e.g. elev", "eg elev", "cota de energía", "cota de energia", "energía (m)", "energia (m)"]),
];

fn col(cabecera: &[String], clave: &str) -> Option<usize> {
    let sin = SINONIMOS.iter().find(|(k, _)| *k == clave)?.1;
    // Coincidencia exacta primero (evita que "area" capture "area channel")
    for s in sin {
        if let Some(i) = cabecera.iter().position(|c| c == s) {
            return Some(i);
        }
    }
    None
}

fn separar(linea: &str, sep: char) -> Vec<String> {
    linea.split(sep).map(|c| c.trim().trim_matches('"').to_string()).collect()
}

fn detectar_separador(texto: &str) -> char {
    if texto.contains('\t') {
        '\t'
    } else if texto.lines().any(|l| l.matches(';').count() >= 3) {
        ';'
    } else {
        ','
    }
}

pub fn leer_tabla_texto(texto: &str) -> Result<Hidraulica, String> {
    let texto = texto.trim_start_matches('\u{feff}');
    let sep = detectar_separador(texto);
    let filas: Vec<Vec<String>> = texto.lines().filter(|l| !l.trim().is_empty()).map(|l| separar(l, sep)).collect();
    leer_filas(&filas, "tabla de HEC-RAS")
}

pub fn leer_filas(filas: &[Vec<String>], origen: &str) -> Result<Hidraulica, String> {
    let ih = filas
        .iter()
        .position(|f| {
            let n: Vec<String> = f.iter().map(|c| norm(c)).collect();
            col(&n, "seccion").is_some() && col(&n, "perfil").is_some()
        })
        .ok_or("No encontré la cabecera de la tabla: hacen falta las columnas «River Sta» y «Profile».")?;
    let cab: Vec<String> = filas[ih].iter().map(|c| norm(c)).collect();
    let falta: Vec<&str> = [("q", "Q Total"), ("p_total", "W.P. Total"), ("a", "Flow Area"), ("b", "Top Width"), ("sf", "E.G. Slope")]
        .iter()
        .filter(|(k, _)| col(&cab, k).is_none())
        .map(|(_, n)| *n)
        .collect();
    if !falta.is_empty() {
        return Err(format!("A la tabla le faltan columnas obligatorias: {}. En HEC-RAS agrégalas al definir la tabla de perfiles.", falta.join(", ")));
    }
    let c = |k: &str| col(&cab, k);
    let (i_sec, i_perf) = (c("seccion").unwrap(), c("perfil").unwrap());
    let get = |f: &[String], i: Option<usize>| i.and_then(|i| f.get(i)).and_then(|s| numero(s));
    let mut registros = Vec::new();
    let mut interpoladas = 0usize;
    for f in &filas[ih + 1..] {
        let Some(seccion) = f.get(i_sec).and_then(|s| numero(s)) else { continue }; // salta la fila de unidades
        let perfil = f.get(i_perf).map(|s| s.trim().to_string()).unwrap_or_default();
        if perfil.is_empty() {
            continue;
        }
        if f[i_sec].trim().ends_with('*') {
            interpoladas += 1;
        }
        let q = get(f, c("q")).ok_or_else(|| format!("Falta Q en la sección {seccion} {perfil}"))?;
        let a = get(f, c("a")).ok_or_else(|| format!("Falta Flow Area en la sección {seccion} {perfil}"))?;
        let sf = get(f, c("sf")).ok_or_else(|| format!("Falta E.G. Slope en la sección {seccion} {perfil}"))?;
        let b = get(f, c("b")).ok_or_else(|| format!("Falta Top Width en la sección {seccion} {perfil}"))?;
        let nan = |x: Option<f64>| x.unwrap_or(f64::NAN);
        registros.push(Registro {
            seccion,
            perfil,
            q,
            p_total: get(f, c("p_total")).ok_or_else(|| format!("Falta W.P. Total en la sección {seccion}"))?,
            a,
            a_canal_act: nan(get(f, c("a_canal_act"))),
            froude_canal: nan(get(f, c("fr_canal"))),
            sf_friccion: get(f, c("sf_fr")),
            froude_total: nan(get(f, c("fr_total"))),
            sf,
            u_canal: nan(get(f, c("u_canal"))),
            p_canal: nan(get(f, c("p_canal"))),
            a_canal: nan(get(f, c("a_canal"))),
            q_original: q,
            sf_original: sf,
            a_original: a,
            b,
            cota_fondo: get(f, c("cota_fondo")),
            nivel_agua: get(f, c("nivel_agua")),
            nivel_critico: get(f, c("nivel_critico")),
            cota_energia: get(f, c("cota_energia")),
        });
    }
    validar(&registros)?;
    let mut avisos = avisos_de(&registros, origen);
    if interpoladas > 0 {
        avisos.push(Aviso { nivel: Nivel::Aviso, mensaje: format!("{interpoladas} registros son secciones interpoladas de HEC-RAS (con *). Se usan igual que las demás.") });
    }
    let perfiles = agrupar(&registros)?;
    Ok(Hidraulica { registros, perfiles, avisos })
}

fn celda(d: &Data) -> String {
    match d {
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::String(s) => s.clone(),
        Data::Empty => String::new(),
        o => o.to_string(),
    }
}

/// Lee un archivo de tabla: .xlsx (cualquier hoja con la tabla) o texto (.txt, .csv, .tsv).
pub fn leer_tabla_archivo(ruta: &Path) -> Result<Hidraulica, String> {
    let ext = ruta.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let nombre = ruta.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if ext == "xlsx" {
        let mut wb: Xlsx<_> = open_workbook(ruta).map_err(|e| format!("{nombre}: {e}"))?;
        let mut ultimo = String::from("No se encontró ninguna hoja con la tabla.");
        for hoja in wb.sheet_names().to_vec() {
            let Ok(rango) = wb.worksheet_range(&hoja) else { continue };
            let filas: Vec<Vec<String>> = rango.rows().map(|r| r.iter().map(celda).collect()).collect();
            match leer_filas(&filas, &format!("hoja «{hoja}» de {nombre}")) {
                Ok(h) => return Ok(h),
                Err(e) => ultimo = e,
            }
        }
        return Err(ultimo);
    }
    let bytes = std::fs::read(ruta).map_err(|e| format!("{nombre}: {e}"))?;
    leer_tabla_texto(&String::from_utf8_lossy(&bytes))
}

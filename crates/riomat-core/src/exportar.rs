//! Exportación de resultados (CSV, Excel y un script de matplotlib). Sin dependencias de interfaz.
use crate::analisis::{FilaAud, FilaSens, mayores, resumen_auditoria};
use crate::{Hidraulica, METODOS, Metodo, Parametros, Resultado, coeficientes, simular_entrada};
use rust_xlsxwriter::Workbook;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Un escenario completo: caudal (perfil) × correlación.
pub struct Escenario {
    pub perfil: String,
    pub metodo: Metodo,
    pub d: Vec<f64>,
    pub res: Resultado,
}

/// Aviso de verificación ya formateado: (nivel, componente, mensaje).
pub type Verificacion = (String, String, String);

/// Contexto de la corrida mostrada en pantalla (para el campo de concentración y los parámetros).
pub struct Mostrado<'a> {
    pub perfil: &'a str,
    pub metodo: Metodo,
    pub resultado: &'a Resultado,
    pub archivos: &'a [String],
}

/// Simula los escenarios (todos los caudales × las tres correlaciones) con los parámetros dados.
pub fn escenarios(h: &Hidraulica, par: &Parametros) -> Result<Vec<Escenario>, String> {
    let par = Parametros { campo_cada: None, ..par.clone() };
    let mut v = Vec::new();
    for p in &h.perfiles {
        for m in METODOS {
            let d = coeficientes(m, &p.u, &p.a, &p.b, &p.r, &p.sf);
            let res = simular_entrada(p.q, &p.x, &p.a, &d, &par).map_err(|e| format!("{} · {}: {e}", p.nombre, m.nombre()))?;
            v.push(Escenario { perfil: p.nombre.clone(), metodo: m, d, res });
        }
    }
    Ok(v)
}

fn csv_campo(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

fn opt(v: Option<f64>) -> String {
    v.map(|x| format!("{x}")).unwrap_or_default()
}

/// Tablas en memoria: nombre de hoja/archivo, cabecera y filas de texto. Sirven para CSV y Excel.
pub struct Tabla {
    pub nombre: &'static str,
    pub cab: Vec<&'static str>,
    pub filas: Vec<Vec<String>>,
}

impl Tabla {
    fn csv(&self) -> String {
        let mut s = self.cab.join(",");
        s.push('\n');
        for f in &self.filas {
            s.push_str(&f.iter().map(|c| csv_campo(c)).collect::<Vec<_>>().join(","));
            s.push('\n');
        }
        s
    }
}

pub fn tablas(h: &Hidraulica, par: &Parametros, esc: &[Escenario], m: &Mostrado, verif: &[Verificacion]) -> Vec<Tabla> {
    let mut out = Vec::new();
    // Parámetros
    let mut p = vec![
        ("programa".to_string(), format!("Riomat {}", env!("CARGO_PKG_VERSION"))),
        ("masa_inyectada".into(), format!("{}", par.masa)),
        ("duracion_pulso_s".into(), format!("{}", par.duracion_pulso)),
        ("dx_max_m".into(), format!("{}", par.dx_max)),
        ("dt_s".into(), format!("{}", par.dt)),
        ("tiempo_final_s".into(), format!("{}", par.t_final)),
        ("escenario_mostrado".into(), format!("{} · {}", m.perfil, m.metodo.nombre())),
        ("alcance".into(), "Trazador pasivo conservativo (no microplásticos); D_L de correlaciones empíricas sin calibrar; duración del pulso = hipótesis".into()),
    ];
    for (i, a) in m.archivos.iter().enumerate() {
        p.push((format!("archivo_{}", i + 1), a.clone()));
    }
    out.push(Tabla { nombre: "parametros", cab: vec!["parametro", "valor"], filas: p.into_iter().map(|(a, b)| vec![a, b]).collect() });

    // Resumen por escenario
    out.push(Tabla {
        nombre: "resumen_escenarios",
        cab: vec!["perfil", "caudal_m3_s", "correlacion", "t50_min", "t95_min", "fraccion_evacuada_final", "error_balance", "D_min_m2_s", "D_max_m2_s", "dx_m", "dt_s"],
        filas: esc.iter().map(|e| {
            let (dmin, dmax) = (e.d.iter().cloned().fold(f64::INFINITY, f64::min), e.d.iter().cloned().fold(0.0, f64::max));
            vec![e.perfil.clone(), format!("{}", e.res.q), e.metodo.nombre().into(), opt(e.res.t50_min), opt(e.res.t95_min), format!("{}", e.res.fraccion_salida), format!("{:e}", e.res.error_balance), format!("{dmin}"), format!("{dmax}"), format!("{}", e.res.dx), format!("{}", e.res.dt)]
        }).collect(),
    });

    // Hidráulica leída
    let mut hid = Vec::new();
    for p in &h.perfiles {
        for i in 0..p.x.len() {
            hid.push(vec![p.nombre.clone(), format!("{}", p.secciones[i]), format!("{}", p.x[i]), format!("{}", p.q), format!("{}", p.a[i]), format!("{}", p.b[i]), format!("{}", p.a[i] / p.r[i]), format!("{}", p.r[i]), format!("{}", p.sf[i]), format!("{}", p.u[i])]);
        }
    }
    out.push(Tabla {
        nombre: "hidraulica",
        cab: vec!["perfil", "seccion", "distancia_m", "caudal_m3_s", "area_m2", "ancho_m", "perimetro_total_m", "radio_hidraulico_m", "pendiente_energia", "velocidad_m_s"],
        filas: hid,
    });

    // D_L por sección y correlación
    let mut dl = Vec::new();
    for e in esc {
        if let Some(p) = h.perfiles.iter().find(|p| p.nombre == e.perfil) {
            for i in 0..p.x.len() {
                dl.push(vec![e.perfil.clone(), e.metodo.nombre().into(), format!("{}", p.secciones[i]), format!("{}", p.x[i]), format!("{}", e.d[i])]);
            }
        }
    }
    out.push(Tabla { nombre: "coeficientes_dl", cab: vec!["perfil", "correlacion", "seccion", "distancia_m", "D_L_m2_s"], filas: dl });

    // Curvas de salida (formato largo)
    let mut cur = Vec::new();
    for e in esc {
        for j in 0..e.res.t.len() {
            cur.push(vec![e.perfil.clone(), format!("{}", e.res.q), e.metodo.nombre().into(), format!("{}", e.res.t[j] / 60.0), format!("{}", e.res.inyectada[j]), format!("{}", e.res.salida[j]), format!("{}", e.res.masa_dominio[j]), format!("{}", e.res.salida[j] / e.res.masa)]);
        }
    }
    out.push(Tabla { nombre: "curvas_salida", cab: vec!["perfil", "caudal_m3_s", "correlacion", "tiempo_min", "masa_inyectada", "masa_salida", "masa_en_tramo", "fraccion_evacuada"], filas: cur });

    // Campo de concentración del escenario mostrado
    let r = m.resultado;
    let mut campo = Vec::new();
    for (paso, c) in &r.campo {
        for (x, v) in r.x.iter().zip(c) {
            campo.push(vec![format!("{}", *paso as f64 * r.dt / 60.0), format!("{x}"), format!("{v}")]);
        }
    }
    out.push(Tabla { nombre: "campo_concentracion", cab: vec!["tiempo_min", "x_m", "concentracion"], filas: campo });

    // Verificación
    out.push(Tabla { nombre: "verificacion", cab: vec!["nivel", "componente", "mensaje"], filas: verif.iter().map(|(a, b, c)| vec![a.clone(), b.clone(), c.clone()]).collect() });
    out
}

fn excel(ruta: &Path, tablas: &[Tabla]) -> Result<(), String> {
    let mut wb = Workbook::new();
    for t in tablas.iter().filter(|t| !matches!(t.nombre, "campo_concentracion" | "curvas_salida")) {
        let hoja = wb.add_worksheet();
        hoja.set_name(t.nombre).map_err(|e| e.to_string())?;
        for (c, n) in t.cab.iter().enumerate() {
            hoja.write_string(0, c as u16, *n).map_err(|e| e.to_string())?;
        }
        for (r, fila) in t.filas.iter().enumerate() {
            for (c, v) in fila.iter().enumerate() {
                match v.parse::<f64>() {
                    Ok(x) if v.chars().next().is_some_and(|ch| ch.is_ascii_digit() || ch == '-') => hoja.write_number(r as u32 + 1, c as u16, x),
                    _ => hoja.write_string(r as u32 + 1, c as u16, v),
                }
                .map_err(|e| e.to_string())?;
            }
        }
        hoja.set_column_range_width(0, t.cab.len() as u16 - 1, 18).map_err(|e| e.to_string())?;
    }
    // Curvas: una columna por escenario (todas comparten el tiempo)
    let curvas_cols: Vec<(String, Vec<f64>)> = {
        let t = tablas.iter().find(|t| t.nombre == "curvas_salida");
        let mut mapa: Vec<(String, Vec<f64>)> = Vec::new();
        if let Some(t) = t {
            for f in &t.filas {
                let clave = format!("{} · {}", f[0], f[2]);
                let v = f[7].parse().unwrap_or(f64::NAN);
                match mapa.iter_mut().find(|(k, _)| *k == clave) {
                    Some((_, vs)) => vs.push(v),
                    None => mapa.push((clave, vec![v])),
                }
            }
        }
        mapa
    };
    if let (Some(t), false) = (tablas.iter().find(|t| t.nombre == "curvas_salida"), curvas_cols.is_empty()) {
        let hoja = wb.add_worksheet();
        hoja.set_name("curvas_fraccion_evacuada").map_err(|e| e.to_string())?;
        hoja.write_string(0, 0, "tiempo_min").map_err(|e| e.to_string())?;
        let n = curvas_cols[0].1.len();
        for (i, f) in t.filas.iter().take(n).enumerate() {
            hoja.write_number(i as u32 + 1, 0, f[3].parse::<f64>().unwrap_or(0.0)).map_err(|e| e.to_string())?;
        }
        for (c, (nombre, vs)) in curvas_cols.iter().enumerate() {
            hoja.write_string(0, c as u16 + 1, nombre).map_err(|e| e.to_string())?;
            for (i, v) in vs.iter().enumerate() {
                hoja.write_number(i as u32 + 1, c as u16 + 1, *v).map_err(|e| e.to_string())?;
            }
        }
        hoja.set_column_range_width(0, curvas_cols.len() as u16, 24).map_err(|e| e.to_string())?;
    }
    wb.save(ruta).map_err(|e| format!("{}: {e}", ruta.display()))
}

/// Tablas del análisis: sensibilidad (45 ensayos), auditoría de D_L (108 valores), resumen (9) y los 12 mayores.
pub fn tablas_analisis(sens: &[FilaSens], aud: &[FilaAud]) -> Vec<Tabla> {
    let f = |x: f64| format!("{x}");
    let fila_aud = |a: &FilaAud| vec![a.perfil.clone(), f(a.seccion), f(a.distancia), a.metodo.nombre().into(), f(a.q), f(a.a), f(a.b), f(a.p), f(a.sf), f(a.u), f(a.h), f(a.r), f(a.u_est), f(a.b_sobre_h), f(a.u_sobre_ust), f(a.d), (a.critica as u8).to_string()];
    let cab_aud = vec!["perfil", "seccion", "distancia_m", "correlacion", "caudal_m3_s", "area_m2", "ancho_m", "perimetro_m", "pendiente_energia", "U_m_s", "H_m", "R_m", "u_estrella_m_s", "B_sobre_H", "U_sobre_u_estrella", "D_L_m2_s", "seccion_critica"];
    vec![
        Tabla {
            nombre: "sensibilidad_ensayos",
            cab: vec!["perfil", "caudal_m3_s", "correlacion", "ensayo", "t50_min", "t95_min", "delta_t50_pct", "delta_t95_pct", "fraccion_evacuada_final", "error_balance", "D_min_m2_s", "D_max_m2_s"],
            filas: sens.iter().map(|x| vec![x.perfil.clone(), f(x.q), x.metodo.nombre().into(), x.ensayo.clave(), opt(x.t50_min), opt(x.t95_min), opt(x.d_t50_pct), opt(x.d_t95_pct), f(x.fraccion_salida), format!("{:e}", x.error_balance), f(x.d_min), f(x.d_max)]).collect(),
        },
        Tabla { nombre: "auditoria_dl", cab: cab_aud.clone(), filas: aud.iter().map(fila_aud).collect() },
        Tabla {
            nombre: "auditoria_resumen",
            cab: vec!["perfil", "correlacion", "D_min_m2_s", "D_mediana_m2_s", "D_max_m2_s", "cociente_max_min"],
            filas: resumen_auditoria(aud).iter().map(|r| vec![r.perfil.clone(), r.metodo.nombre().into(), f(r.d_min), f(r.d_mediana), f(r.d_max), f(r.ratio)]).collect(),
        },
        Tabla { nombre: "auditoria_12_mayores", cab: cab_aud, filas: mayores(aud, 12).iter().map(fila_aud).collect() },
    ]
}

/// Escribe en `dir`: CSV de cada tabla, `riomat_resultados.xlsx` y `graficar_matplotlib.py`. Devuelve los archivos creados.
pub fn escribir(dir: &Path, h: &Hidraulica, par: &Parametros, esc: &[Escenario], m: &Mostrado, verif: &[Verificacion], extra: Vec<Tabla>) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut ts = tablas(h, par, esc, m, verif);
    ts.extend(extra);
    let mut creados = Vec::new();
    for t in &ts {
        let ruta = dir.join(format!("{}.csv", t.nombre));
        std::fs::write(&ruta, t.csv()).map_err(|e| format!("{}: {e}", ruta.display()))?;
        creados.push(ruta);
    }
    let x = dir.join("riomat_resultados.xlsx");
    excel(&x, &ts)?;
    creados.push(x);
    let py = dir.join("graficar_matplotlib.py");
    std::fs::write(&py, include_str!("graficar_matplotlib.py")).map_err(|e| e.to_string())?;
    creados.push(py);
    let mut leeme = String::new();
    let _ = writeln!(leeme, "Exportación de Riomat {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(leeme, "- *.csv: tablas en UTF-8, separador coma, punto decimal.");
    let _ = writeln!(leeme, "- riomat_resultados.xlsx: las mismas tablas en hojas de Excel.");
    let _ = writeln!(leeme, "- figuras/: las cuatro gráficas mostradas (PNG a 300 ppp, SVG y PDF).");
    let _ = writeln!(leeme, "- graficar_matplotlib.py: figuras de publicación con matplotlib (pandas + matplotlib), leyendo estos CSV.");
    let _ = writeln!(leeme, "Alcance: trazador pasivo; D_L sin calibrar; la duración del pulso es una hipótesis.");
    let l = dir.join("LEEME.txt");
    std::fs::write(&l, leeme).map_err(|e| e.to_string())?;
    creados.push(l);
    Ok(creados)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leer_tabla_texto;

    fn hid() -> Hidraulica {
        let mut t = String::from("River Sta\tProfile\tQ Total\tW.P. Total\tFlow Area\tTop Width\tE.G. Slope\n");
        for (s, a) in [(300.0, 60.0), (200.0, 70.0), (100.0, 55.0), (0.0, 65.0)] {
            t.push_str(&format!("{s}\tPF 1\t50\t40\t{a}\t38\t0.001\n"));
        }
        leer_tabla_texto(&t).unwrap()
    }

    #[test]
    fn escribe_csv_excel_y_script() {
        let h = hid();
        let par = Parametros { campo_cada: Some(8), ..Default::default() };
        let esc = escenarios(&h, &par).unwrap();
        assert_eq!(esc.len(), 3);
        let con_campo = simular_entrada(h.perfiles[0].q, &h.perfiles[0].x, &h.perfiles[0].a, &esc[0].d, &par).unwrap();
        let archivos = vec!["tabla.txt".to_string()];
        let m = Mostrado { perfil: "PF 1", metodo: METODOS[0], resultado: &con_campo, archivos: &archivos };
        let dir = std::env::temp_dir().join(format!("riomat-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let creados = escribir(&dir, &h, &par, &esc, &m, &[("Aviso".into(), "Prueba".into(), "con, coma".into())], vec![]).unwrap();
        assert!(creados.len() >= 9);
        let resumen = std::fs::read_to_string(dir.join("resumen_escenarios.csv")).unwrap();
        assert_eq!(resumen.lines().count(), 1 + 3);
        let curvas = std::fs::read_to_string(dir.join("curvas_salida.csv")).unwrap();
        assert_eq!(curvas.lines().count(), 1 + 3 * esc[0].res.t.len());
        assert!(std::fs::read_to_string(dir.join("verificacion.csv")).unwrap().contains("\"con, coma\""));
        let x = std::fs::read(dir.join("riomat_resultados.xlsx")).unwrap();
        assert_eq!(&x[..2], b"PK");
        assert!(std::fs::read_to_string(dir.join("graficar_matplotlib.py")).unwrap().contains("curvas_salida.csv"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Valida el núcleo en Rust contra los resultados del modelo en Python.
use approx::assert_relative_eq;
use riomat_core::*;
use std::path::PathBuf;

/// Carpeta con los datos de referencia (no se versiona). Se busca en `RIOMAT_REFERENCIA` y en `./referencia`.
fn ref_dir() -> Option<PathBuf> {
    let cand = [std::env::var_os("RIOMAT_REFERENCIA").map(PathBuf::from), Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../referencia"))];
    cand.into_iter().flatten().find(|d| d.join("DATOS RADIO HIDRAULICO.xlsx").exists())
}

/// Si faltan los datos de referencia, el test se omite con un aviso.
macro_rules! datos {
    () => {
        match ref_dir() {
            Some(d) => d,
            None => {
                eprintln!("OMITIDO: no hay carpeta de referencia (define RIOMAT_REFERENCIA)");
                return;
            }
        }
    };
}

fn hidraulica(d: &std::path::Path) -> Hidraulica {
    leer_hidraulica(&d.join("Puente_Brena_Tablas_Cientificas_1D.xlsx"), &d.join("DATOS RADIO HIDRAULICO.xlsx")).unwrap()
}

fn correr(h: &Hidraulica, perfil: &str, m: Metodo, dur: f64) -> (Vec<f64>, Resultado) {
    let p = h.perfiles.iter().find(|p| p.nombre == perfil).unwrap();
    let d = coeficientes(m, &p.u, &p.a, &p.b, &p.r, &p.sf);
    let par = Parametros { duracion_pulso: dur, ..Default::default() };
    let r = simular_entrada(p.q, &p.x, &p.a, &d, &par).unwrap();
    (d, r)
}

#[test]
fn lee_36_registros() {
    let d = datos!();
    let h = hidraulica(&d);
    assert_eq!(h.registros.len(), 36);
    assert_eq!(h.perfiles.len(), 3);
    assert!(h.perfiles.iter().all(|p| p.registros.len() == 12));
    assert_relative_eq!(h.perfiles[0].longitud(), 2524.0);
}

#[test]
fn nueve_escenarios_coinciden_con_python() {
    let d = datos!();
    let h = hidraulica(&d);
    let mut rdr = csv::Reader::from_path(d.join("resultados_nueve_escenarios_entrada.csv")).unwrap();
    let mut n = 0;
    for rec in rdr.records() {
        let rec = rec.unwrap();
        let perfil = rec[0].trim();
        let metodo = METODOS.iter().find(|m| m.clave() == &rec[2]).unwrap();
        let (d, r) = correr(&h, perfil, *metodo, 300.0);
        let f = |i: usize| rec[i].parse::<f64>().unwrap();
        assert_relative_eq!(r.t50_min.unwrap(), f(5), max_relative = 1e-9);
        assert_relative_eq!(r.t95_min.unwrap(), f(6), max_relative = 1e-9);
        assert_relative_eq!(r.fraccion_salida, f(7), max_relative = 1e-9);
        assert_relative_eq!(d.iter().cloned().fold(f64::INFINITY, f64::min), f(10), max_relative = 1e-9);
        assert_relative_eq!(d.iter().cloned().fold(0.0, f64::max), f(11), max_relative = 1e-9);
        assert!(r.error_balance < 1e-11, "balance {}", r.error_balance);
        n += 1;
    }
    assert_eq!(n, 9);
}

#[test]
fn sensibilidad_duracion_coincide_con_python() {
    let d = datos!();
    let h = hidraulica(&d);
    let mut rdr = csv::Reader::from_path(d.join("sensibilidad_duracion_pulso.csv")).unwrap();
    let mut n = 0;
    for rec in rdr.records() {
        let rec = rec.unwrap();
        let metodo = METODOS.iter().find(|m| m.clave() == &rec[2]).unwrap();
        let dur: f64 = rec[3].parse().unwrap();
        let (_, r) = correr(&h, rec[0].trim(), *metodo, dur);
        assert_relative_eq!(r.t50_min.unwrap(), rec[4].parse::<f64>().unwrap(), max_relative = 1e-9);
        assert_relative_eq!(r.t95_min.unwrap(), rec[5].parse::<f64>().unwrap(), max_relative = 1e-9);
        n += 1;
    }
    assert_eq!(n, 36);
}

/// Una tabla de HEC-RAS en texto (con Top Width) debe dar exactamente los mismos resultados que los dos Excel.
#[test]
fn tabla_hecras_pegada_equivale_a_los_dos_excel() {
    let d = datos!();
    let h = hidraulica(&d);
    let mut t = String::from("Reach\tRiver Sta\tProfile\tQ Total\tW.P. Total\tFlow Area\tTop Width\tE.G. Slope\tFrctn Slope\tMin Ch El\tW.S. Elev\n");
    t.push_str("\t\t\t(m3/s)\t(m)\t(m2)\t(m)\t(m/m)\t(m/m)\t(m)\t(m)\n"); // fila de unidades
    for r in &h.registros {
        t.push_str(&format!(
            "RIO MANTARO\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.seccion, r.perfil, r.q, r.p_total, r.a, r.b, r.sf,
            r.sf_friccion.map(|v| v.to_string()).unwrap_or_default(),
            r.cota_fondo.unwrap_or(0.0), r.nivel_agua.unwrap_or(0.0)
        ));
    }
    let h2 = leer_tabla_texto(&t).unwrap();
    assert_eq!(h2.registros.len(), 36);
    for (pa, pb) in h.perfiles.iter().zip(&h2.perfiles) {
        let d = coeficientes(Metodo::SeoCheong, &pa.u, &pa.a, &pa.b, &pa.r, &pa.sf);
        let d2 = coeficientes(Metodo::SeoCheong, &pb.u, &pb.a, &pb.b, &pb.r, &pb.sf);
        let par = Parametros::default();
        let r1 = simular_entrada(pa.q, &pa.x, &pa.a, &d, &par).unwrap();
        let r2 = simular_entrada(pb.q, &pb.x, &pb.a, &d2, &par).unwrap();
        assert_relative_eq!(r1.t50_min.unwrap(), r2.t50_min.unwrap(), max_relative = 1e-12);
    }
}

#[test]
fn tabla_con_coma_decimal_y_secciones_interpoladas() {
    let t = "River Sta;Profile;Q Total;W.P. Total;Flow Area;Top Width;E.G. Slope\n\
             100;PF 1;10,5;12,0;20,0;10,0;0,001\n\
             50*;PF 1;10,5;12,5;21,0;10,5;0,0011\n";
    let h = leer_tabla_texto(t).unwrap();
    assert_eq!(h.registros.len(), 2);
    assert!(h.avisos.iter().any(|a| a.mensaje.contains("interpoladas")));
    let falta = leer_tabla_texto("River Sta\tProfile\tQ Total\n1\tPF 1\t5\n");
    assert!(falta.unwrap_err().contains("Top Width"));
}

/// Exporta con los datos reales; si `RIOMAT_EXPORT_DIR` está definida, deja allí los archivos para inspeccionarlos.
#[test]
fn exportar_todo_con_datos_reales() {
    let d = datos!();
    let h = hidraulica(&d);
    let par = Parametros { campo_cada: Some(8), ..Default::default() };
    let esc = exportar::escenarios(&h, &par).unwrap();
    assert_eq!(esc.len(), 9);
    let p = &h.perfiles[1];
    let dl = coeficientes(Metodo::SeoCheong, &p.u, &p.a, &p.b, &p.r, &p.sf);
    let res = simular_entrada(p.q, &p.x, &p.a, &dl, &par).unwrap();
    let archivos = vec!["a.xlsx".to_string(), "b.xlsx".to_string()];
    let m = exportar::Mostrado { perfil: &p.nombre, metodo: Metodo::SeoCheong, resultado: &res, archivos: &archivos };
    let dir = std::env::var_os("RIOMAT_EXPORT_DIR").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("riomat-real-{}", std::process::id())));
    exportar::escribir(&dir, &h, &par, &esc, &m, &[], vec![]).unwrap();
    let resumen = std::fs::read_to_string(dir.join("resumen_escenarios.csv")).unwrap();
    assert_eq!(resumen.lines().count(), 10);
    if std::env::var_os("RIOMAT_EXPORT_DIR").is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn sensibilidad_45_coincide_con_python() {
    let d = datos!();
    let ruta = d.join("sensibilidad_45_python.csv");
    if !ruta.exists() {
        eprintln!("OMITIDO: falta sensibilidad_45_python.csv");
        return;
    }
    let h = hidraulica(&d);
    let crit = analisis::criticas_por_defecto(&h);
    assert_eq!(crit, vec![2175.0, 1221.0]);
    let filas = analisis::sensibilidad(&h, &Parametros::default(), &analisis::ensayos(2.0, 10.0), &crit).unwrap();
    let mut rdr = csv::Reader::from_path(ruta).unwrap();
    let mut n = 0;
    for rec in rdr.records() {
        let rec = rec.unwrap();
        let f = filas.iter().find(|f| f.perfil == rec[0].trim() && f.metodo.clave() == &rec[1] && f.ensayo.clave() == &rec[2]).unwrap_or_else(|| panic!("sin fila para {:?}", rec));
        assert_relative_eq!(f.t50_min.unwrap(), rec[3].parse::<f64>().unwrap(), max_relative = 1e-9);
        assert_relative_eq!(f.t95_min.unwrap(), rec[4].parse::<f64>().unwrap(), max_relative = 1e-9);
        n += 1;
    }
    assert_eq!(n, 45);
}

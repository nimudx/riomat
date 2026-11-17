//! Análisis de sensibilidad (ensayos sobre D_L y el área de secciones críticas) y auditoría de D_L.
//! Reproduce `analisis_sensibilidad_45_escenarios` y `auditoria_coeficientes` del código Python archivado.
use crate::dispersion::{d_l, u_estrella};
use crate::{Hidraulica, METODOS, Metodo, Parametros, simular_entrada};

/// Un ensayo: multiplicador de D_L (en todas las secciones) o del área (solo en las secciones críticas).
#[derive(Clone, Debug, PartialEq)]
pub enum Ensayo {
    Referencia,
    Dispersion(f64),
    AreaCritica(f64),
}

impl Ensayo {
    pub fn etiqueta(&self) -> String {
        match self {
            Ensayo::Referencia => "referencia".into(),
            Ensayo::Dispersion(k) => format!("D_L ×{}", fmt(*k)),
            Ensayo::AreaCritica(k) => format!("área críticas ×{}", fmt(*k)),
        }
    }
    /// Nombre estable para archivos (mismo estilo que el código Python).
    pub fn clave(&self) -> String {
        match self {
            Ensayo::Referencia => "referencia".into(),
            Ensayo::Dispersion(k) => format!("dispersion_{:.0}pct", k * 100.0),
            Ensayo::AreaCritica(k) => format!("area_critica_{:.0}pct", k * 100.0),
        }
    }
}

fn fmt(k: f64) -> String {
    if (k - k.round()).abs() < 1e-9 { format!("{k:.0}") } else { format!("{k}") }
}

/// Ensayos estándar: referencia, D_L ×1/kd y ×kd, área de críticas ×(1−p) y ×(1+p).
pub fn ensayos(kd: f64, pct_area: f64) -> Vec<Ensayo> {
    let p = pct_area / 100.0;
    vec![Ensayo::Referencia, Ensayo::Dispersion(1.0 / kd), Ensayo::Dispersion(kd), Ensayo::AreaCritica(1.0 - p), Ensayo::AreaCritica(1.0 + p)]
}

/// Secciones críticas por defecto: 2175 y 1221 si existen (las de las alertas del estudio); si no, las dos de menor área.
pub fn criticas_por_defecto(h: &Hidraulica) -> Vec<f64> {
    let Some(p) = h.perfiles.first() else { return vec![] };
    let del_estudio: Vec<f64> = p.secciones.iter().cloned().filter(|s| *s == 2175.0 || *s == 1221.0).collect();
    if del_estudio.len() == 2 {
        return del_estudio;
    }
    let mut idx: Vec<usize> = (0..p.a.len()).collect();
    idx.sort_by(|&i, &j| p.a[i].partial_cmp(&p.a[j]).unwrap());
    idx.into_iter().take(2).map(|i| p.secciones[i]).collect()
}

#[derive(Clone, Debug)]
pub struct FilaSens {
    pub perfil: String,
    pub q: f64,
    pub metodo: Metodo,
    pub ensayo: Ensayo,
    pub t50_min: Option<f64>,
    pub t95_min: Option<f64>,
    pub fraccion_salida: f64,
    pub error_balance: f64,
    pub d_min: f64,
    pub d_max: f64,
    pub d_t50_pct: Option<f64>,
    pub d_t95_pct: Option<f64>,
}

/// Corre todos los ensayos (perfiles × correlaciones × ensayos) con los parámetros dados.
pub fn sensibilidad(h: &Hidraulica, par: &Parametros, ens: &[Ensayo], criticas: &[f64]) -> Result<Vec<FilaSens>, String> {
    let par = Parametros { campo_cada: None, ..par.clone() };
    let mut filas: Vec<FilaSens> = Vec::new();
    for p in &h.perfiles {
        let perim: Vec<f64> = (0..p.a.len()).map(|i| p.a[i] / p.r[i]).collect();
        for e in ens {
            let mut aa = p.a.clone();
            if let Ensayo::AreaCritica(k) = e {
                for (i, s) in p.secciones.iter().enumerate() {
                    if criticas.contains(s) {
                        aa[i] *= k;
                    }
                }
            }
            for m in METODOS {
                // Se conservan ancho, perímetro y pendiente; U y R se recalculan con el área ensayada.
                let mut d: Vec<f64> = (0..aa.len()).map(|i| d_l(m, p.q / aa[i], aa[i], p.b[i], aa[i] / perim[i], p.sf[i])).collect();
                if let Ensayo::Dispersion(k) = e {
                    d.iter_mut().for_each(|v| *v *= k);
                }
                let r = simular_entrada(p.q, &p.x, &aa, &d, &par).map_err(|er| format!("{} · {} · {}: {er}", p.nombre, m.nombre(), e.etiqueta()))?;
                filas.push(FilaSens {
                    perfil: p.nombre.clone(),
                    q: p.q,
                    metodo: m,
                    ensayo: e.clone(),
                    t50_min: r.t50_min,
                    t95_min: r.t95_min,
                    fraccion_salida: r.fraccion_salida,
                    error_balance: r.error_balance,
                    d_min: d.iter().cloned().fold(f64::INFINITY, f64::min),
                    d_max: d.iter().cloned().fold(0.0, f64::max),
                    d_t50_pct: None,
                    d_t95_pct: None,
                });
            }
        }
    }
    // Cambio porcentual respecto a la referencia del mismo perfil y correlación.
    let refs: Vec<(String, Metodo, Option<f64>, Option<f64>)> = filas.iter().filter(|f| f.ensayo == Ensayo::Referencia).map(|f| (f.perfil.clone(), f.metodo, f.t50_min, f.t95_min)).collect();
    for f in filas.iter_mut() {
        if let Some((_, _, r50, r95)) = refs.iter().find(|(p, m, _, _)| *p == f.perfil && *m == f.metodo) {
            f.d_t50_pct = f.t50_min.zip(*r50).map(|(a, b)| 100.0 * (a / b - 1.0));
            f.d_t95_pct = f.t95_min.zip(*r95).map(|(a, b)| 100.0 * (a / b - 1.0));
        }
    }
    Ok(filas)
}

/// Una fila de la auditoría de D_L: una sección × una correlación.
#[derive(Clone, Debug)]
pub struct FilaAud {
    pub perfil: String,
    pub seccion: f64,
    pub distancia: f64,
    pub metodo: Metodo,
    pub q: f64,
    pub a: f64,
    pub b: f64,
    pub p: f64,
    pub sf: f64,
    pub u: f64,
    pub h: f64,
    pub r: f64,
    pub u_est: f64,
    pub b_sobre_h: f64,
    pub u_sobre_ust: f64,
    pub d: f64,
    pub critica: bool,
}

pub fn auditoria(h: &Hidraulica, criticas: &[f64]) -> Vec<FilaAud> {
    let mut v = Vec::new();
    for p in &h.perfiles {
        for m in METODOS {
            for i in 0..p.x.len() {
                let (u, hh) = (p.q / p.a[i], p.a[i] / p.b[i]);
                let us = u_estrella(p.r[i], p.sf[i]);
                v.push(FilaAud {
                    perfil: p.nombre.clone(),
                    seccion: p.secciones[i],
                    distancia: p.x[i],
                    metodo: m,
                    q: p.q,
                    a: p.a[i],
                    b: p.b[i],
                    p: p.a[i] / p.r[i],
                    sf: p.sf[i],
                    u,
                    h: hh,
                    r: p.r[i],
                    u_est: us,
                    b_sobre_h: p.b[i] / hh,
                    u_sobre_ust: u / us,
                    d: d_l(m, u, p.a[i], p.b[i], p.r[i], p.sf[i]),
                    critica: criticas.contains(&p.secciones[i]),
                });
            }
        }
    }
    v
}

/// Resumen por perfil y correlación: D mínimo, mediana, máximo y su cociente.
#[derive(Clone, Debug)]
pub struct ResumenAud {
    pub perfil: String,
    pub metodo: Metodo,
    pub d_min: f64,
    pub d_mediana: f64,
    pub d_max: f64,
    pub ratio: f64,
}

pub fn resumen_auditoria(a: &[FilaAud]) -> Vec<ResumenAud> {
    let mut out: Vec<ResumenAud> = Vec::new();
    for f in a {
        if out.iter().any(|r| r.perfil == f.perfil && r.metodo == f.metodo) {
            continue;
        }
        let mut d: Vec<f64> = a.iter().filter(|g| g.perfil == f.perfil && g.metodo == f.metodo).map(|g| g.d).collect();
        d.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let n = d.len();
        let med = if n % 2 == 1 { d[n / 2] } else { (d[n / 2 - 1] + d[n / 2]) / 2.0 };
        out.push(ResumenAud { perfil: f.perfil.clone(), metodo: f.metodo, d_min: d[0], d_mediana: med, d_max: d[n - 1], ratio: d[n - 1] / d[0] });
    }
    out
}

/// Los `n` mayores valores de D_L.
pub fn mayores(a: &[FilaAud], n: usize) -> Vec<FilaAud> {
    let mut v = a.to_vec();
    v.sort_by(|x, y| y.d.partial_cmp(&x.d).unwrap());
    v.truncate(n);
    v
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
    fn cuarenta_y_cinco_son_nueve_por_cinco_y_la_referencia_da_cero() {
        let h = hid();
        let c = criticas_por_defecto(&h);
        assert_eq!(c.len(), 2);
        let f = sensibilidad(&h, &Parametros::default(), &ensayos(2.0, 10.0), &c).unwrap();
        assert_eq!(f.len(), 3 * 5);
        assert!(f.iter().filter(|x| x.ensayo == Ensayo::Referencia).all(|x| x.d_t95_pct.unwrap().abs() < 1e-12));
        // Más dispersión → el pulso sale más pronto en t50 (o igual orden de magnitud); el balance se conserva.
        assert!(f.iter().all(|x| x.error_balance < 1e-9));
    }

    #[test]
    fn auditoria_tiene_filas_y_resumen() {
        let h = hid();
        let a = auditoria(&h, &[]);
        assert_eq!(a.len(), 3 * 4);
        let r = resumen_auditoria(&a);
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|x| x.d_min <= x.d_mediana && x.d_mediana <= x.d_max));
        assert_eq!(mayores(&a, 5).len(), 5);
    }
}

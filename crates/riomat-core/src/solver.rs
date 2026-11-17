//! Transporte 1D conservativo, pulso de masa entrando aguas arriba.
//! d(AC)/dt + d(QC)/dx = d(A D dC/dx)/dx, volúmenes finitos implícitos,
//! upwind en advección, conductancia armónica en difusión.
//! Frontera aguas arriba: flujo de masa M/T durante 0<=t<T. Aguas abajo: Q*C.

#[derive(Clone, Debug)]
pub struct Parametros {
    pub duracion_pulso: f64, // s
    pub masa: f64,           // masa normalizada
    pub dx_max: f64,         // m
    pub dt: f64,             // s
    pub t_final: f64,        // s
    /// Guardar el campo C(x) cada N pasos (para animar). None = no guardar.
    pub campo_cada: Option<usize>,
}

impl Default for Parametros {
    fn default() -> Self {
        Self { duracion_pulso: 300.0, masa: 1.0, dx_max: 10.0, dt: 30.0, t_final: 4.0 * 3600.0, campo_cada: None }
    }
}

#[derive(Clone, Debug)]
pub struct Resultado {
    pub x: Vec<f64>,
    pub t: Vec<f64>,
    pub inyectada: Vec<f64>,
    pub salida: Vec<f64>,
    pub masa_dominio: Vec<f64>,
    /// Campo de concentración guardado: (paso, C por celda).
    pub campo: Vec<(usize, Vec<f64>)>,
    pub c_final: Vec<f64>,
    pub error_balance: f64,
    pub t50_min: Option<f64>,
    pub t95_min: Option<f64>,
    pub fraccion_salida: f64,
    pub dx: f64,
    pub dt: f64,
    pub q: f64,
    pub masa: f64,
    pub concentracion_max: f64,
}

/// Interpolación lineal tipo `numpy.interp` (xp creciente, extremos fijos).
pub fn interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    if x <= xp[0] {
        return fp[0];
    }
    let n = xp.len();
    if x >= xp[n - 1] {
        return fp[n - 1];
    }
    let j = xp.partition_point(|&v| v <= x) - 1;
    fp[j] + (x - xp[j]) / (xp[j + 1] - xp[j]) * (fp[j + 1] - fp[j])
}

/// Tiempo (min) en que la fracción acumulada alcanza `umbral`.
pub fn tiempo_umbral(t: &[f64], f: &[f64], umbral: f64) -> Option<f64> {
    if *f.last()? < umbral {
        return None;
    }
    let j = f.iter().position(|&v| v >= umbral)?;
    if j == 0 {
        return Some(t[0] / 60.0);
    }
    let (f0, f1) = (f[j - 1], f[j]);
    let tt = if f1 == f0 { t[j] } else { t[j - 1] + (umbral - f0) / (f1 - f0) * (t[j] - t[j - 1]) };
    Some(tt / 60.0)
}

pub fn validar(q: f64, p: &Parametros) -> Result<(), String> {
    for (v, n) in [(q, "Q"), (p.duracion_pulso, "duración del pulso"), (p.masa, "masa"), (p.dx_max, "dx"), (p.dt, "dt"), (p.t_final, "tiempo final")] {
        if !(v > 0.0) {
            return Err(format!("{n} debe ser positivo."));
        }
    }
    let pasos = p.t_final / p.dt;
    if p.duracion_pulso > p.t_final || (pasos - pasos.round()).abs() > 1e-9 {
        return Err("Revisar duración del pulso, tiempo final y paso temporal (t_final debe ser múltiplo de dt).".into());
    }
    Ok(())
}

/// `x_sec`: distancia de cada sección (m, creciente); `areas` y `disp` en las secciones.
pub fn simular_entrada(q: f64, x_sec: &[f64], areas: &[f64], disp: &[f64], p: &Parametros) -> Result<Resultado, String> {
    validar(q, p)?;
    let longitud = x_sec[x_sec.len() - 1] - x_sec[0];
    let n = (longitud / p.dx_max).ceil() as usize;
    let dx = longitud / n as f64;
    let x: Vec<f64> = (0..n).map(|i| (i as f64 + 0.5) * dx).collect();
    let a: Vec<f64> = x.iter().map(|&v| interp(v, x_sec, areas)).collect();
    let d: Vec<f64> = x.iter().map(|&v| interp(v, x_sec, disp)).collect();
    if a.iter().any(|&v| v <= 0.0) || d.iter().any(|&v| v <= 0.0) {
        return Err("Área y dispersión deben ser positivas.".into());
    }
    let k: Vec<f64> = a.iter().zip(&d).map(|(a, d)| a * d).collect();
    let cond: Vec<f64> = (0..n - 1).map(|i| 2.0 * k[i] * k[i + 1] / (k[i] + k[i + 1]) / dx).collect();
    let vol_dt: Vec<f64> = a.iter().map(|a| a * dx / p.dt).collect();
    let mut diag = vol_dt.clone();
    for i in 0..n - 1 {
        diag[i] += q + cond[i];
        diag[i + 1] += cond[i];
    }
    diag[n - 1] += q;
    // Tridiagonal: sub[i] (fila i+1, col i) = -q - cond[i]; sup[i] (fila i, col i+1) = -cond[i]
    let sub: Vec<f64> = cond.iter().map(|c| -q - c).collect();
    let sup: Vec<f64> = cond.iter().map(|c| -c).collect();
    // Factorización de Thomas (una vez)
    let mut cp = vec![0.0; n - 1];
    let mut den = vec![0.0; n];
    den[0] = diag[0];
    for i in 1..n {
        cp[i - 1] = sup[i - 1] / den[i - 1];
        den[i] = diag[i] - sub[i - 1] * cp[i - 1];
    }
    let resolver = |rhs: &[f64], out: &mut Vec<f64>| {
        let mut dp = vec![0.0; n];
        dp[0] = rhs[0] / den[0];
        for i in 1..n {
            dp[i] = (rhs[i] - sub[i - 1] * dp[i - 1]) / den[i];
        }
        out[n - 1] = dp[n - 1];
        for i in (0..n - 1).rev() {
            out[i] = dp[i] - cp[i] * out[i + 1];
        }
    };
    let pasos = (p.t_final / p.dt).round() as usize;
    let mut c = vec![0.0; n];
    let mut nuevo = vec![0.0; n];
    let mut t = Vec::with_capacity(pasos + 1);
    let (mut inyectada, mut salida, mut masa_dom) = (vec![0.0; pasos + 1], vec![0.0; pasos + 1], vec![0.0; pasos + 1]);
    let mut campo = Vec::new();
    if p.campo_cada.is_some() {
        campo.push((0usize, c.clone()));
    }
    let mut cmax = 0.0_f64;
    for j in 0..=pasos {
        t.push(j as f64 * p.dt);
    }
    for j in 1..=pasos {
        let activo = ((j as f64 * p.dt).min(p.duracion_pulso) - ((j - 1) as f64 * p.dt).max(0.0)).max(0.0);
        let incremento = p.masa * activo / p.duracion_pulso;
        let mut rhs: Vec<f64> = vol_dt.iter().zip(&c).map(|(v, c)| v * c).collect();
        rhs[0] += incremento / p.dt;
        resolver(&rhs, &mut nuevo);
        std::mem::swap(&mut c, &mut nuevo);
        if c.iter().cloned().fold(f64::INFINITY, f64::min) < -1e-10 {
            return Err("Concentración negativa: revisar la discretización.".into());
        }
        inyectada[j] = inyectada[j - 1] + incremento;
        salida[j] = salida[j - 1] + p.dt * q * c[n - 1];
        masa_dom[j] = a.iter().zip(&c).map(|(a, c)| a * dx * c).sum();
        cmax = c.iter().cloned().fold(cmax, f64::max);
        if let Some(cada) = p.campo_cada {
            if j % cada == 0 {
                campo.push((j, c.clone()));
            }
        }
    }
    let error_balance = (0..=pasos).map(|j| (inyectada[j] - salida[j] - masa_dom[j]).abs()).fold(0.0, f64::max) / p.masa;
    let frac: Vec<f64> = salida.iter().map(|s| s / p.masa).collect();
    Ok(Resultado {
        x, t: t.clone(), inyectada, salida: salida.clone(), masa_dominio: masa_dom, campo, c_final: c,
        error_balance,
        t50_min: tiempo_umbral(&t, &frac, 0.5),
        t95_min: tiempo_umbral(&t, &frac, 0.95),
        fraccion_salida: salida[pasos] / p.masa,
        dx, dt: p.dt, q, masa: p.masa, concentracion_max: cmax,
    })
}

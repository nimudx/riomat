//! Estado y pantalla principal de Riomat (tema claro, esquema vivo del tramo + gráficas + telemetría).
use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use riomat_core::solver::interp;
use riomat_plot::{Figura, a_svg, svg_a_pdf, svg_a_png};
use riomat_core::*;
use riomat_core::{analisis, exportar};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::plot::*;
use crate::theme::*;
use crate::theme::px;
use gpui_kit::px as rpx;

const DURACIONES_BASE: [f64; 4] = [30.0, 300.0, 900.0, 1800.0];
const VELOCIDADES: [usize; 4] = [1, 2, 4, 8];
const CAMPO_CADA: usize = 8; // un cuadro cada 240 s
const SEGMENTOS: usize = 60;

actions!(riomat, [NuevoDatos, AbrirArchivo, PegarTabla, ExportarTodo]);

/// Atajos de teclado (Ctrl en Linux y Windows, Cmd en macOS).
pub fn registrar_atajos(cx: &mut App) {
    let m = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    let k = |tecla: &str| -> &'static str { Box::leak(format!("{m}-{tecla}").into_boxed_str()) };
    cx.bind_keys([
        KeyBinding::new(k("n"), NuevoDatos, Some("Riomat")),
        KeyBinding::new(k("o"), AbrirArchivo, Some("Riomat")),
        KeyBinding::new(k("v"), PegarTabla, Some("Riomat")),
        KeyBinding::new(k("e"), ExportarTodo, Some("Riomat")),
    ]);
}

/// Acción que pide confirmación si hay resultados sin exportar.
#[derive(Clone)]
enum Accion {
    Nuevo,
    Abrir,
    Pegar,
    Reciente(Vec<PathBuf>),
}

/// Carpeta de configuración del usuario (para la lista de archivos recientes).
fn carpeta_config() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(d).join("riomat");
    }
    if let Some(d) = std::env::var_os("APPDATA") {
        return PathBuf::from(d).join("riomat");
    }
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join(".config").join("riomat")
}

/// Ajustes del usuario: archivo `ajustes.txt` con líneas `clave=valor`.
fn leer_ajuste(clave: &str) -> Option<String> {
    let t = std::fs::read_to_string(carpeta_config().join("ajustes.txt")).ok()?;
    t.lines().find_map(|l| l.strip_prefix(&format!("{clave}=")).map(|v| v.trim().to_string())).filter(|v| !v.is_empty())
}

fn guardar_ajuste(clave: &str, valor: Option<&str>) {
    let dir = carpeta_config();
    let ruta = dir.join("ajustes.txt");
    let previo = std::fs::read_to_string(&ruta).unwrap_or_default();
    let mut lineas: Vec<String> = previo.lines().filter(|l| !l.starts_with(&format!("{clave}="))).map(String::from).collect();
    if let Some(v) = valor {
        lineas.push(format!("{clave}={v}"));
    }
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(ruta, lineas.join("\n"));
    }
}

fn leer_recientes() -> Vec<Vec<PathBuf>> {
    std::fs::read_to_string(carpeta_config().join("recientes.txt"))
        .map(|t| t.lines().map(|l| l.split('\t').map(PathBuf::from).filter(|p| p.exists()).collect::<Vec<_>>()).filter(|v| !v.is_empty()).take(5).collect())
        .unwrap_or_default()
}

fn nombre_reciente(v: &[PathBuf]) -> String {
    v.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect::<Vec<_>>().join(" + ")
}

fn color_metodo(m: Metodo) -> u32 {
    match m {
        Metodo::Fischer => C_FISCHER,
        Metodo::SeoCheong => C_SEO,
        Metodo::KashefipourFalconer => C_KF,
    }
}

/// Rampa del trazador: agua clara → ámbar → bermellón (como tinta en el agua).
fn rampa(t: f64) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let (a, b, u) = if t < 0.5 { (AGUA, TRAZ_MEDIO, t * 2.0) } else { (TRAZ_MEDIO, TRAZ_ALTO, (t - 0.5) * 2.0) };
    let m = |s: u32| {
        let (x, y) = (((a >> s) & 0xff) as f64, ((b >> s) & 0xff) as f64);
        ((x + (y - x) * u).round() as u32) << s
    };
    m(16) | m(8) | m(0)
}

#[derive(Clone, Copy, PartialEq)]
enum Pestana {
    Simulacion,
    Figuras,
    Datos,
    Sensibilidad,
    Auditoria,
    Comparar,
}

/// Simulaciones de los tres caudales con la duración de pulso elegida, para compararlas.
struct Comparacion {
    sims: HashMap<(usize, usize), Resultado>, // (perfil, método)
    d: HashMap<(usize, usize), Vec<f64>>,
}

struct Vista {
    sims: HashMap<(usize, usize), Resultado>,
    d: Vec<Vec<f64>>,
}

pub struct Riomat {
    hid: Option<Hidraulica>,
    archivos: Vec<String>,
    error: Option<String>,
    perfil: usize,
    metodo: usize,
    dur: usize,
    vel: usize,
    frame: usize,
    playing: bool,
    generacion: u64,
    vista: Option<Vista>,
    export_msg: Option<String>,
    exportando: bool,
    pestana: Pestana,
    fig_sel: usize,
    ver_avisos: bool,
    ancho: f32,
    sin_exportar: bool,
    confirmar: Option<Accion>,
    acerca: bool,
    menu: bool,
    ajustes: bool,
    recientes: Vec<Vec<PathBuf>>,
    foco: FocusHandle,
    comp: Option<Comparacion>,
    sens: Option<Vec<analisis::FilaSens>>,
    sens_error: Option<String>,
    criticas: Vec<f64>,
    in_kd: Entity<InputState>,
    in_pa: Entity<InputState>,
    durs: Vec<f64>,
    par: Parametros,
    in_masa: Entity<InputState>,
    in_dur: Entity<InputState>,
    in_dx: Entity<InputState>,
    in_dt: Entity<InputState>,
    in_tf: Entity<InputState>,
}

impl Riomat {
    pub fn new(args: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let par = Parametros::default();
        let campo = |valor: String, ayuda: &'static str, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).placeholder(ayuda).default_value(valor));
        let (in_masa, in_dur, in_dx, in_dt, in_tf) = (
            campo(texto_num(par.masa), "masa", window, cx),
            campo(texto_num(par.duracion_pulso), "s", window, cx),
            campo(texto_num(par.dx_max), "m", window, cx),
            campo(texto_num(par.dt), "s", window, cx),
            campo(texto_num(par.t_final / 60.0), "min", window, cx),
        );
        let in_kd = campo("2".into(), "×", window, cx);
        let in_pa = campo("10".into(), "%", window, cx);
        let mut s = Self { hid: None, archivos: vec![], error: None, perfil: 1, metodo: 1, dur: 1, vel: 0, frame: 4, playing: false, generacion: 0, vista: None, export_msg: None, exportando: false, pestana: Pestana::Simulacion, fig_sel: 0, ver_avisos: false, ancho: 1400.0, sin_exportar: false, confirmar: None, acerca: false, menu: false, ajustes: false, recientes: leer_recientes(), foco: cx.focus_handle(), comp: None, sens: None, sens_error: None, criticas: vec![], in_kd, in_pa, durs: DURACIONES_BASE.to_vec(), par, in_masa, in_dur, in_dx, in_dt, in_tf };
        let rutas: Vec<PathBuf> = if !args.is_empty() { args } else { Self::buscar_por_defecto() };
        if !rutas.is_empty() {
            s.cargar(&rutas);
        }
        s
    }

    fn buscar_por_defecto() -> Vec<PathBuf> {
        let cand = [PathBuf::from("referencia"), PathBuf::from("."), PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../referencia")];
        for c in cand {
            let v = excels_en(&c);
            if v.len() >= 2 {
                return v;
            }
        }
        vec![]
    }

    fn cargar(&mut self, rutas: &[PathBuf]) {
        let r = if rutas.len() == 1 && rutas[0].is_dir() { excels_en(&rutas[0]) } else { rutas.to_vec() };
        let nombres: Vec<String> = r.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
        let res = match r.len() {
            0 => Err("No hay archivos.".to_string()),
            1 => leer_tabla_archivo(&r[0]),
            _ => leer_hidraulica_auto(&r[0], &r[1]),
        };
        let ok = res.is_ok();
        self.aplicar(res, nombres.into_iter().take(2).collect());
        if ok {
            self.registrar_reciente(r.into_iter().take(2).collect());
        }
    }

    fn registrar_reciente(&mut self, rutas: Vec<PathBuf>) {
        let rutas: Vec<PathBuf> = rutas.into_iter().map(|p| p.canonicalize().unwrap_or(p)).collect();
        self.recientes.retain(|v| *v != rutas);
        self.recientes.insert(0, rutas);
        self.recientes.truncate(5);
        let texto = self.recientes.iter().map(|v| v.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n");
        let dir = carpeta_config();
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(dir.join("recientes.txt"), texto);
        }
    }

    /// Ejecuta la acción o, si hay resultados sin exportar, pide confirmación primero.
    fn pedir(&mut self, a: Accion, window: &mut Window, cx: &mut Context<Self>) {
        if self.hid.is_some() && self.sin_exportar && !matches!(a, Accion::Nuevo if self.hid.is_none()) {
            self.confirmar = Some(a);
            cx.notify();
        } else {
            self.ejecutar(a, window, cx);
        }
    }

    fn ejecutar(&mut self, a: Accion, window: &mut Window, cx: &mut Context<Self>) {
        self.confirmar = None;
        match a {
            Accion::Nuevo => self.nuevo(window, cx),
            Accion::Abrir => self.abrir_dialogo(cx),
            Accion::Pegar => self.pegar(cx),
            Accion::Reciente(v) => {
                self.cargar(&v);
                self.frame = self.frame_inicial();
                cx.notify();
            }
        }
    }

    /// Cierra los datos y resultados actuales y vuelve a la pantalla de inicio con los parámetros por defecto.
    fn nuevo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playing = false;
        self.generacion += 1;
        self.restablecer(window, cx);
        self.hid = None;
        self.vista = None;
        self.comp = None;
        self.sens = None;
        self.sens_error = None;
        self.archivos.clear();
        self.error = None;
        self.export_msg = None;
        self.criticas.clear();
        self.sin_exportar = false;
        self.ver_avisos = false;
        self.pestana = Pestana::Simulacion;
        self.perfil = 1;
        self.metodo = 1;
        self.frame = 4;
        cx.notify();
    }

    fn aplicar(&mut self, res: Result<Hidraulica, String>, archivos: Vec<String>) {
        self.sens = None;
        self.comp = None;
        match res {
            Ok(h) => {
                self.archivos = archivos;
                self.perfil = self.perfil.min(h.perfiles.len().saturating_sub(1));
                self.criticas = analisis::criticas_por_defecto(&h);
                self.sin_exportar = true;
                self.hid = Some(h);
                self.error = None;
                self.recalcular();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Pega una tabla de resultados por perfil copiada desde HEC-RAS.
    fn pegar(&mut self, cx: &mut Context<Self>) {
        let texto = cx.read_from_clipboard().and_then(|i| i.text());
        match texto {
            Some(t) => {
                let res = leer_tabla_texto(&t);
                self.aplicar(res, vec!["Tabla pegada del portapapeles".into()]);
            }
            None => self.error = Some("El portapapeles no tiene texto. Copia la tabla de perfiles en HEC-RAS y vuelve a intentar.".into()),
        }
        cx.notify();
    }

    fn recalcular(&mut self) {
        self.comp = None;
        let Some(h) = &self.hid else { return };
        let p = &h.perfiles[self.perfil];
        let mut sims = HashMap::new();
        let mut d = Vec::new();
        for (mi, m) in METODOS.iter().enumerate() {
            let dl = coeficientes(*m, &p.u, &p.a, &p.b, &p.r, &p.sf);
            for (di, dur) in self.durs.iter().enumerate() {
                let par = Parametros { duracion_pulso: *dur, campo_cada: Some(CAMPO_CADA), ..self.par.clone() };
                match simular_entrada(p.q, &p.x, &p.a, &dl, &par) {
                    Ok(r) => {
                        sims.insert((mi, di), r);
                    }
                    Err(e) => self.error = Some(format!("{} / {}: {e}", p.nombre, m.nombre())),
                }
            }
            d.push(dl);
        }
        self.vista = Some(Vista { sims, d });
        self.frame = self.frame.min(self.n_frames().saturating_sub(1));
    }

    fn actual(&self) -> Option<&Resultado> {
        self.vista.as_ref()?.sims.get(&(self.metodo, self.dur))
    }

    /// Fotograma inicial: un instante con trazador en el tramo (no el t = 0 vacío).
    fn frame_inicial(&self) -> usize {
        4.min(self.n_frames().saturating_sub(1))
    }

    fn n_frames(&self) -> usize {
        self.actual().map(|r| r.campo.len()).unwrap_or(1)
    }

    fn leer_campo(&self, estado: &Entity<InputState>, nombre: &str, cx: &App) -> Result<f64, String> {
        let t = estado.read(cx).value().to_string();
        riomat_core::tabla::numero(&t).ok_or_else(|| format!("«{nombre}» no es un número válido: «{t}»."))
    }

    /// Lee los parámetros escritos, los valida y ejecuta de nuevo la simulación.
    fn calcular(&mut self, cx: &mut Context<Self>) {
        let leidos = (|| -> Result<(Parametros, f64), String> {
            let masa = self.leer_campo(&self.in_masa, "Masa inyectada", cx)?;
            let dur = self.leer_campo(&self.in_dur, "Duración del pulso", cx)?;
            let dx = self.leer_campo(&self.in_dx, "Paso espacial dx", cx)?;
            let dt = self.leer_campo(&self.in_dt, "Paso de tiempo dt", cx)?;
            let tf = self.leer_campo(&self.in_tf, "Tiempo total", cx)? * 60.0;
            let par = Parametros { duracion_pulso: dur, masa, dx_max: dx, dt, t_final: tf, campo_cada: None };
            riomat_core::solver::validar(1.0, &par)?;
            Ok((par, dur))
        })();
        match leidos {
            Ok((par, dur)) => {
                if !self.durs.iter().any(|d| (d - dur).abs() < 1e-9) {
                    self.durs.push(dur);
                    self.durs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    if self.durs.len() > 6 {
                        // Conserva la elegida y las predefinidas más cercanas
                        let quitar = self.durs.iter().position(|d| !DURACIONES_BASE.contains(d) && (d - dur).abs() > 1e-9);
                        if let Some(q) = quitar {
                            self.durs.remove(q);
                        }
                    }
                }
                self.dur = self.durs.iter().position(|d| (d - dur).abs() < 1e-9).unwrap_or(0);
                self.par = par;
                self.error = None;
                self.sens = None;
                self.recalcular();
                self.frame = self.frame.min(self.n_frames().saturating_sub(1));
            }
            Err(e) => self.error = Some(e),
        }
        cx.notify();
    }

    fn poner(&self, estado: &Entity<InputState>, texto: String, window: &mut Window, cx: &mut Context<Self>) {
        estado.update(cx, |s, cx| s.set_value(texto, window, cx));
    }

    fn restablecer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let p = Parametros::default();
        self.poner(&self.in_masa.clone(), texto_num(p.masa), window, cx);
        self.poner(&self.in_dur.clone(), texto_num(p.duracion_pulso), window, cx);
        self.poner(&self.in_dx.clone(), texto_num(p.dx_max), window, cx);
        self.poner(&self.in_dt.clone(), texto_num(p.dt), window, cx);
        self.poner(&self.in_tf.clone(), texto_num(p.t_final / 60.0), window, cx);
        self.durs = DURACIONES_BASE.to_vec();
        self.par = p;
        self.sens = None;
        self.dur = 1;
        self.error = None;
        self.recalcular();
        cx.notify();
    }

    fn set_perfil(&mut self, i: usize, cx: &mut Context<Self>) {
        self.perfil = i;
        self.recalcular();
        cx.notify();
    }

    fn alternar(&mut self, cx: &mut Context<Self>) {
        self.playing = !self.playing;
        self.generacion += 1;
        if !self.playing {
            cx.notify();
            return;
        }
        if self.frame + 1 >= self.n_frames() {
            self.frame = 0;
        }
        let g = self.generacion;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(80)).await;
                let seguir = this
                    .update(cx, |this, cx| {
                        if this.generacion != g || !this.playing {
                            return false;
                        }
                        this.frame = (this.frame + VELOCIDADES[this.vel]).min(this.n_frames() - 1);
                        if this.frame + 1 >= this.n_frames() {
                            this.playing = false;
                        }
                        cx.notify();
                        this.playing
                    })
                    .unwrap_or(false);
                if !seguir {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
}

fn excels_en(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("xlsx"))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Fecha UTC (AAAA-MM-DD) sin dependencias externas.
fn fecha_utc() -> String {
    let dias = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86400).unwrap_or(0) as i64;
    let z = dias + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Informe de una página: parámetros, los nueve escenarios, dos figuras y la verificación.
#[allow(clippy::too_many_arguments)]
fn armar_informe(h: &Hidraulica, par: &Parametros, esc: &[exportar::Escenario], figs: &[Figura], verif: &[exportar::Verificacion], archivos: &[String], perfil: &str, metodo: Metodo) -> riomat_plot::Informe {
    let p0 = &h.perfiles[0];
    let mut parametros = vec![
        ("Programa".to_string(), format!("Riomat {}", env!("CARGO_PKG_VERSION"))),
        ("Fecha (UTC)".into(), fecha_utc()),
        ("Tramo".into(), format!("{:.0} m, {} secciones", p0.longitud(), p0.x.len())),
        ("Caudales".into(), h.perfiles.iter().map(|p| format!("{} = {:.1}", p.nombre, p.q)).collect::<Vec<_>>().join(", ") + " m³/s"),
        ("Datos".into(), if archivos.is_empty() { "-".into() } else { archivos.join(" + ") }),
        ("Escenario mostrado".into(), format!("{perfil} · {}", metodo.nombre())),
        ("Masa inyectada".into(), format!("{} (normalizada)", par.masa)),
        ("Duración del pulso".into(), format!("{:.0} s (hipótesis)", par.duracion_pulso)),
        ("Malla".into(), format!("dx ≤ {} m · dt = {} s", par.dx_max, par.dt)),
        ("Tiempo simulado".into(), format!("{:.0} min", par.t_final / 60.0)),
    ];
    parametros.truncate(10);
    let mut tabla = vec![vec!["Caudal (m³/s)".to_string(), "Correlación".into(), "t50 (min)".into(), "t95 (min)".into(), "Evacuado (%)".into(), "D_L mín–máx".into(), "Err. balance".into()]];
    for e in esc {
        let (dmin, dmax) = (e.d.iter().cloned().fold(f64::INFINITY, f64::min), e.d.iter().cloned().fold(0.0, f64::max));
        tabla.push(vec![format!("{} ({:.1})", e.perfil, e.res.q), e.metodo.nombre().into(), fmt_min(e.res.t50_min).replace(" min", ""), fmt_min(e.res.t95_min).replace(" min", ""), format!("{:.2}", e.res.fraccion_salida * 100.0), format!("{dmin:.0}–{dmax:.0}"), format!("{:.0e}", e.res.error_balance)]);
    }
    let elegir = |n: &str| figs.iter().find(|f| f.nombre.starts_with(n)).cloned();
    let figuras: Vec<Figura> = [elegir("comparacion_masa_evacuada").or_else(|| elegir("masa_evacuada")), elegir("mapa_espacio_tiempo")].into_iter().flatten().collect();
    let mut avisos: Vec<String> = verif.iter().filter(|(n, _, _)| n != "info").map(|(_, c, m)| format!("{c}: {m}")).collect();
    avisos.extend(verif.iter().filter(|(n, _, _)| n == "info").map(|(_, c, m)| format!("{c}: {m}")));
    riomat_plot::Informe {
        titulo: "Transporte 1D de un trazador pasivo · Puente Breña".into(),
        subtitulo: "Río Mantaro · resultados exploratorios generados con Riomat a partir de la hidráulica de HEC-RAS".into(),
        parametros,
        tabla,
        figuras,
        avisos,
        alcance: "Alcance: trazador pasivo conservativo (no modela deposición, resuspensión, flotación ni granulometría de microplásticos). D_L proviene de correlaciones empíricas sin calibrar; la masa y la duración del pulso son hipótesis, no mediciones.".into(),
        pie: format!("Riomat {} · {} · ecuación: ∂(AC)/∂t + ∂(QC)/∂x = ∂/∂x(A·D·∂C/∂x), volúmenes finitos implícitos", env!("CARGO_PKG_VERSION"), fecha_utc()),
    }
}

fn texto_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 { format!("{v:.0}") } else { format!("{v}") }
}

fn etiqueta_duracion(d: f64) -> String {
    if d >= 60.0 && (d / 60.0 - (d / 60.0).round()).abs() < 1e-9 { format!("{:.0} min", d / 60.0) } else { format!("{d:.0} s") }
}

fn fmt_min(v: Option<f64>) -> String {
    v.map(|m| format!("{m:.1} min")).unwrap_or_else(|| "—".into())
}

// ---------- piezas visuales ----------

fn seccion(titulo: &'static str) -> Div {
    div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(DIM)).child(titulo.to_uppercase())
}

/// Tarjeta con encabezado, como las de la referencia.
fn tarjeta(titulo: &'static str, leyenda: Vec<(String, u32)>, cuerpo: impl IntoElement) -> Div {
    let cab = h_flex().items_center().px_3().h(px(34.)).bg(rgb(HEADER)).border_b_1().border_color(rgb(BORDER)).child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).child(titulo));
    let leyenda = h_flex().gap_3().flex_wrap().children(leyenda.into_iter().map(|(n, c)| {
        h_flex().gap_1p5().items_center().child(div().size(px(10.)).rounded_sm().border_2().border_color(rgb(c))).child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(n))
    }));
    v_flex().flex_none().rounded_lg().bg(rgb(PANEL)).border_1().border_color(rgb(BORDER)).overflow_hidden().min_w_0().child(cab).child(v_flex().gap_2().p_3().child(leyenda).child(cuerpo))
}

/// Tabla de texto con columnas de igual ancho.
fn tabla_txt(cab: &[&str], filas: Vec<Vec<String>>) -> Div {
    let celda = |t: String, cab: bool| div().flex_1().min_w_0().text_size(px(12.)).text_color(rgb(if cab { MUTED } else { TEXT })).font_weight(if cab { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).child(t);
    let mut t = v_flex().child(h_flex().min_h(px(32.)).items_center().px_3().gap_2().bg(rgb(HEADER)).children(cab.iter().map(|c| celda(c.to_string(), true))));
    for f in filas {
        t = t.child(h_flex().min_h(px(30.)).items_center().px_3().gap_2().border_t_1().border_color(rgb(BORDER)).children(f.into_iter().map(|c| celda(c, false))));
    }
    t
}

fn fila(etiqueta: &'static str, valor: String) -> Div {
    h_flex()
        .justify_between()
        .py(px(6.))
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(div().text_size(px(12.)).text_color(rgb(TEXT)).child(etiqueta))
        .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(valor))
}

fn opcion(id: impl Into<ElementId>, texto: String, activo: bool, color: Option<u32>) -> Stateful<Div> {
    let mut b = div()
        .id(id)
        .px_3()
        .h(px(30.))
        .flex()
        .items_center()
        .gap_2()
        .rounded_md()
        .cursor_pointer()
        .text_size(px(12.5))
        .border_1()
        .border_color(rgb(if activo { ACCENT } else { BORDER }))
        .bg(if activo { rgba(ACCENT_SOFT) } else { rgb(PANEL).into() })
        .hover(|s| s.bg(rgb(HOVER)));
    if let Some(c) = color {
        b = b.child(div().size(px(9.)).rounded_full().bg(rgb(c)));
    }
    b.child(texto)
}

fn boton(id: &'static str, texto: &'static str, color: u32) -> Stateful<Div> {
    div().id(id).h(px(28.)).px_4().flex().items_center().rounded_full().cursor_pointer().bg(rgb(color)).text_color(rgb(0xffffff)).text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).child(texto)
}

/// Logotipo de Riomat (incrustado desde `assets/`): una R cuya pierna fluye como un río y termina en la gota de trazador.
fn logo() -> impl IntoElement {
    logo_tam(28.0)
}

fn logo_tam(lado: f32) -> impl IntoElement {
    static IMAGEN: std::sync::OnceLock<Arc<Image>> = std::sync::OnceLock::new();
    let imagen = IMAGEN.get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Png, include_bytes!("../../../assets/icon-128.png").to_vec()))).clone();
    img(imagen).size(px(lado)).flex_none()
}

fn pastilla(texto: String) -> Div {
    div().h(px(28.)).px_3().flex().items_center().rounded_full().bg(rgb(0xe3eef5)).text_size(px(12.)).text_color(rgb(MUTED)).child(texto)
}

impl Render for Riomat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ancho = f32::from(window.viewport_size().width) / ESCALA;
        let raiz = v_flex()
            .id("raiz")
            .key_context("Riomat")
            .track_focus(&self.foco)
            .on_action(cx.listener(|this, _: &NuevoDatos, window, cx| this.pedir(Accion::Nuevo, window, cx)))
            .on_action(cx.listener(|this, _: &AbrirArchivo, window, cx| this.pedir(Accion::Abrir, window, cx)))
            .on_action(cx.listener(|this, _: &PegarTabla, window, cx| this.pedir(Accion::Pegar, window, cx)))
            .on_action(cx.listener(|this, _: &ExportarTodo, _, cx| {
                if this.hid.is_some() && !this.exportando {
                    this.exportar_todo(cx);
                }
            }))
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .on_drop(cx.listener(|this, p: &ExternalPaths, _, cx| {
                this.cargar(p.paths());
                cx.notify();
            }));
        if !self.foco.contains_focused(window, cx) && !self.acerca && self.confirmar.is_none() {
            window.focus(&self.foco, cx);
        }
        let menu = (self.menu && self.hid.is_some() && self.vista.is_some()).then(|| self.menu_perfil(cx));
        let capa = if let Some(a) = self.confirmar.clone() {
            Some(self.dialogo_confirmar(a, cx).into_any_element())
        } else if self.ajustes {
            Some(self.dialogo_ajustes(cx).into_any_element())
        } else if self.acerca {
            Some(self.dialogo_acerca(cx).into_any_element())
        } else {
            None
        };
        if self.hid.is_none() || self.vista.is_none() {
            return raiz.child(self.vacio(cx)).children(capa);
        }
        if self.pestana == Pestana::Comparar && self.comp.is_none() {
            self.calcular_comp();
        }
        let centro = match self.pestana {
            Pestana::Simulacion => self.centro(cx).into_any_element(),
            Pestana::Figuras => self.vista_figuras(cx).into_any_element(),
            Pestana::Datos => self.vista_datos(cx).into_any_element(),
            Pestana::Sensibilidad => self.vista_sensibilidad(cx).into_any_element(),
            Pestana::Auditoria => self.vista_auditoria(cx).into_any_element(),
            Pestana::Comparar => self.vista_comparar(cx).into_any_element(),
        };
        let cuerpo = h_flex().flex_1().min_h_0().child(self.panel_izq(cx)).child(centro);
        let cuerpo = if self.ver_avisos { cuerpo.child(self.panel_der(cx)) } else { cuerpo };
        raiz.child(self.barra_superior(cx)).child(cuerpo).children(menu).children(capa)
    }
}

impl Riomat {
    fn vacio(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(h_flex().gap_3().items_center().child(logo_tam(56.)).child(div().text_size(px(32.)).font_weight(FontWeight::SEMIBOLD).child("riomat")))
            .child(div().text_color(rgb(MUTED)).child("Del resultado de HEC-RAS al transporte de un pulso, sin preparar Excel"))
            .child(
                v_flex().gap_1().max_w(px(560.)).p_4().rounded_lg().bg(rgb(PANEL)).border_1().border_color(rgb(BORDER))
                    .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).child("Cómo cargar los datos"))
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("1. En HEC-RAS abre la tabla de resultados por perfil (Profile Table) y elige las columnas: River Sta, Profile, Q Total, W.P. Total, Flow Area, Top Width y E.G. Slope. Si quieres el perfil del río, añade Min Ch El, W.S. Elev y E.G. Elev."))
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("2. Copia la tabla y pulsa «Abrir archivo…» (o «Pegar tabla»), o arrastra aquí un archivo .txt, .csv o .xlsx con esa tabla."))
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("También funcionan los dos Excel anteriores: arrástralos juntos.")),
            )
            .children(self.error.clone().map(|e| div().max_w(px(560.)).text_color(rgb(ERR)).child(e)))
            .children((!self.recientes.is_empty()).then(|| {
                v_flex().gap_1().items_center()
                    .child(div().text_size(px(11.5)).text_color(rgb(MUTED)).child("Recientes"))
                    .children(self.recientes.iter().enumerate().map(|(i, v)| {
                        let v2 = v.clone();
                        div().id(("rec", i)).cursor_pointer().text_size(px(12.5)).text_color(rgb(ACCENT)).child(nombre_reciente(v))
                            .on_click(cx.listener(move |this, _, window, cx| this.pedir(Accion::Reciente(v2.clone()), window, cx)))
                    }))
            }))
            .child(h_flex().gap_2()
                .child(boton("abrir", "Abrir archivo…", ACCENT).on_click(cx.listener(|this, _, window, cx| this.pedir(Accion::Abrir, window, cx))))
                .child(boton("pegar", "Pegar tabla de HEC-RAS", RESET).on_click(cx.listener(|this, _, window, cx| this.pedir(Accion::Pegar, window, cx))))
                .child(boton("ejemplo", "Cargar datos de ejemplo", RESET).on_click(cx.listener(|this, _, _, cx| {
                    let v = Riomat::buscar_por_defecto();
                    this.cargar(&v);
                    cx.notify();
                })))
                .child(boton("acerca0", "Acerca de", RESET).on_click(cx.listener(|this, _, _, cx| { this.acerca = true; cx.notify(); }))))
    }

    fn capa(&self, contenido: impl IntoElement) -> Div {
        div().absolute().top_0().left_0().size_full().bg(gpui::rgba(0x00000066)).flex().items_center().justify_center()
            .child(v_flex().gap_3().p_5().rounded_lg().bg(rgb(PANEL)).border_1().border_color(rgb(BORDER)).child(contenido))
    }

    /// Menú del avatar (arriba a la derecha): datos, exportación, avisos y «Acerca de».
    fn menu_perfil(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let item = |id: &'static str, icono: &'static str, texto: &'static str, atajo: &'static str| {
            h_flex().id(id).gap_2().h(px(32.)).px_2().items_center().rounded_md().cursor_pointer().hover(|s| s.bg(rgb(0xe3eef5)))
                .text_size(px(13.))
                .child(div().w(px(18.)).flex_none().text_color(rgb(MUTED)).child(icono))
                .child(div().flex_1().child(texto))
                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(atajo))
        };
        let sep = || div().h(px(1.)).my_1().bg(rgb(BORDER));
        let tarjeta = v_flex().w(px(260.)).p_2().gap_0p5().rounded_lg().bg(rgb(PANEL)).border_1().border_color(rgb(BORDER)).shadow_lg()
            .child(
                h_flex().gap_3().items_center().p_2()
                    .child(div().size(px(40.)).flex_none().flex().items_center().justify_center().rounded_full().bg(rgb(ACCENT)).text_color(rgb(0xffffff)).font_weight(FontWeight::SEMIBOLD).child("AR"))
                    .child(v_flex().child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child("Andrea Riveros")).child(div().text_size(px(11.5)).text_color(rgb(MUTED)).child("colab. nimudx"))),
            )
            .child(sep())
            .child(item("m_nuevo", "＋", "Nuevo", "Ctrl+N").on_click(cx.listener(|this, _, window, cx| { this.menu = false; this.pedir(Accion::Nuevo, window, cx); })))
            .child(item("m_abrir", "⌕", "Abrir archivo…", "Ctrl+O").on_click(cx.listener(|this, _, window, cx| { this.menu = false; this.pedir(Accion::Abrir, window, cx); })))
            .child(item("m_pegar", "⎘", "Pegar tabla", "Ctrl+V").on_click(cx.listener(|this, _, window, cx| { this.menu = false; this.pedir(Accion::Pegar, window, cx); })))
            .child(sep())
            .child(item("m_exp", "⇩", "Exportar todo", "Ctrl+E").on_click(cx.listener(|this, _, _, cx| { this.menu = false; if !this.exportando { this.exportar_todo(cx); } cx.notify(); })))
            .child(item("m_carp", "▤", "Abrir carpeta de exportación", "").on_click(cx.listener(|this, _, _, cx| { this.menu = false; Riomat::abrir_carpeta_export(); cx.notify(); })))
            .child(item("m_ajustes", "⚙", "Ajustes…", "").on_click(cx.listener(|this, _, _, cx| { this.menu = false; this.ajustes = true; cx.notify(); })))
            .child(item("m_av", "⚠", if self.ver_avisos { "Ocultar verificación" } else { "Ver verificación" }, "").on_click(cx.listener(|this, _, _, cx| { this.menu = false; this.ver_avisos = !this.ver_avisos; cx.notify(); })))
            .child(sep())
            .child(item("m_acerca", "ⓘ", "Acerca de Riomat", "").on_click(cx.listener(|this, _, _, cx| { this.menu = false; this.acerca = true; cx.notify(); })));
        div().id("menu_fondo").absolute().top_0().left_0().size_full().flex().justify_end().items_start().pt(px(52.)).pr(px(12.))
            .on_click(cx.listener(|this, _, _, cx| { this.menu = false; cx.notify(); }))
            .child(tarjeta)
    }

    fn abrir_carpeta_export() {
        let dir = Riomat::carpeta_export();
        let _ = std::fs::create_dir_all(&dir);
        let programa = if cfg!(target_os = "windows") { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(programa).arg(&dir).spawn();
    }

    fn elegir_carpeta_export(&mut self, cx: &mut Context<Self>) {
        let tarea = cx.background_executor().spawn(async move {
            rfd::AsyncFileDialog::new().set_title("Carpeta donde guardar las exportaciones").pick_folder().await.map(|f| f.path().to_path_buf())
        });
        cx.spawn(async move |this, cx| {
            if let Some(dir) = tarea.await {
                let _ = this.update(cx, |this, cx| {
                    guardar_ajuste("export", Some(&dir.to_string_lossy()));
                    cx.notify();
                    let _ = this;
                });
            }
        })
        .detach();
    }

    fn dialogo_ajustes(&self, cx: &mut Context<Self>) -> Div {
        let dir = Riomat::carpeta_export();
        let personalizada = leer_ajuste("export").is_some();
        self.capa(
            v_flex().gap_3().w(px(420.))
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Ajustes"))
                .child(v_flex().gap_1()
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("Carpeta de exportación"))
                    .child(div().text_size(px(12.5)).p_2().rounded_md().bg(rgb(BG)).border_1().border_color(rgb(BORDER)).child(dir.to_string_lossy().into_owned()))
                    .child(h_flex().gap_2()
                        .child(boton("aj_cambiar", "Cambiar…", ACCENT).on_click(cx.listener(|this, _, _, cx| this.elegir_carpeta_export(cx))))
                        .children(personalizada.then(|| boton("aj_defecto", "Restaurar", RESET).on_click(cx.listener(|_, _, _, cx| { guardar_ajuste("export", None); cx.notify(); }))))))
                .child(div().text_size(px(11.5)).text_color(rgb(MUTED)).child(format!("Los ajustes se guardan en {}", carpeta_config().join("ajustes.txt").display())))
                .child(boton("aj_ok", "Cerrar", ACCENT).on_click(cx.listener(|this, _, _, cx| { this.ajustes = false; cx.notify(); }))),
        )
    }

    fn dialogo_confirmar(&self, a: Accion, cx: &mut Context<Self>) -> Div {
        let a2 = a.clone();
        self.capa(
            v_flex().gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Hay resultados sin exportar"))
                .child(div().text_size(px(12.5)).text_color(rgb(MUTED)).child("Si continúas se descartan los resultados actuales. Puedes exportarlos antes con «Exportar todo»."))
                .child(h_flex().gap_2()
                    .child(boton("conf_si", "Continuar", ACCENT).on_click(cx.listener(move |this, _, window, cx| this.ejecutar(a2.clone(), window, cx))))
                    .child(boton("conf_no", "Cancelar", RESET).on_click(cx.listener(|this, _, _, cx| { this.confirmar = None; cx.notify(); })))),
        )
    }

    fn dialogo_acerca(&self, cx: &mut Context<Self>) -> Div {
        let fila = |k: &'static str, v: String| h_flex().gap_2().text_size(px(12.)).child(div().w(px(84.)).flex_none().text_color(rgb(MUTED)).child(k)).child(div().child(v));
        let sistema = format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH);
        self.capa(
            v_flex().gap_3().w(px(400.))
                .child(h_flex().gap_3().items_center()
                    .child(logo_tam(56.))
                    .child(v_flex()
                        .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child("riomat"))
                        .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(format!("versión {}", env!("CARGO_PKG_VERSION"))))))
                .child(div().text_size(px(12.5)).text_color(rgb(MUTED)).child("Transporte 1D de un trazador pasivo en ríos, directo desde los resultados de HEC-RAS."))
                .child(div().h(px(1.)).bg(rgb(BORDER)))
                .child(v_flex().gap_1()
                    .child(fila("Autoría", "Andrea Riveros".into()))
                    .child(fila("Colaboración", "nimudx".into()))
                    .child(fila("Caso", "Río Mantaro · Puente Breña (2524 m)".into()))
                    .child(fila("Modelo", "volúmenes finitos implícitos, 3 correlaciones de D_L".into()))
                    .child(fila("Licencia", "MIT".into()))
                    .child(fila("Sistema", sistema))
                    .child(fila("Código", "github.com/nimudx/riomat".into())))
                .child(div().h(px(1.)).bg(rgb(BORDER)))
                .child(div().text_size(px(11.5)).text_color(rgb(MUTED)).child("Alcance: trazador pasivo; D_L sin calibrar; pulso de entrada hipotético (T = 300 s)."))
                .child(div().text_size(px(11.5)).text_color(rgb(MUTED)).child("Atajos: Ctrl+N nuevo · Ctrl+O abrir · Ctrl+V pegar · Ctrl+E exportar todo"))
                .child(boton("acerca_ok", "Cerrar", ACCENT).on_click(cx.listener(|this, _, _, cx| { this.acerca = false; cx.notify(); }))),
        )
    }

    fn barra_superior(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let res = self.actual().unwrap();
        let (paso, _) = res.campo[self.frame];
        let t_min = paso as f64 * res.dt / 60.0;
        let total = res.t.last().copied().unwrap_or(0.0) / 60.0;
        let reproducir = boton("play", if self.playing { "⏸  Pausar" } else { "▶  Reproducir" }, RUN).on_click(cx.listener(|this, _, _, cx| this.alternar(cx)));
        let reiniciar = boton("reset", "↺  Reiniciar", RESET).on_click(cx.listener(|this, _, _, cx| {
            this.playing = false;
            this.generacion += 1;
            this.frame = 0;
            cx.notify();
        }));
        let vel = pastilla(format!("Velocidad {}×", VELOCIDADES[self.vel])).id("vel").cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
            this.vel = (this.vel + 1) % VELOCIDADES.len();
            cx.notify();
        }));
        let simulando = matches!(self.pestana, Pestana::Simulacion | Pestana::Comparar);
        let avisos = self.avisos_lista();
        let n_av = avisos.iter().filter(|(n, _, _)| !matches!(n, Nivel::Info)).count();
        let insignia = h_flex()
            .id("insignia")
            .gap_1p5()
            .h(px(28.))
            .px_3()
            .items_center()
            .rounded_full()
            .cursor_pointer()
            .border_1()
            .border_color(rgb(if n_av == 0 { OK } else { WARN }))
            .text_size(px(12.))
            .text_color(rgb(if n_av == 0 { OK } else { WARN }))
            .child(match (n_av == 0, self.ancho >= 1120.0) {
                (true, true) => "✓ Verificado".to_string(),
                (true, false) => "✓".to_string(),
                (false, true) => format!("⚠ {n_av} avisos"),
                (false, false) => format!("⚠ {n_av}"),
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.ver_avisos = !this.ver_avisos;
                cx.notify();
            }));
        let pestanas = h_flex().gap_1().children([(Pestana::Simulacion, "Simulación"), (Pestana::Figuras, "Figuras"), (Pestana::Datos, "Datos"), (Pestana::Comparar, "Comparar"), (Pestana::Sensibilidad, "Sensibilidad"), (Pestana::Auditoria, "Auditoría D_L")].into_iter().enumerate().map(|(i, (p, t))| {
            let activa = self.pestana == p;
            div().id(("pest", i)).h(px(48.)).px_3().flex().items_center().cursor_pointer().text_size(px(13.)).font_weight(if activa { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                .text_color(rgb(if activa { ACCENT } else { MUTED })).border_b_2().border_color(if activa { rgb(ACCENT) } else { rgb(PANEL) }).child(t)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.pestana = p;
                    if p == Pestana::Sensibilidad && this.sens.is_none() {
                        this.calcular_sens(cx);
                    }
                    if p == Pestana::Comparar && this.comp.is_none() {
                        this.calcular_comp();
                    }
                    cx.notify();
                }))
        }));
        h_flex()
            .h(px(48.))
            .flex_none()
            .items_center()
            .gap_3()
            .px_4()
            .bg(rgb(PANEL))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(h_flex().gap_2().items_center().child(logo()).children((self.ancho >= 1000.0).then(|| div().text_size(px(17.)).font_weight(FontWeight::BOLD).text_color(rgb(ACCENT)).child("riomat"))))
            .child(div().w(px(1.)).h(px(20.)).bg(rgb(BORDER)))
            .child(pestanas)
            .child(div().flex_1())
            .children(simulando.then(|| h_flex().gap_2().items_center().child(reproducir).children((self.ancho >= 1000.0).then(|| reiniciar)).children((self.ancho >= 1030.0).then(|| pastilla(format!("{t_min:.0} / {total:.0} min")))).children((self.ancho >= 1200.0).then(|| vel))))
            .child(insignia)
            .child(
                div().id("avatar").size(px(32.)).flex_none().flex().items_center().justify_center().rounded_full().cursor_pointer()
                    .bg(rgb(ACCENT)).text_color(rgb(0xffffff)).text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).child("AR")
                    .border_2().border_color(if self.menu { rgb(0xa6d6dc) } else { rgb(PANEL) })
                    .on_click(cx.listener(|this, _, _, cx| { this.menu = !this.menu; cx.notify(); })),
            )
    }

    fn panel_izq(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let mut perfiles = v_flex().gap_1p5();
        for (i, pf) in h.perfiles.iter().enumerate() {
            perfiles = perfiles.child(opcion(("pf", i), format!("{}    {:.1} m³/s", pf.nombre, pf.q), i == self.perfil, None).on_click(cx.listener(move |this, _, _, cx| this.set_perfil(i, cx))));
        }
        let mut metodos = v_flex().gap_1p5();
        for (i, m) in METODOS.iter().enumerate() {
            metodos = metodos.child(opcion(("m", i), m.nombre().into(), i == self.metodo, Some(color_metodo(*m))).on_click(cx.listener(move |this, _, _, cx| {
                this.metodo = i;
                this.frame = this.frame.min(this.n_frames().saturating_sub(1));
                cx.notify();
            })));
        }
        let mut durs = h_flex().gap_1p5().flex_wrap();
        for (i, d) in self.durs.iter().enumerate() {
            let dv = *d;
            durs = durs.child(opcion(("d", i), etiqueta_duracion(dv), i == self.dur, None).on_click(cx.listener(move |this, _, window, cx| {
                this.dur = i;
                this.comp = None;
                this.sens = None;
                this.frame = this.frame.min(this.n_frames().saturating_sub(1));
                this.poner(&this.in_dur.clone(), texto_num(dv), window, cx);
                cx.notify();
            })));
        }
        v_flex()
            .id("izq")
            .w(px(250.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .gap_5()
            .p_4()
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(v_flex().gap_2().child(seccion("Datos")).children(self.archivos.iter().map(|a| div().text_size(px(11.5)).text_color(rgb(MUTED)).truncate().child(a.clone()))).child(h_flex().gap_2().flex_wrap().child(boton("abrir2", "Abrir…", ACCENT).on_click(cx.listener(|this, _, window, cx| this.pedir(Accion::Abrir, window, cx)))).child(boton("pegar2", "Pegar", RESET).on_click(cx.listener(|this, _, window, cx| this.pedir(Accion::Pegar, window, cx)))).child(boton("nuevo", "Nuevo", RESET).on_click(cx.listener(|this, _, window, cx| this.pedir(Accion::Nuevo, window, cx))))))
            .child(v_flex().gap_2().child(seccion("Caudal")).child(perfiles))
            .child(v_flex().gap_2().child(seccion("Correlación D_L")).child(metodos))
            .child(
                v_flex()
                    .gap_2()
                    .child(seccion("Parámetros de simulación"))
                    .child(durs)
                    .child(self.campo("Masa", "u", &self.in_masa))
                    .child(self.campo("Pulso", "s", &self.in_dur))
                    .child(self.campo("dx", "m", &self.in_dx))
                    .child(self.campo("dt", "s", &self.in_dt))
                    .child(self.campo("Tiempo", "min", &self.in_tf))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(boton("calcular", "Calcular", ACCENT).on_click(cx.listener(|this, _, _, cx| this.calcular(cx))))
                            .child(boton("restablecer", "Por defecto", RESET).on_click(cx.listener(|this, _, window, cx| this.restablecer(window, cx)))),
                    )
            )
    }

    fn campo(&self, etiqueta: &'static str, unidad: &'static str, estado: &Entity<InputState>) -> Div {
        h_flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(div().text_size(px(12.)).text_color(rgb(TEXT)).child(etiqueta))
            .child(h_flex().items_center().gap_1().child(div().w(px(76.)).child(Input::new(estado))).child(div().w(px(28.)).text_size(px(11.)).text_color(rgb(DIM)).child(unidad)))
    }

    /// Perfil longitudinal vivo del río: roca, agua coloreada por el trazador, nivel de agua y línea de energía.
    fn esquema(&self) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let p = &h.perfiles[self.perfil];
        let res = self.actual().unwrap();
        let (paso, c) = &res.campo[self.frame];
        let t = *paso as f64 * res.dt;
        let n = p.x.len();
        let xmax = p.x[n - 1].max(1.0);
        let cmax = res.concentracion_max.max(1e-300);
        // Cotas: reales de HEC-RAS si están en la tabla; si no, un cauce plano con la profundidad A/B.
        let con_cotas = p.registros.iter().all(|r| r.cota_fondo.is_some() && r.nivel_agua.is_some());
        let bed: Vec<f64> = if con_cotas { p.registros.iter().map(|r| r.cota_fondo.unwrap()).collect() } else { vec![0.0; n] };
        let ws: Vec<f64> = if con_cotas { p.registros.iter().map(|r| r.nivel_agua.unwrap()).collect() } else { (0..n).map(|i| p.a[i] / p.b[i]).collect() };
        let eg: Option<Vec<f64>> = if p.registros.iter().all(|r| r.cota_energia.is_some()) { Some(p.registros.iter().map(|r| r.cota_energia.unwrap()).collect()) } else { None };
        let top = eg.as_ref().map(|e| e.iter().cloned().fold(f64::MIN, f64::max)).unwrap_or(0.0).max(ws.iter().cloned().fold(f64::MIN, f64::max));
        let bot_bed = bed.iter().cloned().fold(f64::MAX, f64::min);
        let (lo, hi) = (bot_bed - 0.25 * (top - bot_bed).max(1.0), top + 0.08 * (top - bot_bed).max(1.0));
        // Cortes de agua: color por concentración en cada punto del tramo
        const K: usize = 160;
        let cortes: Vec<(f64, f64, f64, f64, u32)> = (0..K)
            .map(|k| {
                let (x0, x1) = (xmax * k as f64 / K as f64, xmax * (k + 1) as f64 / K as f64);
                let xm = 0.5 * (x0 + x1);
                let col = rampa((interp(xm, &res.x, c) / cmax).max(0.0).sqrt());
                (x0, x1, interp(xm, &p.x, &bed), interp(xm, &p.x, &ws), col)
            })
            .collect();
        let pts = |v: &[f64]| -> Vec<(f64, f64)> { p.x.iter().cloned().zip(v.iter().cloned()).collect() };
        let (l_bed, l_ws, l_eg) = (pts(&bed), pts(&ws), eg.as_ref().map(|e| pts(e)));
        let lienzo = canvas(
            |_, _, _| (),
            move |b, _, window, _| {
                let (w, hh) = (f32::from(b.size.width), f32::from(b.size.height));
                let pos = |x: f64, e: f64| point(b.origin.x + rpx((x / xmax) as f32 * w), b.origin.y + rpx((1.0 - ((e - lo) / (hi - lo)) as f32) * hh));
                let fondo = |x: f64| point(b.origin.x + rpx((x / xmax) as f32 * w), b.origin.y + rpx(hh));
                // Roca
                let mut pb = PathBuilder::fill();
                pb.move_to(fondo(0.0));
                for &(x, e) in &l_bed {
                    pb.line_to(pos(x, e));
                }
                pb.line_to(fondo(xmax));
                pb.close();
                if let Ok(path) = pb.build() {
                    let col: Hsla = rgb(ROCA).into();
                    window.paint_path(path, col);
                }
                // Agua coloreada por el trazador
                for &(x0, x1, b0, w0, col) in &cortes {
                    let mut pb = PathBuilder::fill();
                    pb.move_to(pos(x0, b0));
                    pb.line_to(pos(x1 + xmax * 0.0015, b0));
                    pb.line_to(pos(x1 + xmax * 0.0015, w0));
                    pb.line_to(pos(x0, w0));
                    pb.close();
                    if let Ok(path) = pb.build() {
                        let c: Hsla = rgb(col).into();
                        window.paint_path(path, c);
                    }
                }
                let mut trazo = |linea: &[(f64, f64)], ancho: f32, color: u32| {
                    let mut pb = PathBuilder::stroke(px(ancho));
                    pb.move_to(pos(linea[0].0, linea[0].1));
                    for &(x, e) in &linea[1..] {
                        pb.line_to(pos(x, e));
                    }
                    if let Ok(path) = pb.build() {
                        let c: Hsla = rgb(color).into();
                        window.paint_path(path, c);
                    }
                };
                trazo(&l_bed, 1.6, ROCA_LINEA);
                if let Some(e) = &l_eg {
                    trazo(e, 1.2, ENERGIA);
                }
                trazo(&l_ws, 2.0, AGUA_LINEA);
                for &(x, e) in &l_ws {
                    let q = pos(x, e);
                    window.paint_quad(fill(Bounds::new(point(q.x - px(4.), q.y - px(4.)), size(px(8.), px(8.))), rgb(0xffffff)).corner_radii(px(4.)).border_widths(px(2.)).border_color(rgb(AGUA_LINEA)));
                }
            },
        )
        .size_full();
        // Etiquetas alternadas en dos filas para que no se pisen
        let etiqueta = |i: usize, texto: String, color: u32, peso: bool| {
            let f = (p.x[i] / xmax) as f32;
            let mut d = div().absolute().left(relative(f)).ml(px(-20.)).w(px(40.)).flex().justify_center().text_size(px(10.5)).text_color(rgb(color)).top(px(if i % 2 == 0 { 2. } else { 16. })).child(texto);
            if peso {
                d = d.font_weight(FontWeight::SEMIBOLD);
            }
            d
        };
        let sup = div().relative().w_full().h(px(32.)).children((0..n).map(|i| etiqueta(i, format!("{:.1}", interp(p.x[i], &res.x, c) * 1e6), TRAZ_ALTO, true)));
        let inf = div().relative().w_full().h(px(32.)).children((0..n).map(|i| etiqueta(i, format!("{:.0}", p.secciones[i]), MUTED, false)));
        let entrada_activa = t < self.durs[self.dur];
        let caja = |titulo: &'static str, l1: String, l2: String, color: u32| {
            v_flex().w(px(190.)).flex_none().gap_0p5().px_3().py_1p5().rounded_md().border_1().border_color(rgb(color)).bg(rgb(PANEL))
                .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(color)).child(titulo))
                .child(div().text_size(px(11.)).child(l1))
                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(l2))
        };
        let clave = |color: u32, texto: &'static str| h_flex().gap_1p5().items_center().child(div().size(px(10.)).rounded_sm().bg(rgb(color))).child(div().text_size(px(10.5)).text_color(rgb(MUTED)).child(texto));
        let perfil = v_flex()
            .flex_none()
            .w_full()
            .min_w_0()
            .px(px(20.))
            .child(sup)
            .child(div().w_full().h(px(180.)).rounded_sm().bg(rgb(PLOT_BG)).child(lienzo))
            .child(inf)
            .child(h_flex().gap_4().justify_center().child(clave(ROCA, "Roca")).child(clave(AGUA_LINEA, "Nivel del agua")).child(clave(TRAZ_MEDIO, "Agua con trazador")).children(eg.is_some().then(|| clave(ENERGIA, "Línea de energía"))));
        v_flex()
            .flex_none()
            .gap_2()
            .child(
                h_flex()
                    .flex_none()
                    .justify_between()
                    .child(caja("ENTRADA aguas arriba", format!("M = {:.1} en T = {:.0} s", res.masa, self.durs[self.dur]), if entrada_activa { "● inyectando".into() } else { "○ sin aporte".into() }, if entrada_activa { TRAZ_ALTO } else { DIM }))
                    .child(caja("SALIDA aguas abajo", format!("Q = {:.1} m³/s", p.q), format!("evacuado {:.1} %", res.salida[*paso] / res.masa * 100.0), ACCENT)),
            )
            .child(perfil)
    }

    fn centro(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let v = self.vista.as_ref().unwrap();
        let p = &h.perfiles[self.perfil];
        let res = self.actual().unwrap();
        let metodo = METODOS[self.metodo];
        let col = color_metodo(metodo);
        let (paso, _) = &res.campo[self.frame];
        let t_min = *paso as f64 * res.dt / 60.0;
        let leyenda_metodos: Vec<(String, u32)> = METODOS.iter().map(|m| (m.nombre().to_string(), color_metodo(*m))).collect();

        // Barra de tiempo
        let n = self.n_frames();
        let mut barra = h_flex().flex_none().w_full().h(px(20.)).gap(px(1.)).items_center();
        for s in 0..SEGMENTOS {
            let f = (s * (n - 1)) / (SEGMENTOS - 1);
            let pasado = f <= self.frame;
            barra = barra.child(div().id(("seg", s)).flex_1().h(if pasado { px(12.) } else { px(7.) }).rounded_sm().cursor_pointer().bg(if pasado { rgb(ACCENT) } else { rgb(0xd5e0e8) }).on_click(cx.listener(move |this, _, _, cx| {
                this.frame = f;
                cx.notify();
            })));
        }

        let tile = |etq: &str, val: String| {
            v_flex().flex_1().gap_0p5().px_3().py_2().rounded_lg().bg(rgb(PANEL)).border_1().border_color(rgb(BORDER)).min_w_0()
                .child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child(val))
                .child(div().text_size(px(11.5)).text_color(rgb(MUTED)).child(etq.to_string()))
        };
        let dmin = v.d[self.metodo].iter().cloned().fold(f64::INFINITY, f64::min);
        let dmax = v.d[self.metodo].iter().cloned().fold(0.0, f64::max);
        let tiles = h_flex()
            .flex_none()
            .gap_3()
            .child(tile("t50: mitad de la masa evacuada", fmt_min(res.t50_min)))
            .child(tile("t95: 95 % evacuado", fmt_min(res.t95_min)))
            .child(tile("D_L mín – máx (m²/s)", format!("{dmin:.0} – {dmax:.0}")))
            .child(tile(&format!("Evacuado en {:.1} h", res.t.last().copied().unwrap_or(0.0) / 3600.0), format!("{:.2} %", res.fraccion_salida * 100.0)));

        // Gráficas: se construyen desde las mismas figuras que se exportan.
        let figs = self.figuras().unwrap_or_default();
        let dibujar = |i: usize| {
            let f = figs[i].clone();
            grafica(f.series, f.marcas, f.ejes, 300.0)
        };
        let (g1, g2) = (dibujar(0), dibujar(1));

        v_flex()
            .id("centro")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .gap_3()
            .p_4()
            .child(tarjeta("Perfil longitudinal del río", vec![], v_flex().gap_2().child(self.esquema()).child(barra)))
            .child(tiles)
            .child(h_flex().flex_none().gap_3().items_start().child(div().flex_1().min_w_0().child(tarjeta("Concentración a lo largo del tramo", vec![(metodo.nombre().to_string(), col)], g1))).child(div().flex_1().min_w_0().child(tarjeta("Masa evacuada acumulada", leyenda_metodos, g2))))
    }

    /// Datos de las cuatro gráficas del estado actual (los mismos que se dibujan en pantalla y se exportan).
    fn figuras(&self) -> Option<Vec<Figura>> {
        let h = self.hid.as_ref()?;
        let v = self.vista.as_ref()?;
        let p = &h.perfiles[self.perfil];
        let res = self.actual()?;
        let metodo = METODOS[self.metodo];
        let col = color_metodo(metodo);
        let (paso, _) = &res.campo[self.frame];
        let t_min = *paso as f64 * res.dt / 60.0;
        let contexto = format!("{} · Q = {:.1} m³/s · {}", p.nombre, p.q, metodo.nombre());
        let s = |n: &str, puntos: Vec<(f64, f64)>, color: u32, grosor: f32, relleno: bool, opaca: bool| Serie { nombre: n.to_string(), puntos, color, grosor, relleno, opaca };
        let mut figs = Vec::new();

        // 1) Concentración a lo largo del tramo
        let cmax = res.campo[self.frame].1.iter().cloned().fold(0.0, f64::max) * 1e6;
        // Si el instante está vacío (t = 0) se usa el máximo de toda la simulación para que el eje tenga sentido.
        let cref = if cmax > 1e-9 { cmax } else { res.concentracion_max * 1e6 };
        let ytop = techo_bonito((cref * 1.1).max(1e-6));
        let pts: Vec<(f64, f64)> = res.x.iter().zip(&res.campo[self.frame].1).map(|(x, c)| (x / 1000.0, c * 1e6)).collect();
        let xmax = res.x.last().copied().unwrap_or(1.0) / 1000.0;
        figs.push(Figura {
            mapa: None,
            nombre: format!("concentracion_t{:.0}min", t_min),
            titulo: format!("Concentración a t = {t_min:.0} min · {contexto}"),
            series: vec![s(metodo.nombre(), pts, col, 2.2, true, true)],
            marcas: vec![],
            ejes: Ejes { x: (0.0, xmax), y: (0.0, ytop), x_log: false, y_log: false, x_ticks: ticks_lineales(0.0, xmax, 6), y_ticks: ticks_lineales(0.0, ytop, 5), x_titulo: "Distancia aguas abajo (km)".into(), y_titulo: "Concentración (×10⁻⁶ masa/m³) · escala automática".into() },
        });

        // 2) Masa evacuada acumulada
        let mut series2 = vec![];
        for (mi, m) in METODOS.iter().enumerate() {
            let r = &v.sims[&(mi, self.dur)];
            let puntos: Vec<(f64, f64)> = (0..r.t.len()).step_by(4).map(|j| (r.t[j] / 60.0, r.salida[j] / r.masa)).collect();
            series2.push(s(m.nombre(), puntos, color_metodo(*m), if mi == self.metodo { 2.6 } else { 1.4 }, false, mi == self.metodo));
        }
        let tfin = res.t.last().copied().unwrap_or(1.0) / 60.0;
        figs.push(Figura {
            mapa: None,
            nombre: "masa_evacuada".into(),
            titulo: format!("Masa evacuada acumulada · {} · Q = {:.1} m³/s · pulso de {}", p.nombre, p.q, etiqueta_duracion(self.durs[self.dur])),
            series: series2,
            marcas: vec![Marca { x: t_min, y: res.salida[*paso] / res.masa, color: col }],
            ejes: Ejes { x: (0.0, tfin), y: (0.0, 1.0), x_log: false, y_log: false, x_ticks: ticks_lineales(0.0, tfin, 5), y_ticks: ticks_lineales(0.0, 1.0, 5), x_titulo: "Tiempo (min)".into(), y_titulo: "Fracción evacuada".into() },
        });

        // 3) D_L por sección (escala logarítmica)
        let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
        for d in &v.d {
            for &x in d {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        let (k0, k1) = (lo.log10().floor() as i32, hi.log10().ceil() as i32);
        let lmax = p.x.last().copied().unwrap_or(1.0) / 1000.0;
        let series3: Vec<Serie> = METODOS.iter().enumerate().map(|(mi, m)| s(m.nombre(), p.x.iter().map(|x| x / 1000.0).zip(v.d[mi].iter().cloned()).collect(), color_metodo(*m), if mi == self.metodo { 2.6 } else { 1.4 }, false, mi == self.metodo)).collect();
        figs.push(Figura {
            mapa: None,
            nombre: "dispersion_DL".into(),
            titulo: format!("Coeficiente de dispersión D_L por sección · {} · Q = {:.1} m³/s", p.nombre, p.q),
            series: series3,
            marcas: vec![],
            ejes: Ejes { x: (0.0, lmax), y: (10f64.powi(k0), 10f64.powi(k1)), x_log: false, y_log: true, x_ticks: ticks_lineales(0.0, lmax, 6), y_ticks: (k0..=k1).map(|k| 10f64.powi(k)).collect(), x_titulo: "Distancia aguas abajo (km)".into(), y_titulo: "D_L (m²/s)".into() },
        });

        // 4) Sensibilidad a la duración del pulso
        let mut series4 = vec![];
        let mut tmax = 0.0_f64;
        for (mi, m) in METODOS.iter().enumerate() {
            for sel in [true, false] {
                let puntos: Vec<(f64, f64)> = self.durs.iter().enumerate().filter_map(|(di, d)| {
                    let r = &v.sims[&(mi, di)];
                    let y = if sel { r.t50_min } else { r.t95_min }?;
                    tmax = tmax.max(y);
                    Some((*d, y))
                }).collect();
                series4.push(s(if sel { m.nombre() } else { "" }, puntos, color_metodo(*m), if mi == self.metodo { 2.4 } else { 1.2 }, false, mi == self.metodo));
            }
        }
        let ytop4 = techo_bonito(tmax * 1.05);
        figs.push(Figura {
            mapa: None,
            nombre: "sensibilidad_pulso".into(),
            titulo: format!("Sensibilidad a la duración del pulso · {} · Q = {:.1} m³/s", p.nombre, p.q),
            series: series4,
            marcas: res.t50_min.map(|y| vec![Marca { x: self.durs[self.dur], y, color: col }]).unwrap_or_default(),
            ejes: Ejes { x: (self.durs[0], *self.durs.last().unwrap()), y: (0.0, ytop4), x_log: true, y_log: false, x_ticks: self.durs.clone(), y_ticks: ticks_lineales(0.0, ytop4, 5), x_titulo: "Duración del pulso (s) · t50 abajo, t95 arriba".into(), y_titulo: "Tiempo (min)".into() },
        });
        Some(figs)
    }

    fn carpeta_export() -> PathBuf {
        if let Some(p) = leer_ajuste("export") {
            return PathBuf::from(p);
        }
        let base = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("Documents").join("riomat-export")
    }

    /// Mapa espacio-tiempo: color = concentración (escala √ para ver la cola del pulso). Se simula con un fotograma por minuto
    /// y el eje de tiempo se recorta a ~1.4 × t95 (después ya no queda trazador en el tramo).
    fn figura_mapa(&self) -> Option<Figura> {
        let h = self.hid.as_ref()?;
        let v = self.vista.as_ref()?;
        let p = &h.perfiles[self.perfil];
        let metodo = METODOS[self.metodo];
        let par = Parametros { duracion_pulso: self.durs[self.dur], campo_cada: Some(((60.0 / self.par.dt).round() as usize).max(1)), ..self.par.clone() };
        let res = simular_entrada(p.q, &p.x, &p.a, &v.d[self.metodo], &par).ok()?;
        let corte = self.actual()?.t95_min.map(|t| t * 1.4 * 60.0).unwrap_or(f64::INFINITY);
        let frames: Vec<&(usize, Vec<f64>)> = res.campo.iter().filter(|(paso, _)| (*paso as f64 * res.dt) <= corte).collect();
        if frames.len() < 2 {
            return None;
        }
        let nxm = res.x.len().min(160);
        let cmax = res.concentracion_max.max(1e-300);
        let mut colores = Vec::with_capacity(nxm * frames.len());
        for (_, c) in &frames {
            for k in 0..nxm {
                let i = (((k as f64 + 0.5) * res.x.len() as f64 / nxm as f64) as usize).min(c.len() - 1);
                colores.push(rampa((c[i] / cmax).max(0.0).sqrt()));
            }
        }
        let tfin_m = frames.last().map(|(p, _)| *p as f64 * res.dt / 60.0).unwrap_or(1.0).max(1.0);
        let xmax = res.x.last().copied().unwrap_or(1.0) / 1000.0;
        Some(Figura {
            nombre: "mapa_espacio_tiempo".into(),
            titulo: format!("Mapa espacio-tiempo de la concentración · {} · Q = {:.1} m³/s · {} · pulso de {}", p.nombre, p.q, metodo.nombre(), etiqueta_duracion(self.durs[self.dur])),
            series: vec![],
            marcas: vec![],
            mapa: Some(riomat_plot::Mapa {
                nx: nxm,
                ny: frames.len(),
                colores,
                barra: (0..24).map(|k| rampa(k as f64 / 23.0)).collect(),
                barra_max: format!("{:.1} ×10⁻⁶", cmax * 1e6),
                barra_titulo: "Color ∝ √C (masa/m³)".into(),
            }),
            ejes: Ejes { x: (0.0, xmax), y: (0.0, tfin_m), x_log: false, y_log: false, x_ticks: ticks_lineales(0.0, xmax, 6), y_ticks: ticks_lineales(0.0, tfin_m, 5), x_titulo: "Distancia aguas abajo (km)".into(), y_titulo: "Tiempo (min)".into() },
        })
    }

    /// Todas las figuras exportables: las cinco de la pestaña Figuras y las tres de la comparación.
    fn todas_las_figuras(&mut self) -> Option<Vec<Figura>> {
        if self.comp.is_none() {
            self.calcular_comp();
        }
        let mut f = self.figuras_con_mapa()?;
        f.extend(self.figuras_comparar().unwrap_or_default());
        Some(f)
    }

    /// Genera el informe PDF de una página en `~/Documents/riomat-export/`. Corre en segundo plano.
    fn exportar_informe(&mut self, cx: &mut Context<Self>) {
        let (Some(figs), Some(h)) = (self.todas_las_figuras(), self.hid.clone()) else { return };
        if self.exportando {
            return;
        }
        self.exportando = true;
        self.export_msg = Some("Generando informe…".into());
        let par = Parametros { duracion_pulso: self.durs[self.dur], campo_cada: None, ..self.par.clone() };
        let (perfil, metodo, archivos) = (h.perfiles[self.perfil].nombre.clone(), METODOS[self.metodo], self.archivos.clone());
        let verif = self.verificacion_texto();
        let marca = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let ruta = Self::carpeta_export().join(format!("informe-{marca}.pdf"));
        let tarea = cx.background_executor().spawn(async move {
            let r = ruta.clone();
            let res = std::panic::catch_unwind(move || -> Result<(), String> {
                let esc = exportar::escenarios(&h, &par)?;
                let inf = armar_informe(&h, &par, &esc, &figs, &verif, &archivos, &perfil, metodo);
                let pdf = riomat_plot::svg_a_pdf(&riomat_plot::informe_svg(&inf))?;
                if let Some(d) = r.parent() {
                    std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
                }
                std::fs::write(&r, pdf).map_err(|e| format!("{}: {e}", r.display()))
            })
            .unwrap_or_else(|_| Err("fallo interno al generar el informe".into()));
            (res, ruta)
        });
        cx.spawn(async move |this, cx| {
            let (r, ruta) = tarea.await;
            let _ = this.update(cx, |this, cx| {
                this.exportando = false;
                this.export_msg = Some(match r {
                    Ok(()) => format!("Informe guardado: {}", ruta.display()),
                    Err(e) => format!("No se pudo generar el informe: {e}"),
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn verificacion_texto(&self) -> Vec<exportar::Verificacion> {
        self.avisos_lista().into_iter().map(|(n, c, m)| (match n { Nivel::Info => "info", Nivel::Aviso => "aviso", Nivel::Error => "error" }.to_string(), c.to_string(), m)).collect()
    }

    /// Las figuras de la pestaña Figuras: las cuatro gráficas y el mapa espacio-tiempo.
    fn figuras_con_mapa(&self) -> Option<Vec<Figura>> {
        let mut f = self.figuras()?;
        f.extend(self.figura_mapa());
        Some(f)
    }

    /// Guarda una sola figura (`formato`: "png", "svg" o "pdf") en la carpeta de exportación, sin bloquear la ventana.
    fn guardar_figura(&mut self, idx: usize, formato: &'static str, cx: &mut Context<Self>) {
        let Some(f) = self.figuras_con_mapa().and_then(|v| v.into_iter().nth(idx)) else { return };
        let raiz = Self::carpeta_export();
        self.export_msg = Some("Guardando…".into());
        let tarea = cx.background_executor().spawn(async move {
            std::panic::catch_unwind(move || -> Result<PathBuf, String> {
                std::fs::create_dir_all(&raiz).map_err(|e| format!("{}: {e}", raiz.display()))?;
                let svg = a_svg(&f, 720.0, 380.0);
                let (bytes, ext) = match formato {
                    "svg" => (svg.into_bytes(), "svg"),
                    "pdf" => (svg_a_pdf(&svg)?, "pdf"),
                    _ => (svg_a_png(&svg, 3.0)?, "png"),
                };
                let destino = raiz.join(format!("{}.{ext}", f.nombre));
                std::fs::write(&destino, bytes).map_err(|e| e.to_string())?;
                Ok(destino)
            })
            .unwrap_or_else(|_| Err("fallo interno al generar la imagen".into()))
        });
        cx.spawn(async move |this, cx| {
            let r = tarea.await;
            let _ = this.update(cx, |this, cx| {
                this.export_msg = Some(match r {
                    Ok(p) => format!("Guardado: {}", p.display()),
                    Err(e) => format!("No se pudo guardar: {e}"),
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Pestaña Figuras: lista, vista grande de la elegida y botones de descarga.
    fn vista_figuras(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let figs = self.figuras_con_mapa().unwrap_or_default();
        let sel = self.fig_sel.min(figs.len().saturating_sub(1));
        let titulos = ["Concentración en el tramo", "Masa evacuada acumulada", "Dispersión D_L por sección", "Sensibilidad al pulso", "Mapa espacio-tiempo"];
        let lista = v_flex().w(px(210.)).flex_none().gap_1p5().children(figs.iter().enumerate().map(|(i, _)| {
            opcion(("fig", i), titulos.get(i).copied().unwrap_or("Figura").to_string(), i == sel, None).on_click(cx.listener(move |this, _, _, cx| {
                this.fig_sel = i;
                cx.notify();
            }))
        }));
        let Some(f) = figs.get(sel).cloned() else { return div().flex_1().into_any_element() };
        let leyenda: Vec<(String, u32)> = f.series.iter().filter(|s| !s.nombre.is_empty()).map(|s| (s.nombre.clone(), s.color)).collect();
        let vista = tarjeta("Vista previa", leyenda, v_flex().gap_2().child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).child(f.titulo.clone())).child(if f.mapa.is_some() { mapa_calor(f.clone(), 420.0).into_any_element() } else { grafica(f.series, f.marcas, f.ejes, 420.0).into_any_element() }));
        let botones = h_flex().gap_2().items_center()
            .child(boton("g_png", "PNG", ACCENT).on_click(cx.listener(move |this, _, _, cx| this.guardar_figura(sel, "png", cx))))
            .child(boton("g_svg", "SVG", RESET).on_click(cx.listener(move |this, _, _, cx| this.guardar_figura(sel, "svg", cx))))
            .child(boton("g_pdf", "PDF", RESET).on_click(cx.listener(move |this, _, _, cx| this.guardar_figura(sel, "pdf", cx))))
            .child(boton("g_informe", "Informe PDF", ACCENT).on_click(cx.listener(|this, _, _, cx| this.exportar_informe(cx))))
            .child(boton("g_todo", "Exportar todo", ACCENT).on_click(cx.listener(|this, _, _, cx| this.exportar_todo(cx))))
            .children(self.export_msg.clone().map(|m| div().text_size(px(11.5)).text_color(rgb(MUTED)).child(m)));
        v_flex()
            .id("figuras")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .gap_3()
            .p_4()
            .child(h_flex().flex_none().gap_4().items_start().child(lista).child(v_flex().flex_1().min_w_0().gap_3().child(vista).child(botones)))
            .into_any_element()
    }

    /// Lee los factores, corre los 45 ensayos y guarda el resultado.
    fn calcular_sens(&mut self, cx: &mut Context<Self>) {
        let Some(h) = self.hid.clone() else { return };
        let kd = self.leer_campo(&self.in_kd, "Factor de D_L", cx);
        let pa = self.leer_campo(&self.in_pa, "Variación del área", cx);
        let r = (|| -> Result<Vec<analisis::FilaSens>, String> {
            let (kd, pa) = (kd?, pa?);
            if !(kd > 1.0) {
                return Err("El factor de D_L debe ser mayor que 1 (por ejemplo 2: ensaya ×0.5 y ×2).".into());
            }
            if !(pa > 0.0 && pa < 100.0) {
                return Err("La variación del área debe estar entre 0 y 100 %.".into());
            }
            let par = Parametros { duracion_pulso: self.durs[self.dur], campo_cada: None, ..self.par.clone() };
            analisis::sensibilidad(&h, &par, &analisis::ensayos(kd, pa), &self.criticas)
        })();
        match r {
            Ok(f) => {
                self.sens = Some(f);
                self.sens_error = None;
            }
            Err(e) => {
                self.sens = None;
                self.sens_error = Some(e);
            }
        }
        cx.notify();
    }

    /// Simula los tres caudales × tres correlaciones con la duración de pulso elegida.
    fn calcular_comp(&mut self) {
        let Some(h) = &self.hid else { return };
        let par = Parametros { duracion_pulso: self.durs[self.dur], campo_cada: Some(CAMPO_CADA), ..self.par.clone() };
        let (mut sims, mut d) = (HashMap::new(), HashMap::new());
        for (pi, p) in h.perfiles.iter().enumerate() {
            for (mi, m) in METODOS.iter().enumerate() {
                let dl = coeficientes(*m, &p.u, &p.a, &p.b, &p.r, &p.sf);
                match simular_entrada(p.q, &p.x, &p.a, &dl, &par) {
                    Ok(r) => {
                        sims.insert((pi, mi), r);
                        d.insert((pi, mi), dl);
                    }
                    Err(e) => self.error = Some(format!("{} / {}: {e}", p.nombre, m.nombre())),
                }
            }
        }
        self.comp = Some(Comparacion { sims, d });
    }

    /// Figuras de la comparación entre caudales para la correlación elegida (misma fuente para pantalla y exportación).
    fn figuras_comparar(&self) -> Option<Vec<Figura>> {
        let h = self.hid.as_ref()?;
        let c = self.comp.as_ref()?;
        let m = METODOS[self.metodo];
        let tono = [0x8bb6d9_u32, 0x3f7fbf, 0x14325e];
        let s = |n: String, puntos: Vec<(f64, f64)>, color: u32| Serie { nombre: n, puntos, color, grosor: 2.4, relleno: false, opaca: true };
        let ctx = format!("{} · pulso de {}", m.nombre(), etiqueta_duracion(self.durs[self.dur]));
        let nombre = |pi: usize| format!("{} ({:.1} m³/s)", h.perfiles[pi].nombre, h.perfiles[pi].q);
        let mut figs = Vec::new();

        // 1) Masa evacuada
        let mut s1 = vec![];
        let mut tfin = 1.0_f64;
        for pi in 0..h.perfiles.len() {
            let r = c.sims.get(&(pi, self.metodo))?;
            tfin = tfin.max(r.t.last().copied().unwrap_or(1.0) / 60.0);
            s1.push(s(nombre(pi), (0..r.t.len()).step_by(4).map(|j| (r.t[j] / 60.0, r.salida[j] / r.masa)).collect(), tono[pi % 3]));
        }
        figs.push(Figura {
            mapa: None,
            nombre: "comparacion_masa_evacuada".into(),
            titulo: format!("Masa evacuada acumulada por caudal · {ctx}"),
            series: s1, marcas: vec![],
            ejes: Ejes { x: (0.0, tfin), y: (0.0, 1.0), x_log: false, y_log: false, x_ticks: ticks_lineales(0.0, tfin, 5), y_ticks: ticks_lineales(0.0, 1.0, 5), x_titulo: "Tiempo (min)".into(), y_titulo: "Fracción evacuada".into() },
        });

        // 2) Concentración a lo largo del tramo en el mismo instante
        let mut s2 = vec![];
        let (mut ymax, mut xmax, mut t_min) = (0.0_f64, 1.0_f64, 0.0_f64);
        for pi in 0..h.perfiles.len() {
            let r = c.sims.get(&(pi, self.metodo))?;
            let f = self.frame.min(r.campo.len().saturating_sub(1));
            let (paso, cc) = &r.campo[f];
            t_min = *paso as f64 * r.dt / 60.0;
            xmax = xmax.max(r.x.last().copied().unwrap_or(1.0) / 1000.0);
            let pts: Vec<(f64, f64)> = r.x.iter().zip(cc).map(|(x, v)| (x / 1000.0, v * 1e6)).collect();
            ymax = pts.iter().fold(ymax, |m, p| m.max(p.1));
            s2.push(s(nombre(pi), pts, tono[pi % 3]));
        }
        if ymax < 1e-9 {
            ymax = c.sims.values().map(|r| r.concentracion_max * 1e6).fold(0.0, f64::max);
        }
        let ytop = techo_bonito((ymax * 1.1).max(1e-6));
        figs.push(Figura {
            mapa: None,
            nombre: format!("comparacion_concentracion_t{:.0}min", t_min),
            titulo: format!("Concentración a t = {t_min:.0} min por caudal · {ctx}"),
            series: s2, marcas: vec![],
            ejes: Ejes { x: (0.0, xmax), y: (0.0, ytop), x_log: false, y_log: false, x_ticks: ticks_lineales(0.0, xmax, 6), y_ticks: ticks_lineales(0.0, ytop, 5), x_titulo: "Distancia aguas abajo (km)".into(), y_titulo: "Concentración (×10⁻⁶ masa/m³)".into() },
        });

        // 3) D_L por sección
        let mut s3 = vec![];
        let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
        for (pi, p) in h.perfiles.iter().enumerate() {
            let d = c.d.get(&(pi, self.metodo))?;
            lo = d.iter().fold(lo, |m, v| m.min(*v));
            hi = d.iter().fold(hi, |m, v| m.max(*v));
            s3.push(s(nombre(pi), p.x.iter().map(|x| x / 1000.0).zip(d.iter().cloned()).collect(), tono[pi % 3]));
        }
        let (k0, k1) = (lo.log10().floor() as i32, hi.log10().ceil() as i32);
        let lmax = h.perfiles[0].x.last().copied().unwrap_or(1.0) / 1000.0;
        figs.push(Figura {
            mapa: None,
            nombre: "comparacion_dispersion_DL".into(),
            titulo: format!("D_L por sección y caudal · {}", m.nombre()),
            series: s3, marcas: vec![],
            ejes: Ejes { x: (0.0, lmax), y: (10f64.powi(k0), 10f64.powi(k1)), x_log: false, y_log: true, x_ticks: ticks_lineales(0.0, lmax, 6), y_ticks: (k0..=k1).map(|k| 10f64.powi(k)).collect(), x_titulo: "Distancia aguas abajo (km)".into(), y_titulo: "D_L (m²/s)".into() },
        });
        Some(figs)
    }

    /// Pestaña Comparar: los tres caudales en una sola pantalla, con la correlación y el pulso elegidos.
    fn vista_comparar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let base = v_flex().id("comp").flex_1().h_full().min_w_0().overflow_y_scroll().gap_3().p_4();
        let (Some(h), Some(c), Some(figs)) = (self.hid.as_ref(), self.comp.as_ref(), self.figuras_comparar()) else {
            return base.child(div().text_size(px(13.)).text_color(rgb(MUTED)).child("Calculando…")).into_any_element();
        };
        let m = METODOS[self.metodo];
        let tono = [0x8bb6d9_u32, 0x3f7fbf, 0x14325e];
        // Una cifra por caudal
        let cartas = h_flex().flex_none().gap_3().children(h.perfiles.iter().enumerate().map(|(pi, p)| {
            let r = &c.sims[&(pi, self.metodo)];
            let d = &c.d[&(pi, self.metodo)];
            let (dmin, dmax) = (d.iter().cloned().fold(f64::INFINITY, f64::min), d.iter().cloned().fold(0.0, f64::max));
            v_flex().flex_1().min_w_0().gap_1().px_3().py_2().rounded_lg().bg(rgb(PANEL)).border_1().border_color(rgb(BORDER))
                .child(h_flex().gap_2().items_center().child(div().size(px(10.)).rounded_sm().bg(rgb(tono[pi % 3]))).child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).child(format!("{} · {:.1} m³/s", p.nombre, p.q))))
                .child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child(format!("t50 {}", fmt_min(r.t50_min))))
                .child(div().text_size(px(11.5)).text_color(rgb(MUTED)).child(format!("t95 {} · D_L {dmin:.0}–{dmax:.0} m²/s · evacuado {:.1} %", fmt_min(r.t95_min), r.fraccion_salida * 100.0)))
        }));
        let leyenda: Vec<(String, u32)> = h.perfiles.iter().enumerate().map(|(pi, p)| (format!("{} ({:.1} m³/s)", p.nombre, p.q), tono[pi % 3])).collect();
        let dib = |i: usize| {
            let f = figs[i].clone();
            grafica(f.series, f.marcas, f.ejes, 260.0)
        };
        // Tabla de los nueve escenarios
        let filas: Vec<Vec<String>> = h.perfiles.iter().enumerate().flat_map(|(pi, p)| {
            METODOS.iter().enumerate().map(move |(mi, mm)| (pi, p, mi, mm))
        }).map(|(pi, p, mi, mm)| {
            let r = &c.sims[&(pi, mi)];
            vec![format!("{} ({:.1})", p.nombre, p.q), mm.nombre().to_string(), fmt_min(r.t50_min), fmt_min(r.t95_min), format!("{:.2} %", r.fraccion_salida * 100.0)]
        }).collect();
        base
            .child(div().text_size(px(12.5)).text_color(rgb(MUTED)).child(format!("Correlación: {} · pulso de {} · instante {:.0} min (barra de la pestaña Simulación)", m.nombre(), etiqueta_duracion(self.durs[self.dur]), c.sims[&(0, self.metodo)].campo[self.frame.min(c.sims[&(0, self.metodo)].campo.len() - 1)].0 as f64 * c.sims[&(0, self.metodo)].dt / 60.0)))
            .child(cartas)
            .child(h_flex().flex_none().gap_3().items_start()
                .child(div().flex_1().min_w_0().child(tarjeta("Masa evacuada acumulada", leyenda.clone(), dib(0))))
                .child(div().flex_1().min_w_0().child(tarjeta("Concentración en el tramo", leyenda.clone(), dib(1)))))
            .child(tarjeta("Dispersión D_L por sección", leyenda, dib(2)))
            .child(tarjeta("Los nueve escenarios (min)", vec![], tabla_txt(&["Caudal (m³/s)", "Correlación", "t50", "t95", "Evacuado"], filas)))
            .into_any_element()
    }

    /// Pestaña Sensibilidad: ensayos sobre D_L y el área de las secciones críticas.
    fn vista_sensibilidad(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let p0 = &h.perfiles[0];
        let chips = h_flex().gap_1p5().flex_wrap().children(p0.secciones.iter().enumerate().map(|(i, s)| {
            let s = *s;
            opcion(("crit", i), format!("{s:.0}"), self.criticas.contains(&s), None).on_click(cx.listener(move |this, _, _, cx| {
                if let Some(k) = this.criticas.iter().position(|c| *c == s) {
                    this.criticas.remove(k);
                } else {
                    this.criticas.push(s);
                }
                this.sens = None;
                cx.notify();
            }))
        }));
        let config = tarjeta("Ensayos", vec![], v_flex().gap_3()
            .child(h_flex().gap_6().items_center().flex_wrap()
                .child(self.campo("D_L ×(1/f) y ×f, f =", "", &self.in_kd))
                .child(self.campo("Área ±", "%", &self.in_pa))
                .child(boton("calc_sens", "Calcular", ACCENT).on_click(cx.listener(|this, _, _, cx| this.calcular_sens(cx)))))
            .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("Secciones críticas (el área se cambia solo ahí):"))
            .child(chips)
            .children(self.sens_error.clone().map(|e| div().text_size(px(12.)).text_color(rgb(ERR)).child(e))));
        let base = v_flex().id("sens").flex_1().h_full().min_w_0().overflow_y_scroll().gap_3().p_4().child(config);
        let Some(filas) = &self.sens else {
            return base.child(div().text_size(px(13.)).text_color(rgb(MUTED)).child("Pulsa Calcular para correr los ensayos (3 caudales × 3 correlaciones × 5 ensayos).")).into_any_element();
        };
        let perfil = &h.perfiles[self.perfil].nombre;
        let ens: Vec<analisis::Ensayo> = filas.iter().filter(|f| &f.perfil == perfil && f.metodo == METODOS[0]).map(|f| f.ensayo.clone()).collect();
        let fmtv = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
        let fmtp = |v: Option<f64>| v.map(|x| format!("{x:+.1} %")).unwrap_or("–".into());
        let cab: Vec<String> = std::iter::once("Ensayo".to_string()).chain(METODOS.iter().map(|m| format!("{} · t50 / t95 (Δt95)", m.nombre()))).collect();
        let filas_tabla: Vec<Vec<String>> = ens.iter().map(|e| {
            std::iter::once(e.etiqueta()).chain(METODOS.iter().map(|m| {
                filas.iter().find(|f| &f.perfil == perfil && f.metodo == *m && f.ensayo == *e).map(|f| format!("{} / {} ({})", fmtv(f.t50_min), fmtv(f.t95_min), fmtp(f.d_t95_pct))).unwrap_or_default()
            })).collect()
        }).collect();
        let series: Vec<Serie> = METODOS.iter().map(|m| Serie {
            nombre: m.nombre().to_string(),
            puntos: ens.iter().enumerate().filter_map(|(i, e)| filas.iter().find(|f| &f.perfil == perfil && f.metodo == *m && f.ensayo == *e).and_then(|f| f.d_t95_pct).map(|y| ((i + 1) as f64, y))).collect(),
            color: color_metodo(*m), grosor: 2.0, relleno: false, opaca: true,
        }).collect();
        let (mut lo, mut hi) = (0.0_f64, 0.0_f64);
        for s in &series { for (_, y) in &s.puntos { lo = lo.min(*y); hi = hi.max(*y); } }
        let pad = ((hi - lo) * 0.1).max(1.0);
        let (lo, hi) = (lo - pad, hi + pad);
        let leyenda: Vec<(String, u32)> = METODOS.iter().map(|m| (m.nombre().to_string(), color_metodo(*m))).collect();
        let etiquetas = ens.iter().enumerate().map(|(i, e)| format!("{} {}", i + 1, e.etiqueta())).collect::<Vec<_>>().join(" · ");
        let g = grafica(series, vec![], Ejes { x: (1.0, ens.len() as f64), y: (lo, hi), x_log: false, y_log: false, x_ticks: (1..=ens.len()).map(|i| i as f64).collect(), y_ticks: ticks_lineales(lo, hi, 5), x_titulo: etiquetas, y_titulo: "Cambio en t95 respecto a la referencia (%)".into() }, 280.0);
        base
            .child(tarjeta("Cambio en t95 por ensayo", leyenda, g))
            .child(tarjeta("Tiempos por ensayo (min)", vec![], tabla_txt(&cab.iter().map(|s| s.as_str()).collect::<Vec<_>>(), filas_tabla)))
            .into_any_element()
    }

    /// Pestaña Auditoría: resumen de D_L por escenario y los 12 valores mayores.
    fn vista_auditoria(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let aud = analisis::auditoria(h, &self.criticas);
        let n_mal = aud.iter().filter(|a| !a.d.is_finite()).count();
        let res = analisis::resumen_auditoria(&aud);
        let resumen = tabla_txt(&["Perfil", "Correlación", "D_L mín", "Mediana", "D_L máx", "Máx / mín"], res.iter().map(|r| vec![r.perfil.clone(), r.metodo.nombre().into(), format!("{:.1}", r.d_min), format!("{:.1}", r.d_mediana), format!("{:.1}", r.d_max), format!("{:.1}", r.ratio)]).collect());
        let top = tabla_txt(&["Perfil", "Sección", "Correlación", "D_L (m²/s)", "B/H", "U/u*", "Crítica"], analisis::mayores(&aud, 12).iter().map(|a| vec![a.perfil.clone(), format!("{:.0}", a.seccion), a.metodo.nombre().into(), format!("{:.1}", a.d), format!("{:.1}", a.b_sobre_h), format!("{:.2}", a.u_sobre_ust), if a.critica { "sí".into() } else { String::new() }]).collect());
        v_flex().id("aud").flex_1().h_full().min_w_0().overflow_y_scroll().gap_3().p_4()
            .child(div().text_size(px(12.5)).text_color(rgb(MUTED)).child(format!("{} valores de D_L recalculados ({} no finitos). El detalle completo va en la exportación.", aud.len(), n_mal)))
            .child(tarjeta("Resumen por caudal y correlación (m²/s)", vec![], resumen))
            .child(tarjeta("Los 12 valores mayores", vec![], top))
    }

    /// Pestaña Datos: tabla de las secciones del perfil elegido y avisos de lectura.
    fn vista_datos(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let v = self.vista.as_ref().unwrap();
        let p = &h.perfiles[self.perfil];
        let d = &v.d[self.metodo];
        let cols: [(&str, f32); 8] = [("Sección", 90.), ("Distancia (m)", 110.), ("Área (m²)", 100.), ("Ancho (m)", 100.), ("Perímetro (m)", 110.), ("Sf (m/m)", 100.), ("U (m/s)", 90.), ("D_L (m²/s)", 100.)];
        let celda = |t: String, _w: f32, cab: bool| div().flex_1().min_w_0().text_size(px(12.)).text_color(rgb(if cab { MUTED } else { TEXT })).font_weight(if cab { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).child(t);
        let mut tabla = v_flex().child(h_flex().h(px(32.)).items_center().px_3().bg(rgb(HEADER)).children(cols.iter().map(|(n, w)| celda(n.to_string(), *w, true))));
        for i in 0..p.x.len() {
            let fila = [
                format!("{}", p.secciones[i]),
                format!("{:.0}", p.x[i]),
                format!("{:.1}", p.a[i]),
                format!("{:.1}", p.b[i]),
                format!("{:.1}", p.a[i] / p.r[i]),
                format!("{:.5}", p.sf[i]),
                format!("{:.2}", p.u[i]),
                format!("{:.0}", d[i]),
            ];
            tabla = tabla.child(h_flex().h(px(30.)).items_center().px_3().border_t_1().border_color(rgb(BORDER)).children(fila.into_iter().zip(cols.iter()).map(|(t, (_, w))| celda(t, *w, false))));
        }
        let avisos = v_flex().gap_1().children(h.avisos.iter().map(|a| {
            let c = match a.nivel { Nivel::Info => OK, Nivel::Aviso => WARN, Nivel::Error => ERR };
            h_flex().gap_2().items_start().child(div().mt(px(5.)).size(px(8.)).flex_none().rounded_full().bg(rgb(c))).child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(a.mensaje.clone()))
        }));
        v_flex()
            .id("datos")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .gap_3()
            .p_4()
            .child(tarjeta("Secciones leídas", vec![], v_flex().child(tabla)))
            .child(tarjeta("Avisos de los datos", vec![], avisos))
    }

    /// Exporta todo a una carpeta nueva: CSV, Excel, figuras (SVG, PNG y PDF) y el script de matplotlib. Corre en segundo plano.
    fn exportar_todo(&mut self, cx: &mut Context<Self>) {
        let (Some(figs), Some(h), Some(res)) = (self.todas_las_figuras(), self.hid.clone(), self.actual().cloned()) else { return };
        if self.exportando {
            return;
        }
        self.exportando = true;
        self.export_msg = Some("Exportando…".into());
        let par = Parametros { duracion_pulso: self.durs[self.dur], campo_cada: Some(CAMPO_CADA), ..self.par.clone() };
        let (perfil, metodo, archivos) = (h.perfiles[self.perfil].nombre.clone(), METODOS[self.metodo], self.archivos.clone());
        let verif: Vec<exportar::Verificacion> = self
            .avisos_lista()
            .into_iter()
            .map(|(n, c, m)| (match n { Nivel::Info => "info", Nivel::Aviso => "aviso", Nivel::Error => "error" }.to_string(), c.to_string(), m))
            .collect();
        let (kd, pa) = (self.leer_campo(&self.in_kd, "Factor de D_L", cx).unwrap_or(2.0), self.leer_campo(&self.in_pa, "Variación del área", cx).unwrap_or(10.0));
        let criticas = self.criticas.clone();
        let marca = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let dir = Self::carpeta_export().join(format!("riomat-{marca}"));
        let tarea = cx.background_executor().spawn(async move {
            let d = dir.clone();
            let r = std::panic::catch_unwind(move || -> Result<usize, String> {
                let esc = exportar::escenarios(&h, &par)?;
                let m = exportar::Mostrado { perfil: &perfil, metodo, resultado: &res, archivos: &archivos };
                let sens = analisis::sensibilidad(&h, &par, &analisis::ensayos(if kd > 1.0 { kd } else { 2.0 }, if pa > 0.0 && pa < 100.0 { pa } else { 10.0 }), &criticas)?;
                let aud = analisis::auditoria(&h, &criticas);
                let mut n = exportar::escribir(&d, &h, &par, &esc, &m, &verif, exportar::tablas_analisis(&sens, &aud))?.len();
                let inf = armar_informe(&h, &par, &esc, &figs, &verif, &archivos, &perfil, metodo);
                std::fs::write(d.join("informe.pdf"), riomat_plot::svg_a_pdf(&riomat_plot::informe_svg(&inf))?).map_err(|e| e.to_string())?;
                n += 1;
                let fdir = d.join("figuras");
                std::fs::create_dir_all(&fdir).map_err(|e| format!("{}: {e}", fdir.display()))?;
                for f in &figs {
                    let svg = a_svg(f, 720.0, 380.0);
                    std::fs::write(fdir.join(format!("{}.svg", f.nombre)), &svg).map_err(|e| e.to_string())?;
                    std::fs::write(fdir.join(format!("{}.png", f.nombre)), svg_a_png(&svg, 3.0)?).map_err(|e| e.to_string())?;
                    std::fs::write(fdir.join(format!("{}.pdf", f.nombre)), svg_a_pdf(&svg)?).map_err(|e| e.to_string())?;
                    n += 3;
                }
                Ok(n)
            })
            .unwrap_or_else(|_| Err("fallo interno al exportar".into()));
            (r, dir)
        });
        cx.spawn(async move |this, cx| {
            let (r, dir) = tarea.await;
            let _ = this.update(cx, |this, cx| {
                this.exportando = false;
                this.sin_exportar = this.sin_exportar && r.is_err();
                this.export_msg = Some(match r {
                    Ok(n) => format!("{n} archivos guardados en {}", dir.display()),
                    Err(e) => format!("No se pudo exportar: {e}"),
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Abre el selector de archivos del sistema y carga lo elegido.
    fn abrir_dialogo(&mut self, cx: &mut Context<Self>) {
        let tarea = cx.background_executor().spawn(async move {
            rfd::AsyncFileDialog::new()
                .set_title("Elige la tabla de HEC-RAS (o los dos Excel)")
                .add_filter("Tablas (xlsx, csv, txt)", &["xlsx", "xls", "csv", "txt", "tsv"])
                .pick_files()
                .await
                .map(|v| v.into_iter().map(|f| f.path().to_path_buf()).collect::<Vec<_>>())
        });
        cx.spawn(async move |this, cx| {
            if let Some(rutas) = tarea.await {
                let _ = this.update(cx, |this, cx| {
                    this.cargar(&rutas);
                    this.frame = this.frame_inicial();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Verificación numérica y advertencias de datos (alimenta la insignia y el panel de verificación).
    fn avisos_lista(&self) -> Vec<(Nivel, &'static str, String)> {
        let h = self.hid.as_ref().unwrap();
        let v = self.vista.as_ref().unwrap();
        let p = &h.perfiles[self.perfil];
        let res = self.actual().unwrap();
        let (mut cour, mut pe) = (0.0_f64, 0.0_f64);
        let d = &v.d[self.metodo];
        for &x in &res.x {
            let a = interp(x, &p.x, &p.a);
            let dl = interp(x, &p.x, d);
            let u = res.q / a;
            cour = cour.max(u * res.dt / res.dx);
            pe = pe.max(u * res.dx / dl);
        }
        let mediana = {
            let mut s = d.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            s[s.len() / 2]
        };
        let dmax = d.iter().cloned().fold(0.0, f64::max);
        let mut avisos: Vec<(Nivel, &'static str, String)> = vec![];
        let nv = |c: bool| if c { Nivel::Info } else { Nivel::Aviso };
        avisos.push((nv(res.error_balance < 1e-9), "Balance de masa", format!("Error {:.1e}. Prueba la conservación numérica, no la exactitud física.", res.error_balance)));
        avisos.push((nv(pe < 2.0), "Número de Péclet", format!("Máximo por celda {pe:.2}{}", if pe < 2.0 { ": domina la difusión." } else { ": domina la advección; hay difusión numérica del esquema upwind." })));
        avisos.push((Nivel::Info, "Número de Courant", format!("Máximo {cour:.2}. El esquema implícito es estable para cualquier valor.")));
        if dmax > 4.0 * mediana {
            avisos.push((Nivel::Aviso, "D_L extremo", format!("El máximo ({dmax:.0} m²/s) es {:.1}× la mediana de las secciones: revisar ancho efectivo y pendiente.", dmax / mediana)));
        }
        if res.fraccion_salida < 0.999 {
            avisos.push((Nivel::Aviso, "Tiempo simulado", format!("A las {:.1} h solo salió el {:.2} % de la masa; t95 puede quedar fuera. Aumenta el tiempo total y pulsa Calcular.", res.t.last().copied().unwrap_or(0.0) / 3600.0, res.fraccion_salida * 100.0)));
        }
        for a in &h.avisos {
            avisos.push((a.nivel.clone(), "Datos HEC-RAS", a.mensaje.clone()));
        }
        if let Some(e) = &self.error {
            avisos.push((Nivel::Error, "Error", e.clone()));
        }
        avisos
    }

    fn panel_der(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let h = self.hid.as_ref().unwrap();
        let v = self.vista.as_ref().unwrap();
        let p = &h.perfiles[self.perfil];
        let res = self.actual().unwrap();
        let (paso, _) = res.campo[self.frame];
        let sec = (paso as f64 * res.dt) / 60.0;
        let avisos = self.avisos_lista();
        let lista = v_flex().children(avisos.into_iter().map(|(n, comp, m)| {
            let c = match n {
                Nivel::Info => OK,
                Nivel::Aviso => WARN,
                Nivel::Error => ERR,
            };
            v_flex().gap_0p5().py_2().border_b_1().border_color(rgb(BORDER))
                .child(h_flex().gap_2().items_center().child(div().size(px(8.)).rounded_full().bg(rgb(c))).child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(comp)))
                .child(div().pl(px(16.)).text_size(px(11.5)).text_color(rgb(MUTED)).child(m))
        }));
        v_flex()
            .id("telemetria")
            .w(px(320.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .bg(rgb(PANEL))
            .border_l_1()
            .border_color(rgb(BORDER))
            .child(h_flex().px_4().h(px(40.)).items_center().justify_between().bg(rgb(HEADER)).child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("Verificación")).child(div().id("cerrar_ver").cursor_pointer().text_color(rgb(MUTED)).child("✕").on_click(cx.listener(|this, _, _, cx| { this.ver_avisos = false; cx.notify(); }))))
            .child(
                v_flex().px_4().py_2()
                    .child(fila("Tiempo simulado", format!("{sec:.0} min")))
                    .child(fila("Caudal", format!("{:.2} m³/s", p.q)))
                    .child(fila("Masa inyectada", format!("{:.4}", res.inyectada[paso])))
                    .child(fila("Masa en el tramo", format!("{:.4}", res.masa_dominio[paso])))
                    .child(fila("Masa evacuada", format!("{:.4}", res.salida[paso])))
                    .child(fila("Concentración máx.", format!("{:.2e}", res.concentracion_max)))
                    .child(fila("Malla", format!("dx {:.2} m · dt {:.0} s", res.dx, res.dt))),
            )
                        .child(div().px_4().pb_4().child(lista))
    }
}

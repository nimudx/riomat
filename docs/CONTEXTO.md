# Contexto del proyecto Riomat

Documento para retomar el trabajo (personas o asistentes) sin perder las decisiones tomadas.

## Qué es
Programa de escritorio (Rust + GPUI, Linux y Windows) para simular el transporte 1D de un **trazador pasivo**
(sustituto de microplásticos) en el Río Mantaro, tramo del Puente Breña (2524 m, 12 secciones). Los resultados
alimentan un artículo universitario. Autoría: Riveros (de ahí el nombre: Ri-o + mat-e).

## Restricciones que no se negocian
- **No cambiar el modelo matemático ni inventar datos.** Ecuación: ∂(AC)/∂t + ∂(QC)/∂x = ∂/∂x(A·D·∂C/∂x),
  volúmenes finitos implícitos, advección upwind, conductancia difusiva con media armónica, sistema tridiagonal (Thomas).
- Frontera aguas arriba: flujo de masa prescrito M/T durante 0 ≤ t < T (T = 300 s es una **hipótesis**, no una medición).
  C(x,0) = 0. Aguas abajo: flujo advectivo Q·C. dx ≈ 10 m, dt = 30 s, t_final = 4 h.
- D_L por tres correlaciones (Fischer, Seo-Cheong, Kashefipour-Falconer) con U = Q/A, H = A/B, u* = √(g·R·Sf), R = A/P total.
  No están calibradas. R y B pueden incluir zonas inefectivas. Falta la pendiente de fricción en la sección 74.
- Es trazador pasivo: **no** modela deposición, resuspensión, flotación ni granulometría.
- Caudales: PF1 = 45.2, PF2 = 113.67, PF3 = 342.47 m³/s.

## Fuente de verdad
El modelo en Python (`~/Documents/artrice`, `modelo_entrada_aguas_arriba.py`) es el oráculo. El núcleo Rust coincide
con él a 1e-9 en los 9 escenarios y los 36 casos de sensibilidad (tests en `crates/riomat-core/tests`).
Los datos de referencia (Excel y CSV) **no se versionan**: carpeta `referencia/` o variable `RIOMAT_REFERENCIA`.

## Arquitectura
- `riomat-core`: lectura de HEC-RAS (dos Excel antiguos o una tabla única pegada/archivo con Top Width), D_L, solver. Sin código de SO.
- `riomat-plot`: modelo de gráficas (`Figura`) y exportación SVG → PNG/PDF (resvg, svg2pdf). Misma fuente para pantalla y archivos.
- `riomat`: interfaz GPUI. Pestañas Simulación / Figuras / Datos; insignia de verificación; panel de parámetros con botón Calcular.
- HEC-RAS: las distancias corren desde aguas abajo; el modelo corre desde aguas arriba (x = s0 − sección).

## Decisiones de diseño
Tema claro piedra/agua; el trazador es el único color cálido; perfil longitudinal real del río con agua teñida por la concentración.
Poco texto en pantalla: la verificación vive en una insignia (✓ / ⚠ N) que abre un panel. Identidad propia, no copia de R-THYM.

## Hecho
Núcleo validado · entrada por tabla de HEC-RAS (pegar, abrir, arrastrar) · parámetros editables + Calcular ·
exportación de figuras PNG/SVG/PDF (una o las 4) · **Exportar todo** (CSV + Excel + figuras + `graficar_matplotlib.py` en `~/Documents/riomat-export/riomat-<marca>/`, módulo `riomat-core/src/exportar.rs`) · selector de archivos · pestañas · flujos de Windows y Linux en GitHub.

## Pendiente (orden propuesto)
1. ~~Exportar todo~~ (hecho; falta informe PDF de una página con parámetros, tablas y avisos).
2. ~~Sensibilidad y auditoría de D_L~~ (hecho: `riomat-core/src/analisis.rs`, pestañas Sensibilidad y Auditoría D_L, validado contra Python a 1e-9;
   factores y secciones críticas editables; sale en Exportar todo). Origen: `04_Codigo_Python` (sus rutas `/mnt/data` están rotas).
3. **Comparar los tres caudales** en una pantalla; mapa espacio-tiempo en Figuras.
4. Probar con una **tabla real de HEC-RAS** (hasta ahora solo con tablas armadas desde los Excel); `.hdf`/`.O01` sin soporte.
5. Convergencia en malla y tiempo con la condición de entrada nueva.
6. Verificar el build en Windows (el flujo de Actions nunca se ha visto en verde).

## Cómo trabajar
Ver README. Convenciones: `PathBuf` siempre; lo específico de SO con `cfg` o dependencias por plataforma; commits de una línea en inglés.

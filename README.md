# Riomat

Simulador 1D de transporte de un trazador pasivo en un tramo de río, a partir de los datos hidráulicos de HEC-RAS.
Interfaz nativa en Rust + GPUI. Núcleo de cálculo independiente y validado contra el modelo en Python.

## Estructura
- `crates/riomat-core`: lectura de los dos Excel de HEC-RAS, coeficientes D_L (Fischer, Seo y Cheong, Kashefipour y Falconer) y solver de volúmenes finitos implícito. Sin código específico de sistema operativo.
- `crates/riomat`: interfaz (esquema vivo del tramo, gráficas, telemetría y verificación).
- `referencia/` (no se versiona): datos y resultados de Python usados como oráculo en los tests; sin ella los tests se omiten. Ruta alternativa: variable `RIOMAT_REFERENCIA`.

## Uso
```sh
cargo run -p riomat --release            # carga los Excel de ./referencia
cargo run -p riomat --release -- a.xlsx b.xlsx
```
También se pueden soltar los dos Excel sobre la ventana.

## Pruebas
```sh
cargo test -p riomat-core
```
Compara los 9 escenarios y los 36 casos de sensibilidad con el CSV de Python (tolerancia relativa 1e-9).

## Windows
`.github/workflows/windows.yml` compila `riomat.exe` en GitHub Actions (artefacto `riomat-windows`).
No se ha probado todavía en Windows.

## Alcance
Trazador pasivo conservativo, no microplásticos. D_L viene de correlaciones empíricas sin calibrar y la duración del pulso es una hipótesis.

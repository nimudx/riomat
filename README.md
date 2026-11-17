# Riomat

Simulador 1D de transporte de un trazador pasivo en un tramo de río, a partir de los datos hidráulicos de HEC-RAS.
Interfaz nativa en Rust + GPUI. Núcleo de cálculo independiente y validado contra el modelo en Python.

## Estructura
- `crates/riomat-core`: lectura de los dos Excel de HEC-RAS, coeficientes D_L (Fischer, Seo y Cheong, Kashefipour y Falconer) y solver de volúmenes finitos implícito. Sin código específico de sistema operativo.
- `crates/riomat`: interfaz (esquema vivo del tramo, gráficas, telemetría y verificación).
- `referencia/` (no se versiona): datos y resultados de Python usados como oráculo en los tests; sin ella los tests se omiten. Ruta alternativa: variable `RIOMAT_REFERENCIA`.

## Compilar y ejecutar
Requisito común: Rust estable 1.88 o superior (`rustup`).

**Linux** (necesita las librerías de escritorio habituales: Wayland o X11, Vulkan y fontconfig)
```sh
scripts/build-linux.sh run      # pruebas + build release + abre la app
cargo run -p riomat --release   # alternativa directa
```

**Windows** (PowerShell, con Visual Studio Build Tools «Desarrollo para el escritorio con C++»)
```powershell
.\scripts\build-windows.ps1 -Run    # pruebas + build release; deja dist\riomat.exe
```

**Windows sin tener Windows**: súbelo a GitHub. El flujo `.github/workflows/windows.yml` compila en `windows-latest`
y publica `riomat-windows.zip` en la pestaña Actions (y en el Release si publicas uno).
Experimental: `scripts/cross-windows.sh` intenta compilar el `.exe` desde Linux con `cargo-xwin`.

El código de cálculo (`riomat-core`) y de figuras (`riomat-plot`) no depende del sistema operativo. Lo específico
está aislado en `crates/riomat/Cargo.toml` (selector de archivos) y `main.rs` (sin consola en Windows).

## Uso
```sh
cargo run -p riomat --release -- tabla_hecras.txt     # o los dos Excel; también se pueden soltar sobre la ventana
```

## Pruebas
```sh
cargo test --workspace
```
Compara los 9 escenarios y los 36 casos de sensibilidad con el CSV de Python (tolerancia relativa 1e-9) si existe `referencia/`.

## Alcance
Trazador pasivo conservativo, no microplásticos. D_L viene de correlaciones empíricas sin calibrar y la duración del pulso es una hipótesis.

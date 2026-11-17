"""Figuras de publicación a partir de los CSV exportados por Riomat.

Uso:  python graficar_matplotlib.py        (en esta misma carpeta)
Requiere: pandas y matplotlib.  Salida: carpeta figuras_matplotlib/ (PNG a 300 ppp y PDF).
Los datos son los que calculó Riomat; este script solo los dibuja.
"""
from pathlib import Path

import matplotlib
import pandas as pd

matplotlib.use("Agg")
import matplotlib.pyplot as plt

AQUI = Path(__file__).resolve().parent
SALIDA = AQUI / "figuras_matplotlib"
SALIDA.mkdir(exist_ok=True)
COLOR = {"Fischer": "#0072B2", "Seo y Cheong": "#D55E00", "Kashefipour y Falconer": "#009E73"}
plt.rcParams.update({"font.size": 9, "axes.grid": True, "grid.alpha": 0.25, "axes.spines.top": False, "axes.spines.right": False})


def guardar(fig, nombre):
    fig.tight_layout()
    fig.savefig(SALIDA / f"{nombre}.png", dpi=300)
    fig.savefig(SALIDA / f"{nombre}.pdf")
    plt.close(fig)
    print("OK", nombre)


curvas = pd.read_csv(AQUI / "curvas_salida.csv")
coef = pd.read_csv(AQUI / "coeficientes_dl.csv")
perfiles = list(dict.fromkeys(curvas["perfil"]))

# 1) Masa evacuada acumulada, un panel por caudal
fig, ejes = plt.subplots(1, len(perfiles), figsize=(3.4 * len(perfiles), 3.0), sharey=True, squeeze=False)
for ax, pf in zip(ejes[0], perfiles):
    d = curvas[curvas["perfil"] == pf]
    for met, g in d.groupby("correlacion", sort=False):
        ax.plot(g["tiempo_min"], g["fraccion_evacuada"], color=COLOR.get(met), label=met, lw=1.4)
    ax.set_title(f"{pf} (Q = {d['caudal_m3_s'].iloc[0]:.1f} m³/s)")
    ax.set_xlabel("Tiempo (min)")
ejes[0][0].set_ylabel("Fracción de masa evacuada")
ejes[0][-1].legend(frameon=False, loc="lower right")
guardar(fig, "masa_evacuada_acumulada")

# 2) Coeficiente de dispersión por sección
fig, ejes = plt.subplots(1, len(perfiles), figsize=(3.4 * len(perfiles), 3.0), sharey=True, squeeze=False)
for ax, pf in zip(ejes[0], perfiles):
    d = coef[coef["perfil"] == pf]
    for met, g in d.groupby("correlacion", sort=False):
        ax.plot(g["distancia_m"] / 1000, g["D_L_m2_s"], "o-", ms=3, color=COLOR.get(met), label=met, lw=1.1)
    ax.set_yscale("log")
    ax.set_title(pf)
    ax.set_xlabel("Distancia aguas abajo (km)")
ejes[0][0].set_ylabel("$D_L$ (m²/s)")
ejes[0][-1].legend(frameon=False)
guardar(fig, "dispersion_por_seccion")

# 3) Concentración a lo largo del tramo (escenario mostrado en la app)
ruta = AQUI / "campo_concentracion.csv"
if ruta.exists():
    c = pd.read_csv(ruta)
    tiempos = sorted(c["tiempo_min"].unique())
    elegidos = tiempos[:: max(1, len(tiempos) // 6)]
    fig, ax = plt.subplots(figsize=(5.2, 3.2))
    for t in elegidos:
        g = c[c["tiempo_min"] == t]
        ax.plot(g["x_m"] / 1000, g["concentracion"] * 1e6, label=f"{t:.0f} min", lw=1.2)
    ax.set_xlabel("Distancia aguas abajo (km)")
    ax.set_ylabel("Concentración (×10⁻⁶ masa/m³)")
    ax.legend(frameon=False, ncol=2)
    guardar(fig, "concentracion_en_el_tramo")

//! Núcleo de cálculo de Riomat: lectura de datos HEC-RAS, coeficientes de
//! dispersión longitudinal y transporte 1D de un trazador pasivo.
//!
//! Es una traducción fiel de `modelo_puente_brena_actualizado.py` y
//! `modelo_entrada_aguas_arriba.py`. No contiene nada específico de un sistema
//! operativo, para compilar igual en Linux y Windows.

pub mod dispersion;
pub mod hidraulica;
pub mod solver;
pub mod tabla;

pub use dispersion::{Metodo, METODOS, coeficientes};
pub use hidraulica::{Aviso, Hidraulica, Nivel, Perfil, Registro, leer_hidraulica, leer_hidraulica_auto};
pub use solver::{Parametros, Resultado, simular_entrada};
pub use tabla::{leer_tabla_archivo, leer_tabla_texto};

pub type Resultado2<T> = std::result::Result<T, String>;

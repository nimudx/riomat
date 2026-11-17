//! Incrusta el icono y los datos de versión en el `.exe` de Windows. En otros sistemas no hace nada.
fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon.ico");
        res.set("ProductName", "Riomat");
        res.set("FileDescription", "Riomat - transporte 1D de un trazador en ríos a partir de HEC-RAS");
        res.set("OriginalFilename", "riomat.exe");
        if let Err(e) = res.compile() {
            // No impide compilar: solo se queda sin icono.
            println!("cargo:warning=no se pudo incrustar el icono: {e}");
        }
    }
}

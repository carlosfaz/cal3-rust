//! Calendario mensual con notas — versión de escritorio (Linux y Windows).

// En Windows (release) no abre una consola detrás de la ventana.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    cal3_rust::run_desktop()
}

/// En Android el punto de entrada es `android_main` (en `lib.rs`).
#[cfg(target_os = "android")]
fn main() {}

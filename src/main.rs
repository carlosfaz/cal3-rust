//! Calendario mensual con notas — port en Rust de cal3.py (Flet).
//! Solo escritorio: Linux y Windows. Los datos se guardan en
//! `calendario_notas.json` junto al ejecutable (o en `CAL3_DATA_DIR`).

// En Windows (release) no abre una consola detrás de la ventana.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod note_manager;
mod utils;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("calendario.tex")
            .with_inner_size([460.0, 380.0])
            .with_min_inner_size([460.0, 380.0])
            .with_max_inner_size([1000.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Calendario",
        options,
        Box::new(|cc| {
            crate::app::setup_style(&cc.egui_ctx);
            Ok(Box::new(app::CalendarApp::new()))
        }),
    )
}

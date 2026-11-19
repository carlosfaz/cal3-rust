//! Calendario mensual con notas — port en Rust de cal3.py (Flet).
//! Escritorio (Linux y Windows) y Android. Los datos se guardan en
//! `calendario_notas.json` junto al ejecutable (o en `CAL3_DATA_DIR`);
//! en Android, en la carpeta de archivos de la app.

mod app;
mod note_manager;
mod utils;

use eframe::egui;

/// Handle de la app Android, para consultar los márgenes de las barras del sistema.
#[cfg(target_os = "android")]
pub(crate) static ANDROID_APP: std::sync::OnceLock<winit::platform::android::activity::AndroidApp> =
    std::sync::OnceLock::new();

/// Márgenes en píxeles que ocupan las barras del sistema y el recorte de
/// pantalla: (arriba, abajo, izquierda, derecha). Se piden por JNI a
/// `View.getRootWindowInsets()` de la ventana de la actividad.
#[cfg(target_os = "android")]
pub(crate) fn android_window_insets() -> Option<(i32, i32, i32, i32)> {
    use jni::objects::{JObject, JValue};
    let app = ANDROID_APP.get()?;
    // SAFETY: los punteros los entrega android-activity y viven mientras la app.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let result = (|| -> jni::errors::Result<Option<(i32, i32, i32, i32)>> {
        let window = env.call_method(&activity, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
        let decor = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
        let insets = env
            .call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?
            .l()?;
        if insets.is_null() {
            return Ok(None);
        }
        let mut get = |name: &str| -> jni::errors::Result<i32> {
            env.call_method(&insets, name, "()I", &[] as &[JValue])?.i()
        };
        Ok(Some((
            get("getSystemWindowInsetTop")?,
            get("getSystemWindowInsetBottom")?,
            get("getSystemWindowInsetLeft")?,
            get("getSystemWindowInsetRight")?,
        )))
    })();
    match result {
        Ok(v) => v,
        Err(_) => {
            let _ = env.exception_clear();
            None
        }
    }
}

/// Opciones de ventana comunes (en Android se ignora el tamaño).
fn base_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("calendario.tex")
            .with_inner_size([460.0, 380.0])
            .with_min_inner_size([460.0, 380.0])
            .with_max_inner_size([1000.0, 700.0]),
        ..Default::default()
    }
}

fn run(options: eframe::NativeOptions) -> eframe::Result<()> {
    eframe::run_native(
        "Calendario",
        options,
        Box::new(|cc| {
            app::setup_style(&cc.egui_ctx);
            Ok(Box::new(app::CalendarApp::new()))
        }),
    )
}

/// Punto de entrada de escritorio (lo llama `main.rs`).
#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> eframe::Result<()> {
    run(base_options())
}

/// Punto de entrada de Android (NativeActivity).
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    // Datos en la carpeta de archivos de la app (visible por USB en
    // Android/data/<paquete>/files); si no existe, la interna.
    if let Some(dir) = android_app
        .external_data_path()
        .or_else(|| android_app.internal_data_path())
    {
        std::env::set_var("CAL3_DATA_DIR", dir);
    }
    let _ = ANDROID_APP.set(android_app.clone());
    let mut options = base_options();
    options.android_app = Some(android_app);
    if let Err(e) = run(options) {
        log::error!("cal3-rust terminó con error: {e}");
    }
}

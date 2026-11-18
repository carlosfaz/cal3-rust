//! Gestiona las notas en JSON con escritura atómica, copia de seguridad
//! (.bak), cuarentena de archivos corruptos y deshacer/rehacer.
//! Equivalente a la clase NoteManager de cal3.py y compatible con su
//! formato de archivo (`calendario_notas.json`).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use chrono::Local;

use crate::utils::normalize_note;

const MAX_HISTORY: usize = 100;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Carpeta de datos: `CAL3_DATA_DIR` / `FLET_APP_STORAGE_DATA` si existen;
/// si no, junto al ejecutable (como "junto al script" en Python).
pub fn default_data_dir() -> PathBuf {
    for var in ["CAL3_DATA_DIR", "FLET_APP_STORAGE_DATA"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    if let Some(dir) = &exe_dir {
        if dir_is_writable(dir) {
            return dir.clone();
        }
    }
    // Carpeta del ejecutable de solo lectura (p. ej. Program Files, /usr/bin):
    // se usa la carpeta de datos del usuario.
    let user_dir = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    if let Some(base) = user_dir {
        let dir = base.join("cal3");
        if fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    exe_dir.unwrap_or_else(|| PathBuf::from("."))
}

fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".cal3-write-test-{}", std::process::id()));
    let ok = fs::File::create(&probe).is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

/// Escapa una cadena igual que `json.dumps(..., ensure_ascii=False)`.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Serializa igual que `json.dump(notes, ensure_ascii=False, indent=4, sort_keys=True)`.
fn dumps_sorted(notes: &BTreeMap<String, String>) -> String {
    if notes.is_empty() {
        return "{}".to_string();
    }
    let mut out = String::from("{");
    for (i, (k, v)) in notes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("\n    \"{}\": \"{}\"", json_escape(k), json_escape(v)));
    }
    out.push_str("\n}");
    out
}

pub struct NoteManager {
    pub filepath: PathBuf,
    pub backup_path: PathBuf,
    pub last_error: String,
    pub load_warning: String,
    mtime: Option<SystemTime>,
    undo: Vec<(String, Option<String>)>,
    redo: Vec<(String, Option<String>)>,
    pub notes: BTreeMap<String, String>,
}

impl NoteManager {
    pub fn new(filepath: Option<PathBuf>) -> Self {
        let filepath = filepath.unwrap_or_else(|| default_data_dir().join("calendario_notas.json"));
        let backup_path = PathBuf::from(format!("{}.bak", filepath.display()));
        let mut mgr = Self {
            filepath,
            backup_path,
            last_error: String::new(),
            load_warning: String::new(),
            mtime: None,
            undo: Vec::new(),
            redo: Vec::new(),
            notes: BTreeMap::new(),
        };
        mgr.notes = mgr.load_notes();
        mgr
    }

    // ---- lectura ------------------------------------------------------- #

    /// Analiza el JSON igual que NoteManager._parse de Python.
    fn parse(raw: &str) -> Result<BTreeMap<String, String>, String> {
        let data: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| format!("JSON inválido: {e}"))?;
        let obj = data
            .as_object()
            .ok_or_else(|| "El archivo no contiene un objeto JSON.".to_string())?;
        let mut notes = BTreeMap::new();
        for (key, value) in obj {
            let text = match value {
                serde_json::Value::Null => continue,
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                other => other.to_string(),
            };
            let text = normalize_note(&text);
            if !text.is_empty() {
                notes.insert(key.clone(), text);
            }
        }
        Ok(notes)
    }

    fn read(path: &Path) -> Result<BTreeMap<String, String>, String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        // "utf-8-sig": descarta el BOM si lo hubiera.
        let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
        Self::parse(raw)
    }

    fn stat_mtime(&self) -> Option<SystemTime> {
        fs::metadata(&self.filepath).ok().and_then(|m| m.modified().ok())
    }

    /// Aparta un archivo corrupto en lugar de sobrescribirlo.
    fn quarantine(&self) -> Option<PathBuf> {
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let target = PathBuf::from(format!("{}.corrupto-{}", self.filepath.display(), stamp));
        fs::rename(&self.filepath, &target).ok().map(|_| target)
    }
    fn load_notes(&mut self) -> BTreeMap<String, String> {
        if !self.filepath.exists() {
            self.mtime = None;
            // Si solo queda la copia de seguridad, se recupera de ella.
            if self.backup_path.exists() {
                if let Ok(notes) = Self::read(&self.backup_path) {
                    self.load_warning = "No se encontró el archivo de notas; se restauró la \
                        última copia de seguridad."
                        .to_string();
                    return notes;
                }
            }
            return BTreeMap::new();
        }

        if let Ok(notes) = Self::read(&self.filepath) {
            self.mtime = self.stat_mtime();
            return notes;
        }

        let moved = self.quarantine();
        let (notes, recovered) = match Self::read(&self.backup_path) {
            Ok(notes) => (notes, "Se restauró la última copia de seguridad.".to_string()),
            Err(_) => (BTreeMap::new(), "No había copia de seguridad utilizable.".to_string()),
        };
        let where_msg = match &moved {
            Some(path) => format!("\nSe conservó en:\n{}", path.display()),
            None => String::new(),
        };
        self.load_warning = format!(
            "El archivo de notas estaba dañado o no se podía leer. {recovered}{where_msg}"
        );
        self.mtime = self.stat_mtime();
        notes
    }

    /// Si otro proceso modificó el archivo, se parte de su contenido.
    pub fn refresh_if_external_change(&mut self) {
        let mtime = self.stat_mtime();
        if mtime == self.mtime {
            return;
        }
        if mtime.is_none() {
            // borrado externamente: se conserva lo de memoria
            self.mtime = None;
            return;
        }
        if let Ok(notes) = Self::read(&self.filepath) {
            self.notes = notes;
            self.mtime = mtime;
        }
        // si no se puede leer, no pisamos la memoria con un archivo ilegible
    }

    // ---- escritura ----------------------------------------------------- #

    fn write(&self, notes: &BTreeMap<String, String>) -> std::io::Result<()> {
        let dir = self.filepath.parent().unwrap_or_else(|| Path::new("."));
        let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = dir.join(format!(".notas-{}-{seq}.tmp", std::process::id()));
        let result = (|| {
            {
                let mut file = fs::File::create(&tmp_path)?;
                file.write_all(dumps_sorted(notes).as_bytes())?;
                file.sync_all()?;
            }
            if self.filepath.exists() {
                // solo respalda archivos válidos
                if Self::read(&self.filepath).is_ok() {
                    let _ = fs::copy(&self.filepath, &self.backup_path);
                }
            }
            fs::rename(&tmp_path, &self.filepath) // atómico (reemplaza, también en Windows)
        })();
        if let Err(err) = result {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
        #[cfg(unix)]
        {
            // fsync del directorio para que el renombrado sobreviva a un corte.
            if let Ok(dir_fd) = fs::File::open(dir) {
                let _ = dir_fd.sync_all();
            }
        }
        Ok(())
    }

    /// Aplica un cambio (None borra) y lo persiste. Revierte si falla.
    fn set(&mut self, date_str: &str, value: Option<String>) -> bool {
        self.refresh_if_external_change();
        let previous = self.notes.get(date_str).cloned();
        if previous == value {
            return true;
        }
        match &value {
            None => {
                self.notes.remove(date_str);
            }
            Some(v) => {
                self.notes.insert(date_str.to_string(), v.clone());
            }
        }
        if let Err(exc) = self.write(&self.notes) {
            match previous {
                None => {
                    self.notes.remove(date_str);
                }
                Some(p) => {
                    self.notes.insert(date_str.to_string(), p);
                }
            }
            self.last_error = exc.to_string();
            return false;
        }
        self.last_error.clear();
        true
    }

    // ---- API pública --------------------------------------------------- #

    pub fn get_note(&self, date_str: &str) -> &str {
        self.notes.get(date_str).map(String::as_str).unwrap_or("")
    }

    /// Guarda (o elimina si queda vacía) la nota. Devuelve true si tuvo éxito.
    pub fn save_note(&mut self, date_str: &str, text: &str) -> bool {
        let cleaned = normalize_note(text);
        self.refresh_if_external_change();
        let previous = self.notes.get(date_str).cloned();
        let new_value = if cleaned.is_empty() { None } else { Some(cleaned) };
        if previous == new_value {
            return true;
        }
        if !self.set(date_str, new_value) {
            return false;
        }
        self.undo.push((date_str.to_string(), previous));
        while self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    /// Deshace el último cambio. Devuelve la fecha afectada o None.
    pub fn undo(&mut self) -> Option<String> {
        self.travel(true)
    }

    pub fn redo(&mut self) -> Option<String> {
        self.travel(false)
    }

    fn travel(&mut self, is_undo: bool) -> Option<String> {
        // Se toman las pilas en propiedad para evitar problemas de préstamo.
        let mut source = std::mem::take(if is_undo { &mut self.undo } else { &mut self.redo });
        let mut target = std::mem::take(if is_undo { &mut self.redo } else { &mut self.undo });
        let mut result = None;
        if let Some((date_str, value)) = source.last().cloned() {
            self.refresh_if_external_change();
            let current = self.notes.get(&date_str).cloned();
            if self.set(&date_str, value) {
                source.pop();
                target.push((date_str.clone(), current));
                while target.len() > MAX_HISTORY {
                    target.remove(0);
                }
                result = Some(date_str);
            }
        }
        *if is_undo { &mut self.undo } else { &mut self.redo } = source;
        *if is_undo { &mut self.redo } else { &mut self.undo } = target;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn mgr_en_tmp(nombre: &str) -> (NoteManager, PathBuf) {
        let dir = env::temp_dir().join(format!("cal3-rust-test-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        (NoteManager::new(Some(dir.join("calendario_notas.json"))), dir)
    }

    #[test]
    fn formato_json_compatible_con_python() {
        let mut notas = BTreeMap::new();
        notas.insert("2026-11-16".to_string(), "línea1\nlínea2 ñ".to_string());
        notas.insert("2026-11-17".to_string(), "asdf".to_string());
        let esperado = "{\n    \"2026-11-16\": \"línea1\\nlínea2 ñ\",\n    \"2026-11-17\": \"asdf\"\n}";
        assert_eq!(dumps_sorted(&notas), esperado);
        assert_eq!(dumps_sorted(&BTreeMap::new()), "{}");
    }

    #[test]
    fn guardar_deshacer_rehacer() {
        let (mut mgr, dir) = mgr_en_tmp("undo");
        assert!(mgr.save_note("2026-11-16", "hola"));
        assert_eq!(mgr.get_note("2026-11-16"), "hola");
        assert_eq!(mgr.undo().as_deref(), Some("2026-11-16"));
        assert_eq!(mgr.get_note("2026-11-16"), "");
        assert_eq!(mgr.redo().as_deref(), Some("2026-11-16"));
        assert_eq!(mgr.get_note("2026-11-16"), "hola");
        // recargar desde disco
        let mgr2 = NoteManager::new(Some(dir.join("calendario_notas.json")));
        assert_eq!(mgr2.get_note("2026-11-16"), "hola");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cuarentena_y_rescate_desde_bak() {
        let (mut mgr, dir) = mgr_en_tmp("corrupto");
        assert!(mgr.save_note("2026-11-16", "nota válida"));
        assert!(dir.join("calendario_notas.json.bak").exists() || {
            // primer guardado: aún no hay .bak; hacemos otro cambio
            mgr.save_note("2026-11-16", "nota válida 2")
        });
        fs::write(&mgr.filepath, b"{json roto").unwrap();
        let mgr2 = NoteManager::new(Some(dir.join("calendario_notas.json")));
        assert!(!mgr2.load_warning.is_empty());
        assert_eq!(mgr2.get_note("2026-11-16"), "nota válida");
        assert!(fs::read_dir(&dir).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".corrupto-")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bom_utf8_se_descarta() {
        let (_mgr, dir) = mgr_en_tmp("bom");
        let path = dir.join("calendario_notas.json");
        fs::write(&path, "\u{feff}{\"2026-01-01\": \"año nuevo\"}").unwrap();
        let mgr = NoteManager::new(Some(path));
        assert_eq!(mgr.get_note("2026-01-01"), "año nuevo");
        let _ = fs::remove_dir_all(&dir);
    }
}


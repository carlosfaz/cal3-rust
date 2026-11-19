//! Interfaz del calendario con egui/eframe (port de CalendarApp de cal3.py,
//! solo escritorio: Linux y Windows).

use std::time::{Duration, Instant};

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use crate::note_manager::NoteManager;
use crate::utils::{
    days_in_month, format_date, insert_at_cursor, iso_date, long_date, month_weeks, preview_text, toggle_line_mark,
    word_count, LineMark,
    DIAS_SEMANA, MESES,
};

// Paleta idéntica a CalendarApp.COLORS de cal3.py.
mod colors {
    use super::Color32;
    pub const BG_MAIN: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const HEADER_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const HEADER_FG: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const WEEKDAY_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const WEEKDAY_FG: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const WEEKEND_HEADER_FG: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const DAY_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const DAY_FG: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const WEEKEND_BG: Color32 = Color32::from_rgb(0xE4, 0xE4, 0xE4);
    pub const WEEKEND_FG: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const TODAY_BG: Color32 = Color32::from_rgb(0x35, 0x84, 0xE4);
    pub const TODAY_FG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const SELECTED_BORDER: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const NORMAL_BORDER: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const NOTE_DOT: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const EMPTY_CELL_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const PREVIEW_FG: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
    pub const STATUS_FG: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
    pub const SNACK_OK: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const SNACK_ERR: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
}

/// Celda de tabla: relleno + líneas de 1 px (derecha/abajo; izquierda/arriba
/// solo en el borde de la tabla) para que las líneas vecinas no se dupliquen.
fn table_cell(p: &egui::Painter, r: Rect, fill: Color32, left: bool, top: bool) {
    let r = Rect::from_min_max(r.min.round(), r.max.round());
    p.rect_filled(r, 0.0, fill);
    let st = Stroke::new(1.0, colors::NORMAL_BORDER);
    let (l, t, rt, b) = (r.left() + 0.5, r.top() + 0.5, r.right() - 0.5, r.bottom() - 0.5);
    p.line_segment([Pos2::new(rt, t), Pos2::new(rt, b)], st);
    p.line_segment([Pos2::new(l, b), Pos2::new(rt, b)], st);
    if left {
        p.line_segment([Pos2::new(l, t), Pos2::new(l, b)], st);
    }
    if top {
        p.line_segment([Pos2::new(l, t), Pos2::new(rt, t)], st);
    }
}

/// Colores suaves para marcar días (clic derecho).
pub const PALETA: [(&str, Color32); 8] = [
    ("Rosa", Color32::from_rgb(0xF8, 0xD4, 0xD8)),
    ("Durazno", Color32::from_rgb(0xFB, 0xDF, 0xC4)),
    ("Amarillo", Color32::from_rgb(0xFF, 0xF3, 0xC2)),
    ("Verde", Color32::from_rgb(0xD6, 0xEE, 0xD0)),
    ("Menta", Color32::from_rgb(0xCB, 0xEE, 0xE4)),
    ("Celeste", Color32::from_rgb(0xD0, 0xE4, 0xF7)),
    ("Lila", Color32::from_rgb(0xE2, 0xD8, 0xF4)),
    ("Gris", Color32::from_rgb(0xE6, 0xE6, 0xE6)),
];

pub const HELP_TEXT: &str = "Atajos de teclado (Linux / Windows)\n\
• Ctrl+Z / Ctrl+Y (o Ctrl+Mayús+Z): deshacer / rehacer el último cambio guardado.\n\
• ← / →: día anterior / siguiente (cambia de mes solo).\n\
• ↑ / ↓: una semana antes / después.\n\
• Re Pág / Av Pág: mes anterior / siguiente.\n\
• Enter: abrir la nota del día seleccionado.\n\
• T: volver al mes actual.\n\
• F1: abrir esta ayuda.\n\
• Ctrl+S (en el editor): guardar sin cerrar.\n\
• Esc (en el editor): guardar y cerrar.\n\
• Ctrl+A (en el editor): seleccionar todo.";
pub struct CalendarApp {
    note_manager: NoteManager,
    today: NaiveDate,
    current_year: i32,
    current_month: u32,
    selected_date: NaiveDate,
    status: String,
    // editor de notas
    editor_date: Option<NaiveDate>,
    editor_text: String,
    editor_focused: bool,
    // diálogo de ayuda
    help_open: bool,
    // notificación flotante: (mensaje, es_error, creado)
    snack: Option<(String, bool, Instant)>,
    last_watch: Instant,
    day_colors: std::collections::BTreeMap<String, usize>,
    colors_path: std::path::PathBuf,
}

impl CalendarApp {
    pub fn new() -> Self {
        let note_manager = NoteManager::new(None);
        let today = Local::now().date_naive();
        let load_warning = note_manager.load_warning.clone();
        let colors_path = note_manager
            .filepath
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("calendario_colores.json");
        let day_colors = std::fs::read_to_string(&colors_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let status = format!("Notas en: {}", note_manager.filepath.display());
        Self {
            note_manager,
            today,
            current_year: today.year(),
            current_month: today.month(),
            selected_date: today,
            status,
            editor_date: None,
            editor_text: String::new(),
            editor_focused: false,
            help_open: false,
            snack: if load_warning.is_empty() {
                None
            } else {
                Some((load_warning, true, Instant::now()))
            },
            last_watch: Instant::now(),
            day_colors,
            colors_path,
        }
    }

    /// Asigna (o quita) el color de un día y lo guarda en disco.
    fn set_day_color(&mut self, date: &NaiveDate, color: Option<usize>) {
        let key = iso_date(date);
        match color {
            Some(i) => self.day_colors.insert(key, i),
            None => self.day_colors.remove(&key),
        };
        if let Ok(text) = serde_json::to_string_pretty(&self.day_colors) {
            let tmp = self.colors_path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &self.colors_path);
            }
        }
    }

    // ---- navegación ---------------------------------------------------- #

    fn shift_month(&mut self, delta: i32) {
        let mut month = self.current_month as i32 + delta;
        let mut year = self.current_year;
        if month < 1 {
            month = 12;
            year -= 1;
        } else if month > 12 {
            month = 1;
            year += 1;
        }
        let Some(last_day) = days_in_month(year, month as u32) else {
            return; // fuera de rango representable
        };
        let day = self.selected_date.day().min(last_day);
        let Some(new_selected) = NaiveDate::from_ymd_opt(year, month as u32, day) else {
            return;
        };
        self.current_year = year;
        self.current_month = month as u32;
        self.selected_date = new_selected;
        self.status = format!("{}  ·  Enter para editar.", long_date(&new_selected));
    }

    /// Mueve el día seleccionado (cambia de mes si hace falta).
    fn move_selection(&mut self, days: i64) {
        let Some(new) = self
            .selected_date
            .checked_add_signed(ChronoDuration::days(days))
        else {
            return; // fuera del rango de fechas
        };
        self.selected_date = new;
        self.current_year = new.year();
        self.current_month = new.month();
        self.status = format!("{}  ·  Enter para editar.", long_date(&new));
    }

    fn go_today(&mut self) {
        self.current_year = self.today.year();
        self.current_month = self.today.month();
        self.selected_date = self.today;
        self.status = format!("Hoy es {}.", long_date(&self.today));
    }

    // ---- deshacer / rehacer (a nivel de calendario) --------------------- #

    fn undo(&mut self) {
        let res = self.note_manager.undo();
        self.apply_history(res, "Nada que deshacer.", "↶ Deshecho el cambio en");
    }

    fn redo(&mut self) {
        let res = self.note_manager.redo();
        self.apply_history(res, "Nada que rehacer.", "↷ Rehecho el cambio en");
    }

    fn apply_history(&mut self, date_str: Option<String>, empty_msg: &str, ok_prefix: &str) {
        let Some(date_str) = date_str else {
            if !self.note_manager.last_error.is_empty() {
                let msg = format!("No se pudo aplicar: {}", self.note_manager.last_error);
                self.snack(&msg, true);
            } else {
                self.status = empty_msg.to_string();
            }
            return;
        };
        let Ok(date_obj) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") else {
            return;
        };
        self.current_year = date_obj.year();
        self.current_month = date_obj.month();
        self.selected_date = date_obj;
        // Si el editor está abierto sobre esa fecha, refleja el nuevo contenido.
        if self.editor_date == Some(date_obj) {
            self.editor_text = self.note_manager.get_note(&date_str).to_string();
        }
        self.status = format!("{ok_prefix} {}.", format_date(&date_obj));
    }

    // ---- mensajes ------------------------------------------------------- #

    fn snack(&mut self, msg: &str, error: bool) {
        self.snack = Some((msg.to_string(), error, Instant::now()));
    }

    // ---- editor de notas ------------------------------------------------ #

    fn open_editor(&mut self, date: NaiveDate) {
        self.selected_date = date;
        self.editor_date = Some(date);
        self.editor_text = self.note_manager.get_note(&iso_date(&date)).to_string();
        self.editor_focused = false;
    }

    /// Guarda la nota del editor. Devuelve true si tuvo éxito.
    fn save_current_note(&mut self) -> bool {
        let Some(date) = self.editor_date else {
            return false;
        };
        let ok = self
            .note_manager
            .save_note(&iso_date(&date), &self.editor_text);
        if ok {
            self.status = format!("Nota guardada en {}.", format_date(&date));
        } else {
            let msg = format!("No se pudo guardar: {}", self.note_manager.last_error);
            self.snack(&msg, true);
        }
        ok
    }

    fn close_editor(&mut self, save: bool) {
        if save && !self.save_current_note() {
            return; // no cerrar si falló el guardado
        }
        self.editor_date = None;
        self.editor_text.clear();
    }

    /// Alterna un marcador (casilla / estrella-bolita-flecha) en la línea del cursor.
    fn apply_line_mark(&mut self, ctx: &egui::Context, mark: LineMark) {
        let id = egui::Id::new("note_editor");
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let cursor = state
            .cursor
            .char_range()
            .map_or_else(|| self.editor_text.chars().count(), |r| r.primary.index);
        let (text, new_cursor) = toggle_line_mark(&self.editor_text, cursor, mark);
        self.editor_text = text;
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(new_cursor),
        )));
        state.store(ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }

    /// Inserta la hora o la fecha en la posición del cursor, o sea, en la
    /// línea que se está editando.
    fn insert_stamp(&mut self, ctx: &egui::Context, kind: &str) {
        let now = Local::now();
        let stamp = if kind == "hora" {
            now.format("%H:%M").to_string()
        } else {
            format_date(&now.date_naive())
        };
        let id = egui::Id::new("note_editor");
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let cursor = state
            .cursor
            .char_range()
            .map_or_else(|| self.editor_text.chars().count(), |r| r.primary.index);
        let (text, new_cursor) = insert_at_cursor(&self.editor_text, cursor, &stamp);
        self.editor_text = text;
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(new_cursor),
        )));
        state.store(ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }

    // ---- teclado --------------------------------------------------------- #

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        if self.help_open {
            // Esc / Enter cierran la ayuda.
            let close = events.iter().any(|e| {
                matches!(e, egui::Event::Key { key: egui::Key::Escape | egui::Key::Enter, pressed: true, .. })
            });
            if close {
                self.help_open = false;
            }
            return;
        }
        for event in events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = event else {
                continue;
            };
            let ctrl = modifiers.ctrl || modifiers.command;
            if self.editor_date.is_some() {
                match key {
                    egui::Key::Escape => self.close_editor(true),
                    egui::Key::S if ctrl => {
                        self.save_current_note();
                    }
                    _ => {}
                }
                continue;
            }
            match key {
                egui::Key::Z if ctrl && !modifiers.shift => self.undo(),
                egui::Key::Y if ctrl => self.redo(),
                egui::Key::Z if ctrl && modifiers.shift => self.redo(),
                egui::Key::ArrowLeft => self.move_selection(-1),
                egui::Key::ArrowRight => self.move_selection(1),
                egui::Key::ArrowUp => self.move_selection(-7),
                egui::Key::ArrowDown => self.move_selection(7),
                egui::Key::PageUp => self.shift_month(-1),
                egui::Key::PageDown => self.shift_month(1),
                egui::Key::Enter => self.open_editor(self.selected_date),
                egui::Key::T if !ctrl => self.go_today(),
                egui::Key::F1 => self.help_open = true,
                _ => {}
            }
        }
    }

    // ---- sincronización en caliente (Syncthing, etc.) -------------------- #

    fn watch_external_changes(&mut self, ctx: &egui::Context) {
        // Repintado periódico para detectar medianoche y cambios externos.
        ctx.request_repaint_after(Duration::from_secs(1));
        if self.last_watch.elapsed() < Duration::from_secs(8) {
            return;
        }
        self.last_watch = Instant::now();
        let today = Local::now().date_naive();
        if today != self.today {
            self.today = today;
        }
        let before = self.note_manager.notes.clone();
        self.note_manager.refresh_if_external_change();
        if before != self.note_manager.notes {
            self.status = "Notas actualizadas desde otro dispositivo.".to_string();
        }
    }
}

// ------------------------------------------------------------------------ //
// Renderizado
// ------------------------------------------------------------------------ //

impl CalendarApp {
    /// Fila de navegación: ◀ Mes de año · [Hoy] ▶
    fn nav_row(&mut self, ui: &mut Ui) {
        let btn_size = Vec2::new(30.0, 26.0);
        ui.horizontal(|ui| {
            if ui
                .add_sized(btn_size, egui::Button::new(RichText::new("◀").size(13.0)))
                .on_hover_text("Mes anterior")
                .clicked()
            {
                self.shift_month(-1);
            }
            // Reserva el ancho de los botones de la derecha (en Android hay uno
            // más, «?», porque allí no existe la tecla F1).
            let n_right = if cfg!(target_os = "android") { 3.0 } else { 2.0 };
            let spacing = ui.spacing().item_spacing.x;
            let label_w = (ui.available_width() - btn_size.x * n_right - spacing * n_right).max(0.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(label_w, 28.0), Sense::hover());
            let label = format!(
                "{} de {}",
                MESES[(self.current_month - 1) as usize], self.current_year
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(16.0),
                colors::HEADER_FG,
            );
            if cfg!(target_os = "android")
                && ui
                    .add_sized(btn_size, egui::Button::new(RichText::new("?").size(13.0)))
                    .on_hover_text("Ayuda")
                    .clicked()
            {
                self.help_open = true;
            }
            if ui
                .add_sized(btn_size, egui::Button::new(RichText::new("●").size(11.0)))
                .on_hover_text("Ir a hoy")
                .clicked()
            {
                self.go_today();
            }
            if ui
                .add_sized(btn_size, egui::Button::new(RichText::new("▶").size(13.0)))
                .on_hover_text("Mes siguiente")
                .clicked()
            {
                self.shift_month(1);
            }
        });
    }

    /// Cabeceras Lun..Dom (sábado y domingo en rojo).
    fn weekday_row(&mut self, ui: &mut Ui) {
        let spacing = 0.0;
        let total_w = ui.available_width();
        let cell_w = ((total_w - spacing * 6.0) / 7.0).max(10.0);
        ui.horizontal(|ui| {
            let prev_spacing = ui.spacing().item_spacing;
            ui.spacing_mut().item_spacing = Vec2::splat(spacing);
            for (i, day) in DIAS_SEMANA.iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(cell_w, 26.0), Sense::hover());
                table_cell(ui.painter(), rect, colors::WEEKDAY_BG, i == 0, true);
                let fg = if i >= 5 { colors::WEEKEND_HEADER_FG } else { colors::WEEKDAY_FG };
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    *day,
                    FontId::proportional(14.0),
                    fg,
                );
            }
            ui.spacing_mut().item_spacing = prev_spacing;
        });
    }

    /// Rejilla mensual: cada celda muestra el día, el punto de nota y el
    /// resumen; clic abre el editor.
    fn calendar_grid(&mut self, ui: &mut Ui) {
        let Some(weeks) = month_weeks(self.current_year, self.current_month) else {
            return;
        };
        let spacing = 0.0;
        let cell_w = ((ui.available_width() - spacing * 6.0) / 7.0).max(10.0);
        let row_h = ((ui.available_height() - spacing * (weeks.len() - 1) as f32)
            / weeks.len() as f32)
            .max(26.0);

        let prev_spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = Vec2::splat(spacing);

        let first = NaiveDate::from_ymd_opt(self.current_year, self.current_month, 1);
        let offset = first.map_or(0, |d| d.weekday().num_days_from_monday() as i64);
        for (week_idx, week) in weeks.iter().enumerate() {
            ui.horizontal(|ui| {
                for (weekday, &day) in week.iter().enumerate() {
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(cell_w, row_h), Sense::click());
                    if day == 0 {
                        // Día de otro mes: celda negra con el número en blanco.
                        table_cell(ui.painter(), rect, Color32::from_gray(0x55), weekday == 0, false);
                        let x = rect.right().round() - 1.5;
                        ui.painter().line_segment(
                            [Pos2::new(x, rect.top() + 1.0), Pos2::new(x, rect.bottom() - 1.0)],
                            Stroke::new(1.0, Color32::from_gray(0x3A)),
                        );
                        if let Some(d) = first.and_then(|f| {
                            f.checked_add_signed(ChronoDuration::days(
                                (week_idx * 7 + weekday) as i64 - offset,
                            ))
                        }) {
                            ui.painter().text(
                                rect.left_top() + Vec2::new(3.0, 3.0),
                                Align2::LEFT_TOP,
                                d.day().to_string(),
                                FontId::proportional(16.0),
                                Color32::WHITE,
                            );
                        }
                        continue;
                    }
                    let Some(date) =
                        NaiveDate::from_ymd_opt(self.current_year, self.current_month, day as u32)
                    else {
                        continue;
                    };

                    // Datos de la celda (se calculan antes de pintar para no
                    // mantener préstamos de self durante el clic).
                    let note = self.note_manager.get_note(&iso_date(&date));
                    let has_note = !note.is_empty();
                    let preview = if has_note {
                        preview_text(note, if row_h >= 56.0 { 2 } else { 1 }, 40)
                    } else {
                        String::new()
                    };
                    let tooltip: String = note.chars().take(300).collect();

                    let is_today = date == self.today;
                    let is_selected = date == self.selected_date;
                    let is_weekend = weekday >= 5;
                    let tinted = self.day_colors.get(&iso_date(&date)).and_then(|&i| PALETA.get(i)).map(|c| c.1);
                    let (bg, fg) = if let Some(c) = tinted {
                        (c, colors::DAY_FG)
                    } else if is_today {
                        (colors::TODAY_BG, colors::TODAY_FG)
                    } else if is_weekend {
                        (colors::WEEKEND_BG, colors::WEEKEND_FG)
                    } else {
                        (colors::DAY_BG, colors::DAY_FG)
                    };
                    // Sobre el azul de hoy, el resto del texto también va en blanco.
                    let on_blue = tinted.is_none() && is_today;
                    let (border_w, border_c): (f32, Color32) = if is_selected {
                        (2.0, colors::SELECTED_BORDER)
                    } else {
                        (1.0, colors::NORMAL_BORDER)
                    };

                    let painter = ui.painter_at(rect);
                    table_cell(&painter, rect, bg, weekday == 0, false);

                    // Textos dentro de la celda.
                    let inner = rect.shrink(3.0);
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(inner)
                            .layout(Layout::top_down_justified(Align::LEFT)),
                    );
                    child.set_clip_rect(inner);
                    let day_text = RichText::new(day.to_string()).size(16.0).color(fg);
                    let day_text = if is_today { day_text.strong() } else { day_text };
                    child.add(egui::Label::new(day_text).selectable(false));
                    if !preview.is_empty() {
                        child.add(
                            egui::Label::new(
                                RichText::new(preview)
                                    .size(12.5)
                                    .color(if on_blue { fg } else { colors::PREVIEW_FG }),
                            )
                            .selectable(false)
                            .truncate(),
                        );
                    }

                    // Punto de nota (esquina superior derecha).
                    if has_note {
                        painter.text(
                            rect.right_top() + Vec2::new(-6.0, 4.0),
                            Align2::RIGHT_TOP,
                            "*",
                            FontId::proportional(26.0),
                            if on_blue { fg } else { colors::NOTE_DOT },
                        );
                    }
                    // Daga (†) en el día actual; si también tiene nota, va a la
                    // izquierda del asterisco.
                    if is_today {
                        let dx = if has_note { -22.0 } else { -6.0 };
                        painter.text(
                            rect.right_top() + Vec2::new(dx, 4.0),
                            Align2::RIGHT_TOP,
                            "†",
                            FontId::proportional(22.0),
                            fg,
                        );
                    }
                    if is_selected {
                        painter.rect_stroke(
                            rect.shrink(1.0),
                            0.0,
                            Stroke::new(2.0, colors::SELECTED_BORDER),
                            StrokeKind::Inside,
                        );
                    }
                    if has_note {
                        response.clone().on_hover_text(tooltip);
                    }
                    if response.clicked() {
                        self.open_editor(date);
                    }
                    let mut chosen: Option<Option<usize>> = None;
                    response.context_menu(|ui| {
                        ui.set_min_width(150.0);
                        ui.label(RichText::new("Color del día").size(13.0));
                        ui.separator();
                        for (i, (name, c)) in PALETA.iter().enumerate() {
                            let b = egui::Button::new(RichText::new(*name).size(14.0))
                                .fill(*c)
                                .min_size(Vec2::new(150.0, 24.0));
                            if ui.add(b).clicked() {
                                chosen = Some(Some(i));
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui
                            .add(egui::Button::new(RichText::new("Sin color").size(14.0)).min_size(Vec2::new(150.0, 24.0)))
                            .clicked()
                        {
                            chosen = Some(None);
                            ui.close_menu();
                        }
                    });
                    if let Some(c) = chosen {
                        self.set_day_color(&date, c);
                    }
                }
            });
        }
        ui.spacing_mut().item_spacing = prev_spacing;
    }

    /// Barra de estado inferior.
    fn bottom_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(20.0)
            .frame(egui::Frame::NONE.fill(colors::BG_MAIN))
            .show(ctx, |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add_space(8.0);
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new((ui.available_width() - 8.0).max(0.0), 16.0),
                        Sense::hover(),
                    );
                    ui.painter().with_clip_rect(rect).text(
                        rect.left_center(),
                        Align2::LEFT_CENTER,
                        &self.status,
                        FontId::proportional(10.0),
                        colors::STATUS_FG,
                    );
                });
            });
    }

    /// Notificación flotante tipo SnackBar (se oculta sola a los 5 s).
    fn snack_bar(&mut self, ctx: &egui::Context) {
        let Some((msg, error, created)) = self.snack.clone() else {
            return;
        };
        if created.elapsed() > Duration::from_secs(5) {
            self.snack = None;
            return;
        }
        let bg = if error { colors::SNACK_ERR } else { colors::SNACK_OK };
        egui::Area::new(egui::Id::new("snack"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -32.0))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(bg)
                    .corner_radius(0.0)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        ui.label(RichText::new(msg).color(Color32::WHITE).size(12.0));
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(200));
    }

    /// Ventana modal del editor de notas (equivalente al AlertDialog).
    fn editor_window(&mut self, ctx: &egui::Context) {
        let Some(date) = self.editor_date else {
            return;
        };
        let screen = ctx.screen_rect();
        // El cuadro de texto ocupa todo el alto disponible, dejando sitio
        // al título, la barra, el contador y los botones.
        let text_h = (screen.height() - 200.0).clamp(60.0, 520.0);
        let lines = ((text_h / 18.0) as usize).max(3);
        let dialog_w = (screen.width() - 60.0).clamp(280.0, 560.0);

        // "cancel" | "save" | "save_only"
        let mut action: Option<&str> = None;
        egui::Window::new(RichText::new(long_date(&date)).size(16.0).strong())
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(dialog_w);
                // Barra de herramientas (se ajusta a varias líneas si no cabe).
                ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Ins. Hora").on_hover_text("Insertar hora actual en esta línea").clicked() {
                        self.insert_stamp(ctx, "hora");
                    }
                    if ui.button("Ins. Fecha").on_hover_text("Insertar fecha de hoy en esta línea").clicked() {
                        self.insert_stamp(ctx, "fecha");
                    }
                    if ui.button("Item").on_hover_text("Casilla: - [ ] → - [x] → quitar").clicked() {
                        self.apply_line_mark(ctx, LineMark::Item);
                    }
                    if ui
                        .button("*")
                        .on_hover_text("1 clic: *  ·  2: •  ·  3: ►  ·  4: quitar")
                        .clicked()
                    {
                        self.apply_line_mark(ctx, LineMark::Estrella);
                    }
                });
                ui.add_space(2.0);

                let width = dialog_w.min(ui.available_width());
                // Editor con numeración de líneas (como un IDE): el número
                // aparece solo en el primer renglón de cada línea lógica.
                let n_lines = self.editor_text.split('\n').count().max(1);
                let digits = n_lines.to_string().len().max(2);
                let gutter_w = 14.0 + 7.5 * digits as f32;
                let editor_id = egui::Id::new("note_editor");
                let mut response = None;
                egui::ScrollArea::vertical()
                    .max_height(text_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let (gutter, _) = ui.allocate_exact_size(
                                Vec2::new(gutter_w, 1.0),
                                Sense::hover(),
                            );
                            let out = egui::TextEdit::multiline(&mut self.editor_text)
                                .id(editor_id)
                                .desired_rows(lines)
                                .desired_width((width - gutter_w).max(100.0))
                                .font(FontId::proportional(14.0))
                                .show(ui);
                            let painter = ui.painter();
                            let bg = Rect::from_min_max(
                                Pos2::new(gutter.left(), out.response.rect.top()),
                                Pos2::new(gutter.right(), out.response.rect.bottom()),
                            );
                            painter.rect_filled(bg, 0.0, Color32::from_rgb(0xEE, 0xEE, 0xEE));
                            let mut number = 1;
                            let mut line_start = true;
                            for row in &out.galley.rows {
                                if line_start {
                                    painter.text(
                                        Pos2::new(
                                            gutter.right() - 6.0,
                                            out.galley_pos.y + row.rect.top() + 1.5,
                                        ),
                                        Align2::RIGHT_TOP,
                                        number.to_string(),
                                        FontId::monospace(11.0),
                                        colors::STATUS_FG,
                                    );
                                    number += 1;
                                }
                                line_start = row.ends_with_newline;
                            }
                            response = Some(out.response);
                        });
                    });
                let response = response.expect("editor");
                if !self.editor_focused {
                    response.request_focus();
                    self.editor_focused = true;
                }

                // Contador de palabras y caracteres.
                let counter = format!(
                    "{} palabras · {} caracteres",
                    word_count(&self.editor_text),
                    self.editor_text.chars().count()
                );
                ui.allocate_ui_with_layout(Vec2::new(width, 18.0), Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(counter).size(11.0).color(colors::STATUS_FG));
                });
                ui.add_space(2.0);

                ui.allocate_ui_with_layout(Vec2::new(width, 28.0), Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new("Guardar").fill(Color32::from_rgb(0xE4, 0xE4, 0xE4)))
                        .clicked()
                    {
                        action = Some("save");
                    }
                    if ui.button("Cancelar").clicked() {
                        action = Some("cancel");
                    }
                });
            });

        match action {
            Some("save") => self.close_editor(true),
            Some("cancel") => self.close_editor(false),
            _ => {}
        }
    }

    /// Ventana de ayuda con los atajos de teclado.
    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        egui::Window::new(RichText::new("Ayuda").size(16.0).strong())
            .collapsible(false)
            .resizable(true)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(480.0, 260.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new(HELP_TEXT).size(13.0));
                });
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Cerrar").clicked() {
                        self.help_open = false;
                    }
                });
            });
    }
}

/// Márgenes (en puntos) que ocupan las barras del sistema de Android:
/// (arriba, abajo, izquierda, derecha). En escritorio son 0.
#[cfg(target_os = "android")]
fn system_insets(ctx: &egui::Context) -> (f32, f32, f32, f32) {
    type Cached = (Instant, Vec2, (f32, f32, f32, f32));
    let id = egui::Id::new("system_insets");
    let size = ctx.screen_rect().size();
    if let Some((t, sz, v)) = ctx.data(|d| d.get_temp::<Cached>(id)) {
        if sz == size && t.elapsed() < Duration::from_millis(1000) {
            return v;
        }
    }
    let ppp = ctx.pixels_per_point();
    let v = match crate::android_window_insets() {
        Some((t, b, l, r)) => (t as f32 / ppp, b as f32 / ppp, l as f32 / ppp, r as f32 / ppp),
        None => (0.0, 0.0, 0.0, 0.0),
    };
    ctx.data_mut(|d| d.insert_temp::<Cached>(id, (Instant::now(), size, v)));
    v
}

#[cfg(not(target_os = "android"))]
fn system_insets(_ctx: &egui::Context) -> (f32, f32, f32, f32) {
    (0.0, 0.0, 0.0, 0.0)
}

impl eframe::App for CalendarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.watch_external_changes(ctx);
        self.handle_keys(ctx);

        // Android: la ventana ocupa toda la pantalla, así que se dejan libres
        // los márgenes de las barras de estado/navegación.
        let (top, bottom, left, right) = system_insets(ctx);
        let blank = egui::Frame::NONE.fill(colors::BG_MAIN);
        if top > 0.0 {
            egui::TopBottomPanel::top("inset_top").exact_height(top).frame(blank).show(ctx, |_| {});
        }
        if bottom > 0.0 {
            egui::TopBottomPanel::bottom("inset_bottom").exact_height(bottom).frame(blank).show(ctx, |_| {});
        }
        if left > 0.0 {
            egui::SidePanel::left("inset_left").exact_width(left).frame(blank).show(ctx, |_| {});
        }
        if right > 0.0 {
            egui::SidePanel::right("inset_right").exact_width(right).frame(blank).show(ctx, |_| {});
        }

        self.bottom_panel(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(colors::BG_MAIN)
                    .inner_margin(egui::Margin::symmetric(6, 4)),
            )
            .show(ctx, |ui| {
                self.nav_row(ui);
                ui.add_space(4.0);
                self.weekday_row(ui);
                self.calendar_grid(ui);
            });

        self.editor_window(ctx);
        self.help_window(ctx);
        self.snack_bar(ctx);
    }
}


/// Estilo sobrio: blanco, tinta negra, ángulos rectos, y
/// la fuente monoespaciada (Nimbus Mono, clon de Courier) para todo el texto.
pub fn setup_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/NimbusMonoPS-Regular.otf"
        ))),
    );
    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(fam).or_default().insert(0, "mono".into());
    }
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::light();
    let zero = egui::CornerRadius::ZERO;
    let ink = Color32::BLACK;
    v.window_corner_radius = zero;
    v.menu_corner_radius = zero;
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.window_fill = Color32::WHITE;
    v.window_stroke = Stroke::new(1.0, ink);
    v.panel_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::WHITE;
    v.override_text_color = Some(ink);
    v.selection.bg_fill = Color32::from_rgb(0xCC, 0xCC, 0xCC);
    v.selection.stroke = Stroke::new(1.0, ink);
    let w = &mut v.widgets;
    for (wv, fill) in [
        (&mut w.noninteractive, Color32::WHITE),
        (&mut w.inactive, Color32::WHITE),
        (&mut w.hovered, Color32::from_rgb(0xE4, 0xE4, 0xE4)),
        (&mut w.active, Color32::from_rgb(0xCC, 0xCC, 0xCC)),
        (&mut w.open, Color32::WHITE),
    ] {
        wv.corner_radius = zero;
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::new(1.0, ink);
        wv.fg_stroke = Stroke::new(1.0, ink);
    }
    ctx.set_visuals(v);
}

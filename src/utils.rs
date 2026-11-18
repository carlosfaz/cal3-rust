//! Utilidades compartidas: nombres, normalización, formatos de fecha y
//! resumen de notas para la celda. Equivalentes a las funciones de cal3.py.

use chrono::{Datelike, NaiveDate};

pub const MESES: [&str; 12] = [
    "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio",
    "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre",
];
pub const DIAS_LARGOS: [&str; 7] = [
    "Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo",
];
pub const DIAS_SEMANA: [&str; 7] = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];

/// Normaliza saltos de línea y espacios exteriores de una nota.
pub fn normalize_note(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").trim().to_string()
}

/// Clave ISO `aaaa-mm-dd` con relleno fijo (igual que `date.isoformat()`).
pub fn iso_date(date: &NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

/// `dd/mm/aaaa` con relleno fijo.
pub fn format_date(date: &NaiveDate) -> String {
    format!("{:02}/{:02}/{:04}", date.day(), date.month(), date.year())
}

/// "Lunes, 16 de noviembre de 2026".
pub fn long_date(date: &NaiveDate) -> String {
    let wd = date.weekday().num_days_from_monday() as usize;
    format!(
        "{}, {} de {} de {}",
        DIAS_LARGOS[wd],
        date.day(),
        MESES[(date.month() - 1) as usize].to_lowercase(),
        date.year()
    )
}

/// Semanas de un mes (semanas que empiezan en lunes; 0 = hueco).
/// Devuelve None si el año/mes queda fuera del rango de chrono.
pub fn month_weeks(year: i32, month: u32) -> Option<Vec<Vec<i32>>> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let days = days_in_month(year, month)?;
    let offset = first.weekday().num_days_from_monday() as i32;
    let nweeks = ((offset + days as i32 + 6) / 7) as usize;
    let mut weeks = Vec::with_capacity(nweeks);
    let mut day = 1 - offset;
    for _ in 0..nweeks {
        let mut row = Vec::with_capacity(7);
        for _ in 0..7 {
            row.push(if day >= 1 && (day as u32) <= days { day } else { 0 });
            day += 1;
        }
        weeks.push(row);
    }
    Some(weeks)
}

/// Número de días del mes (None si se sale del rango representable).
pub fn days_in_month(year: i32, month: u32) -> Option<u32> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let (ny, nm) = if month == 12 { (year.checked_add(1)?, 1) } else { (year, month + 1) };
    let first_next = NaiveDate::from_ymd_opt(ny, nm, 1)?;
    Some((first_next - first).num_days() as u32)
}

/// Reemplaza los marcadores de tareas y encabezados como hace la regex en Python.
fn clean_preview_line(ln: &str) -> String {
    let replaced = if let Some(rest) = ln.strip_prefix("- [ ] ") {
        format!("[ ] {rest}")
    } else if ln.starts_with("- [x] ") || ln.starts_with("- [X] ") {
        format!("[x] {}", &ln[6..])
    } else {
        ln.to_string()
    };
    // `^#{1,3} ` — quita de 1 a 3 almohadillas seguidas de espacio.
    let hashes = replaced.chars().take(3).take_while(|c| *c == '#').count();
    if hashes >= 1 && replaced[hashes..].starts_with(' ') {
        replaced[hashes + 1..].to_string()
    } else {
        replaced
    }
}

/// Resumen corto de una nota para mostrar dentro de la celda.
/// (max_chars cuenta caracteres Unicode, como len() en Python).
pub fn preview_text(note: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = note
        .split('\n')
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let out: Vec<String> = lines.iter().take(max_lines).map(|l| clean_preview_line(l)).collect();
    let mut text = out.join("\n");
    let mut truncated = lines.len() > max_lines;
    if text.chars().count() > max_chars {
        text = text.chars().take(max_chars).collect::<String>().trim_end().to_string();
        truncated = true;
    }
    if truncated {
        text.push('…');
    }
    text
}

/// Cuenta palabras como la regex `\w+` (letras/dígitos Unicode o '_').
pub fn word_count(text: &str) -> usize {
    let mut count = 0;
    let mut in_word = false;
    for c in text.chars() {
        let w = c.is_alphanumeric() || c == '_';
        if w && !in_word {
            count += 1;
        }
        in_word = w;
    }
    count
}

/// Tipo de marcador de línea del editor.
#[derive(Clone, Copy, PartialEq)]
pub enum LineMark {
    /// `- [ ] ` → `- [x] ` → sin marcador
    Item,
    /// `* ` → `• ` → `► ` → sin marcador (un solo botón, varios clics)
    Estrella,
}

/// Marcadores del botón de estrella, en el orden de los clics.
const ESTRELLAS: [&str; 3] = ["* ", "• ", "► "];

/// Quita el marcador (si lo hay) y devuelve (resto, marcador_actual).
fn split_mark(line: &str) -> (&str, &'static str) {
    for m in ["- [ ] ", "- [x] ", "- [X] ", "* ", "• ", "► "] {
        if let Some(rest) = line.strip_prefix(m) {
            return (rest, match m {
                "- [ ] " => "item",
                "- [x] " | "- [X] " => "hecho",
                "* " => "estrella",
                "• " => "bolita",
                _ => "flecha",
            });
        }
    }
    (line, "")
}

/// Aplica/alterna un marcador en la línea donde está el cursor
/// (`cursor` es un índice de carácter). Devuelve el texto nuevo y la
/// nueva posición del cursor.
pub fn toggle_line_mark(text: &str, cursor: usize, mark: LineMark) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = chars[..cursor].iter().rposition(|c| *c == '\n').map_or(0, |i| i + 1);
    let end = chars[cursor..].iter().position(|c| *c == '\n').map_or(chars.len(), |i| cursor + i);
    let line: String = chars[start..end].iter().collect();
    let (rest, current) = split_mark(&line);
    let prefix = match (mark, current) {
        (LineMark::Item, "item") => "- [x] ",
        (LineMark::Item, "hecho") => "",
        (LineMark::Item, _) => "- [ ] ",
        (LineMark::Estrella, "estrella") => ESTRELLAS[1],
        (LineMark::Estrella, "bolita") => ESTRELLAS[2],
        (LineMark::Estrella, "flecha") => "",
        (LineMark::Estrella, _) => ESTRELLAS[0],
    };
    let old_len = line.chars().count() - rest.chars().count();
    let new_line = format!("{prefix}{rest}");
    let delta = prefix.chars().count() as isize - old_len as isize;
    let new_cursor = (cursor as isize + delta).max(start as isize) as usize;
    let mut out: String = chars[..start].iter().collect();
    out.push_str(&new_line);
    out.extend(chars[end..].iter());
    (out, new_cursor)
}

/// Inserta `stamp` en la posición del cursor (dentro de su línea), con un
/// espacio antes si hace falta y otro después. Devuelve el texto nuevo y la
/// posición del cursor tras lo insertado.
pub fn insert_at_cursor(text: &str, cursor: usize, stamp: &str) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let before = match cursor.checked_sub(1).map(|i| chars[i]) {
        None | Some(' ') | Some('\n') => "",
        Some(_) => " ",
    };
    let ins = format!("{before}{stamp} ");
    let mut out: String = chars[..cursor].iter().collect();
    out.push_str(&ins);
    out.extend(chars[cursor..].iter());
    (out, cursor + ins.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_crlf_y_espacios() {
        assert_eq!(normalize_note("  hola\r\nmundo\r\n  "), "hola\nmundo");
    }

    #[test]
    fn meses_se_inician_en_lunes() {
        // Noviembre 2026 empieza en domingo y tiene 30 días.
        let weeks = month_weeks(2026, 11).unwrap();
        assert_eq!(weeks[0], vec![0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(weeks.last().unwrap()[0], 30);
    }

    #[test]
    fn preview_marcadores_y_truncado() {
        let note = "- [ ] cocinar\n\n- [x] hecho\n# título\n..." ;
        assert_eq!(preview_text(note, 3, 110), "[ ] cocinar\n[x] hecho\ntítulo…");
        // como en Python: text[:max].rstrip() + "…"
        assert_eq!(preview_text("uno dos tres", 3, 4), "uno…");
    }

    #[test]
    fn palabras() {
        assert_eq!(word_count("Hola mazacuata, 123 _x"), 4);
    }

    #[test]
    fn marcadores_de_linea() {
        let t = "Hola\nMi nombre\nes\nAsdf";
        let (a, _) = toggle_line_mark(t, 2, LineMark::Item);
        assert_eq!(a, "- [ ] Hola\nMi nombre\nes\nAsdf");
        let (b, _) = toggle_line_mark(&a, 3, LineMark::Item);
        assert_eq!(b, "- [x] Hola\nMi nombre\nes\nAsdf");
        let (c, _) = toggle_line_mark(&b, 3, LineMark::Item);
        assert_eq!(c, t);
        // Un solo botón: * → • → ► → nada.
        let (d, cur) = toggle_line_mark(t, t.chars().count(), LineMark::Estrella);
        assert_eq!(d, "Hola\nMi nombre\nes\n* Asdf");
        assert_eq!(cur, d.chars().count());
        let (e, _) = toggle_line_mark(&d, d.chars().count(), LineMark::Estrella);
        assert_eq!(e, "Hola\nMi nombre\nes\n• Asdf");
        let (f, _) = toggle_line_mark(&e, e.chars().count(), LineMark::Estrella);
        assert_eq!(f, "Hola\nMi nombre\nes\n► Asdf");
        let (g, _) = toggle_line_mark(&f, f.chars().count(), LineMark::Estrella);
        assert_eq!(g, t);
    }

    #[test]
    fn insertar_en_la_linea_del_cursor() {
        // Cursor al final de la línea 2 (texto de 3 líneas).
        let t = "uno\ndos\ntres";
        let (a, c) = insert_at_cursor(t, 7, "10:30");
        assert_eq!(a, "uno\ndos 10:30 \ntres");
        assert_eq!(c, 14);
        // Inicio de línea: sin espacio antes.
        let (b, _) = insert_at_cursor(t, 4, "10:30");
        assert_eq!(b, "uno\n10:30 dos\ntres");
        // Texto vacío.
        let (d, _) = insert_at_cursor("", 0, "x");
        assert_eq!(d, "x ");
    }
}

use crate::room::{Cue, CueDraft, DEFAULT_CUE_MS};

/// Accepts minutes, mm:ss, or hh:mm:ss.
pub fn parse_duration(raw: &str) -> Option<u64> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    if parts
        .iter()
        .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    let mut value: u64 = 0;
    for part in &parts {
        value = value
            .checked_mul(60)?
            .checked_add(part.parse::<u64>().ok()?)?;
    }
    if parts.len() == 1 {
        value = value.checked_mul(60)?;
    }
    value.checked_mul(1000)
}

const HEADER: &str = "title,speaker,duration,notes";

/// Reads a running order. A header row names the columns, and its absence
/// means the default order.
pub fn parse_csv(body: &str) -> Result<Vec<CueDraft>, String> {
    let mut rows = Vec::new();
    for (index, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        rows.push((index + 1, split_row(line)));
    }
    if rows.is_empty() {
        return Err("the document holds no rows".to_string());
    }

    let mut columns = vec!["title", "speaker", "duration", "notes"];
    if rows
        .first()
        .is_some_and(|(_, first)| looks_like_header(first))
    {
        columns = rows[0]
            .1
            .iter()
            .map(|name| column_name(name.trim()))
            .collect::<Vec<&str>>();
        rows.remove(0);
    }
    if rows.is_empty() {
        return Err("the document holds a header and no cues".to_string());
    }

    let mut cues = Vec::new();
    for (line, fields) in rows {
        let mut cue = CueDraft {
            title: String::new(),
            speaker: String::new(),
            duration_ms: DEFAULT_CUE_MS,
            notes: String::new(),
        };
        for (index, value) in fields.iter().enumerate() {
            let value = value.trim();
            match columns.get(index).copied().unwrap_or("") {
                "title" => cue.title = value.to_string(),
                "speaker" => cue.speaker = value.to_string(),
                "notes" => cue.notes = value.to_string(),
                "duration" if !value.is_empty() => {
                    cue.duration_ms = parse_duration(value)
                        .ok_or_else(|| format!("line {line}: {value} is not a duration"))?;
                }
                _ => {}
            }
        }
        if cue.title.is_empty() {
            return Err(format!("line {line}: a cue needs a title"));
        }
        cues.push(cue);
    }
    Ok(cues)
}

pub fn to_csv(cues: &[Cue]) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for cue in cues {
        let row = [
            escape(&cue.title),
            escape(&cue.speaker),
            format_duration(cue.duration_ms),
            escape(&cue.notes),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

fn format_duration(ms: u64) -> String {
    let total = ms / 1000;
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn looks_like_header(fields: &[String]) -> bool {
    fields
        .iter()
        .any(|field| matches!(column_name(field.trim()), "title" | "duration"))
}

/// Maps the spellings a spreadsheet is likely to carry onto one column name.
fn column_name(raw: &str) -> &'static str {
    match raw.to_ascii_lowercase().as_str() {
        "title" | "cue" | "name" | "segment" => "title",
        "speaker" | "presenter" | "who" | "person" => "speaker",
        "duration" | "length" | "time" | "minutes" => "duration",
        "notes" | "note" | "comment" => "notes",
        _ => "",
    }
}

/// Splits one line on commas, honoring quotes and doubled quotes.
fn split_row(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields
}

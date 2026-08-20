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

/// A cue as a document rather than as live state: no id, and a duration in the
/// same shape the CSV and the console use.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CueDocument {
    pub title: String,
    pub speaker: String,
    pub duration: String,
    pub notes: String,
}

#[derive(serde::Deserialize)]
struct RundownDocument {
    cues: Vec<CueDraft>,
}

/// Writes the running order as JSON carrying the same fields as the CSV.
pub fn to_json(cues: &[Cue]) -> String {
    let document: Vec<CueDocument> = cues
        .iter()
        .map(|cue| CueDocument {
            title: cue.title.clone(),
            speaker: cue.speaker.clone(),
            duration: format_duration(cue.duration_ms),
            notes: cue.notes.clone(),
        })
        .collect();
    serde_json::json!({ "cues": document }).to_string()
}

/// Reads a running order from JSON. A cue takes `duration` in clock form, or
/// `duration_ms` for a caller that already counts milliseconds.
pub fn parse_json(body: &str) -> Result<Vec<CueDraft>, String> {
    let document: RundownDocument = serde_json::from_str(body).map_err(|err| err.to_string())?;
    Ok(document.cues)
}

/// Reads a running order. A header row names the columns, and its absence
/// means the default order.
pub fn parse_csv(body: &str) -> Result<Vec<CueDraft>, String> {
    let mut rows = split_rows(body)?;
    rows.retain(|(_, fields)| fields.iter().any(|field| !field.trim().is_empty()));
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

/// Splits a document into rows of fields, honoring quotes and doubled quotes.
/// A quoted field may hold a newline, which is how a note with two lines
/// survives a round trip through a spreadsheet. Each row carries the line it
/// starts on, so an error points at the right place in a text editor.
fn split_rows(body: &str) -> Result<Vec<(usize, Vec<String>)>, String> {
    let mut rows = Vec::new();
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut line = 1;
    let mut row_line = 1;
    let mut quote_line = 1;
    let mut chars = body.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => {
                quoted = !quoted;
                if quoted {
                    quote_line = line;
                }
            }
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            '\r' if !quoted && chars.peek() == Some(&'\n') => {}
            '\n' if !quoted => {
                fields.push(std::mem::take(&mut field));
                rows.push((row_line, std::mem::take(&mut fields)));
                line += 1;
                row_line = line;
            }
            _ => {
                if c == '\n' {
                    line += 1;
                }
                field.push(c);
            }
        }
    }

    // A quote left open would otherwise swallow every row after it.
    if quoted {
        return Err(format!("line {quote_line}: a quote is never closed"));
    }
    if !field.is_empty() || !fields.is_empty() {
        fields.push(field);
        rows.push((row_line, fields));
    }
    Ok(rows)
}

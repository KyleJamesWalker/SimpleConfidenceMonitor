use crate::room::{Cue, CueDraft, DEFAULT_CUE_MS, MAX_CUES, Note};

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

/// Notes for a whole rundown. Every frame carries all of them to every client,
/// and a slow client may hold a backlog of frames.
pub const RUNDOWN_NOTES_LIMIT: usize = 100_000;

/// Notes as one block of text, which is how a CSV column carries them and how
/// a rundown written before notes had times reads. A leading clock time starts
/// a note. Any other line continues the note above it, so a note keeps its
/// line breaks through a round trip.
pub fn notes_from_text(body: &str) -> Vec<Note> {
    let mut notes: Vec<Note> = Vec::new();
    for line in body.lines() {
        let indented = line.starts_with([' ', '\t']);
        match split_at_time(line.trim()) {
            Some((at_ms, text)) if !indented => notes.push(Note {
                at_ms,
                text: text.to_string(),
            }),
            _ => match notes.last_mut() {
                Some(note) => {
                    note.text.push('\n');
                    note.text.push_str(line.trim());
                }
                None => notes.push(Note {
                    at_ms: 0,
                    text: line.trim().to_string(),
                }),
            },
        }
    }
    for note in &mut notes {
        note.text = note.text.trim().to_string();
    }
    notes.retain(|note| !note.text.is_empty());
    notes
}

/// The reverse. A note's later lines are indented, so reading it back does not
/// mistake one of them for a new note.
pub fn notes_to_text(notes: &[Note]) -> String {
    notes
        .iter()
        .map(|note| {
            let mut lines = note.text.lines();
            let mut out = format!(
                "{} {}",
                format_duration(note.at_ms),
                lines.next().unwrap_or("")
            );
            for line in lines {
                out.push_str("\n  ");
                out.push_str(line);
            }
            out
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// "5:00 wrap up" splits into the time and the rest. The time needs a colon:
/// a note that opens with a bare number is text, not a cue point.
fn split_at_time(line: &str) -> Option<(u64, &str)> {
    let (head, rest) = line.split_once(' ')?;
    if !head.contains(':') {
        return None;
    }
    let at_ms = parse_duration(head)?;
    Some((at_ms, rest.trim_start()))
}

/// A cue as a document rather than as live state: no id, and a duration in the
/// same shape the CSV and the console use.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CueDocument {
    pub title: String,
    pub speaker: String,
    pub duration: String,
    pub notes: Vec<NoteDocument>,
}

/// A note as a document carries it: the time into the cue in clock form, the
/// way the duration beside it reads.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NoteDocument {
    pub at: String,
    pub text: String,
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
            notes: cue
                .notes
                .iter()
                .map(|note| NoteDocument {
                    at: format_duration(note.at_ms),
                    text: note.text.clone(),
                })
                .collect(),
        })
        .collect();
    serde_json::json!({ "cues": document }).to_string()
}

/// Reads a running order from JSON. A cue takes `duration` in clock form, or
/// `duration_ms` for a caller that already counts milliseconds.
pub fn parse_json(body: &str) -> Result<Vec<CueDraft>, String> {
    let document: RundownDocument = serde_json::from_str(body).map_err(|err| err.to_string())?;
    within_ceiling(document.cues)
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
            notes: Vec::new(),
        };
        for (index, value) in fields.iter().enumerate() {
            let value = value.trim();
            match columns.get(index).copied().unwrap_or("") {
                "title" => cue.title = value.to_string(),
                "speaker" => cue.speaker = value.to_string(),
                "notes" => cue.notes = notes_from_text(value),
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
    within_ceiling(cues)
}

/// A rundown rides in every state frame to every client, so an import that
/// would not fit a show is refused rather than truncated in silence. The notes
/// carry the most text by far, so they have a budget of their own.
fn within_ceiling(cues: Vec<CueDraft>) -> Result<Vec<CueDraft>, String> {
    if cues.len() > MAX_CUES {
        return Err(format!(
            "a rundown holds at most {MAX_CUES} cues, and this one has {}",
            cues.len()
        ));
    }
    let notes: usize = cues
        .iter()
        .flat_map(|cue| cue.notes.iter())
        .map(|note| note.text.chars().count())
        .sum();
    if notes > RUNDOWN_NOTES_LIMIT {
        return Err(format!(
            "a rundown holds at most {RUNDOWN_NOTES_LIMIT} characters of notes, and this one has {notes}"
        ));
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
            escape(&notes_to_text(&cue.notes)),
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

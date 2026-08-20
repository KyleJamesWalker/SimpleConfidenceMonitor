/// A validated room name, safe in a URL path and as a snapshot filename.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoomName(String);

pub const RESERVED_NAMES: [&str; 3] = ["api", "assets", "healthz"];
pub const MAX_NAME_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    Reserved,
    BadCharacter,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Empty => "room name is empty",
            Self::TooLong => "room name is longer than 64 characters",
            Self::Reserved => "room name is reserved",
            Self::BadCharacter => "room name allows only a-z, 0-9, dash and underscore",
        };
        f.write_str(text)
    }
}

impl RoomName {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if raw.chars().count() > MAX_NAME_LEN {
            return Err(NameError::TooLong);
        }
        let name = raw.to_ascii_lowercase();
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(NameError::BadCharacter);
        }
        if RESERVED_NAMES.contains(&name.as_str()) {
            return Err(NameError::Reserved);
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoomName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Default)]
pub struct Room {}

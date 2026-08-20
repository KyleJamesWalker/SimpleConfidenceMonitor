use axum::http::{HeaderMap, header};
use subtle::ConstantTimeEq;

/// Who may control a room. Viewer routes never consult this.
#[derive(Debug, Default)]
pub struct Auth {
    token: Option<String>,
}

/// The name of the cookie the console keeps, so an operator pastes the link once.
pub const COOKIE: &str = "scm_token";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Allowed. Carries a cookie value when the request presented the token another way.
    Allowed {
        store_cookie: Option<String>,
    },
    Denied,
}

impl Auth {
    pub fn open() -> Self {
        Self { token: None }
    }

    pub fn with_token(token: impl Into<String>) -> Self {
        Self {
            token: Some(token.into()),
        }
    }

    pub fn is_open(&self) -> bool {
        self.token.is_none()
    }

    pub fn check(&self, headers: &HeaderMap, query_token: Option<&str>) -> Outcome {
        let Some(expected) = &self.token else {
            return Outcome::Allowed { store_cookie: None };
        };
        if bearer_token(headers).is_some_and(|offered| matches(expected, &offered)) {
            return Outcome::Allowed { store_cookie: None };
        }
        // Only a browser navigation needs the cookie, and that arrives in the query.
        if query_token.is_some_and(|offered| matches(expected, offered)) {
            return Outcome::Allowed {
                store_cookie: query_token.map(str::to_string),
            };
        }
        if cookie_token(headers).is_some_and(|offered| matches(expected, &offered)) {
            return Outcome::Allowed { store_cookie: None };
        }
        Outcome::Denied
    }
}

/// Length is public, so comparing it early leaks nothing a caller cannot measure.
fn matches(expected: &str, offered: &str) -> bool {
    expected.len() == offered.len() && expected.as_bytes().ct_eq(offered.as_bytes()).into()
}

/// Reads the token a request presents, in header, then query, then cookie order.
pub fn presented(headers: &HeaderMap, query_token: Option<&str>) -> Option<String> {
    if let Some(bearer) = bearer_token(headers) {
        return Some(bearer);
    }
    if let Some(token) = query_token {
        return Some(token.to_string());
    }
    cookie_token(headers)
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|token| token.trim().to_string())
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_string())
}

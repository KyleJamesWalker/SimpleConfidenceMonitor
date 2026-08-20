use axum::http::{HeaderMap, HeaderValue, header};
use simple_confidence_monitor::auth::{Auth, COOKIE, Outcome, presented};

fn headers(pairs: &[(header::HeaderName, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(name.clone(), HeaderValue::from_str(value).unwrap());
    }
    map
}

fn allowed() -> Outcome {
    Outcome::Allowed { store_cookie: None }
}

#[test]
fn an_open_server_allows_every_request() {
    let auth = Auth::open();
    assert!(auth.is_open());
    assert_eq!(auth.check(&HeaderMap::new(), None), allowed());
}

#[test]
fn a_guarded_server_denies_a_request_with_no_token() {
    let auth = Auth::with_token("s3cret");
    assert_eq!(auth.check(&HeaderMap::new(), None), Outcome::Denied);
}

#[test]
fn a_bearer_header_is_accepted() {
    let auth = Auth::with_token("s3cret");
    let map = headers(&[(header::AUTHORIZATION, "Bearer s3cret")]);
    assert_eq!(auth.check(&map, None), allowed());
}

#[test]
fn a_query_token_is_accepted_and_asks_for_a_cookie() {
    let auth = Auth::with_token("s3cret");
    assert_eq!(
        auth.check(&HeaderMap::new(), Some("s3cret")),
        Outcome::Allowed {
            store_cookie: Some("s3cret".to_string())
        }
    );
}

#[test]
fn a_cookie_is_accepted_without_asking_for_another() {
    let auth = Auth::with_token("s3cret");
    let map = headers(&[(header::COOKIE, "other=1; scm_token=s3cret")]);
    assert_eq!(auth.check(&map, None), allowed());
}

#[test]
fn a_wrong_token_is_denied_from_every_source() {
    let auth = Auth::with_token("s3cret");
    let map = headers(&[(header::AUTHORIZATION, "Bearer nope")]);
    assert_eq!(auth.check(&map, None), Outcome::Denied);
    assert_eq!(auth.check(&HeaderMap::new(), Some("nope")), Outcome::Denied);
    let map = headers(&[(header::COOKIE, "scm_token=nope")]);
    assert_eq!(auth.check(&map, None), Outcome::Denied);
}

#[test]
fn a_token_that_is_a_prefix_is_denied() {
    let auth = Auth::with_token("s3cret");
    assert_eq!(
        auth.check(&HeaderMap::new(), Some("s3cre")),
        Outcome::Denied
    );
    assert_eq!(
        auth.check(&HeaderMap::new(), Some("s3cretly")),
        Outcome::Denied
    );
}

#[test]
fn the_header_wins_over_the_query_and_the_cookie() {
    let map = headers(&[
        (header::AUTHORIZATION, "Bearer from-header"),
        (header::COOKIE, "scm_token=from-cookie"),
    ]);
    assert_eq!(presented(&map, Some("from-query")).unwrap(), "from-header");
}

#[test]
fn the_query_wins_over_the_cookie() {
    let map = headers(&[(header::COOKIE, "scm_token=from-cookie")]);
    assert_eq!(presented(&map, Some("from-query")).unwrap(), "from-query");
}

#[test]
fn a_cookie_header_without_our_cookie_presents_nothing() {
    let map = headers(&[(header::COOKIE, "session=abc; theme=dark")]);
    assert_eq!(presented(&map, None), None);
    assert_eq!(COOKIE, "scm_token");
}

#[test]
fn an_authorization_header_without_bearer_presents_nothing() {
    let map = headers(&[(header::AUTHORIZATION, "Basic abc")]);
    assert_eq!(presented(&map, None), None);
}

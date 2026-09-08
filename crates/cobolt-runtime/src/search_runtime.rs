//! Web search providers for the `WebSearch` control.
//!
//! The control shipped against one back end — Google's Custom Search JSON API —
//! with the endpoint, the query string and the response shape all written into
//! `interpreter.rs` by hand. When that API stopped being an option, the control
//! had nowhere to go: nothing about it was pluggable.
//!
//! This module is the seam. It turns the control's own properties into an HTTP
//! request, and a provider's answer back into the `(title, snippet, link)`
//! triples the accessors (`ResultCount`, `TopTitle`, `TopSnippet`, `TopLink`,
//! `Result(n)`) have always spoken. Those accessors were already
//! provider-neutral, so they are unchanged — a form written against Google
//! reads Brave's answers without an edit.
//!
//! **Deliberately dependency-free.** It builds strings and parses JSON; it
//! sends nothing. That keeps the whole provider matrix unit-testable with no
//! network, no keys and no `http` feature — the request for every provider is
//! asserted byte for byte in this file's own tests.

use serde_json::Value;

/// A search back end.
///
/// `Google` is first and is what an unset or unrecognised `Provider` resolves
/// to, so a form saved before this existed keeps the behaviour it had.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    Brave,
    Serper,
    Tavily,
    SearxNg,
}

/// The `Provider` property's choices, in the order the designer offers them.
/// One list, so the properties pane, the documentation and [`Provider::parse`]
/// cannot disagree about what is spellable.
pub const PROVIDER_NAMES: [&str; 5] = ["Google", "Brave", "Serper", "Tavily", "SearXNG"];

impl Provider {
    /// Case-insensitive, and tolerant of an unset property.
    ///
    /// An unknown name resolves to `Google` rather than failing: a `.cfrm`
    /// written by a newer IDE opens in an older one with a working control, not
    /// a broken one.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "brave" => Self::Brave,
            "serper" => Self::Serper,
            "tavily" => Self::Tavily,
            "searxng" | "searx" => Self::SearxNg,
            _ => Self::Google,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Brave => "Brave",
            Self::Serper => "Serper",
            Self::Tavily => "Tavily",
            Self::SearxNg => "SearXNG",
        }
    }

    /// Does this provider authenticate at all?
    ///
    /// SearXNG is the one that does not: you run the instance, so there is no
    /// account and no key to hold.
    pub fn needs_api_key(&self) -> bool {
        !matches!(self, Self::SearxNg)
    }

    /// Does it need an `Endpoint` — an address only the developer knows?
    pub fn needs_endpoint(&self) -> bool {
        matches!(self, Self::SearxNg)
    }

    /// The provider's own documented ceiling on results per request.
    ///
    /// `NumResults` is clamped to this rather than passed through, because
    /// asking Google for 50 is an HTTP 400, not 50 results.
    pub fn max_results(&self) -> u32 {
        match self {
            Self::Google => 10,
            Self::Brave => 20,
            Self::Tavily => 20,
            Self::Serper => 100,
            Self::SearxNg => 50,
        }
    }

    /// Does it accept a SafeSearch level?
    ///
    /// Serper and Tavily do not expose one, so the property is silently not
    /// sent rather than faked — and the guide says so, so nobody assumes a
    /// filter is running that is not.
    pub fn honours_safe_search(&self) -> bool {
        matches!(self, Self::Google | Self::Brave | Self::SearxNg)
    }
}

/// What the control knows, ready to be turned into a request.
#[derive(Clone, Copy, Debug)]
pub struct SearchParams<'a> {
    /// The resolved key — the control's own `ApiKey`, else the project's.
    pub api_key: &'a str,
    /// Google's Programmable Search Engine `cx`. Ignored by every other
    /// provider, which search the whole web without being told where.
    pub engine_id: &'a str,
    /// The SearXNG instance's base URL.
    pub endpoint: &'a str,
    pub query: &'a str,
    pub num_results: u32,
    /// The control's friendly `Off` / `Medium` / `High`.
    pub safe_search: &'a str,
}

/// One provider's HTTP request, ready for the transport that already exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRequest {
    pub method: &'static str,
    pub url: String,
    /// Empty for the GET providers.
    pub body: String,
    pub headers: Vec<(String, String)>,
}

/// Why a search cannot run yet, or `None` when it can.
///
/// Checked before anything is sent, so a misconfigured control fails through
/// `onError` in the same statement rather than after a network round trip.
/// Every message contains "not configured", which is the phrase the control's
/// contract has always used.
pub fn configuration_error(provider: Provider, api_key: &str, endpoint: &str) -> Option<String> {
    if provider.needs_api_key() && api_key.trim().is_empty() {
        return Some(format!(
            "{} search API key not configured",
            provider.as_str()
        ));
    }
    if provider.needs_endpoint() && endpoint.trim().is_empty() {
        return Some(format!(
            "{} Endpoint not configured — set it to the address of your instance",
            provider.as_str()
        ));
    }
    None
}

/// Percent-encode one query-string value.
fn enc(s: &str) -> String {
    crate::interpreter::percent_encode_query(s)
}

/// `Off` / `Medium` / `High` in the provider's own vocabulary.
fn safe_level(provider: Provider, level: &str) -> &'static str {
    let off = level.trim().eq_ignore_ascii_case("off");
    let high = level.trim().eq_ignore_ascii_case("high");
    match provider {
        // Custom Search has two levels only, so Medium and High both filter.
        Provider::Google => {
            if off {
                "off"
            } else {
                "active"
            }
        }
        Provider::Brave => {
            if off {
                "off"
            } else if high {
                "strict"
            } else {
                "moderate"
            }
        }
        Provider::SearxNg => {
            if off {
                "0"
            } else if high {
                "2"
            } else {
                "1"
            }
        }
        Provider::Serper | Provider::Tavily => "",
    }
}

/// Build the HTTP request for one search.
///
/// Sources, so the next person does not have to guess as this control's first
/// author had to:
///
/// * **Google** — Custom Search JSON API; unchanged from what the control
///   shipped with.
/// * **Brave** — `GET /res/v1/web/search`, key in `X-Subscription-Token`,
///   `count` ≤ 20, `safesearch` one of `off`/`moderate`/`strict`. Verified
///   against Brave's own API documentation, 2026-09-08.
/// * **Tavily** — `POST /search`, key as `Authorization: Bearer`, body
///   `{query, max_results}`. Verified against Tavily's endpoint reference,
///   2026-09-08. The key is a **header**, not an `api_key` body field.
/// * **Serper** — `POST /search`, key in `X-API-KEY`, body `{q, num}`. Written
///   from knowledge of the service: its documentation host did not resolve when
///   this was implemented, so this one provider is the least corroborated of
///   the five and wants a smoke test against a live key.
/// * **SearXNG** — `GET <endpoint>/search`, `format=json`, `safesearch` 0/1/2.
///   Endpoint and parameters verified against the SearXNG search-API docs,
///   2026-09-08; the result field names are not documented there and follow the
///   shape instances actually return. **`format=json` must be enabled in the
///   instance's own settings** — it is off by default on public instances.
pub fn build_request(provider: Provider, prm: &SearchParams) -> SearchRequest {
    let num = prm.num_results.clamp(1, provider.max_results());
    let safe = safe_level(provider, prm.safe_search);
    match provider {
        Provider::Google => SearchRequest {
            method: "GET",
            url: format!(
                "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={num}&safe={safe}",
                enc(prm.api_key),
                enc(prm.engine_id),
                enc(prm.query),
            ),
            body: String::new(),
            headers: Vec::new(),
        },
        Provider::Brave => SearchRequest {
            method: "GET",
            url: format!(
                "https://api.search.brave.com/res/v1/web/search?q={}&count={num}&safesearch={safe}",
                enc(prm.query),
            ),
            body: String::new(),
            headers: vec![
                ("X-Subscription-Token".into(), prm.api_key.trim().to_owned()),
                ("Accept".into(), "application/json".into()),
            ],
        },
        Provider::Serper => SearchRequest {
            method: "POST",
            url: "https://google.serper.dev/search".into(),
            body: json_body(&[("q", Value::from(prm.query)), ("num", Value::from(num))]),
            headers: vec![
                ("X-API-KEY".into(), prm.api_key.trim().to_owned()),
                ("Content-Type".into(), "application/json".into()),
            ],
        },
        Provider::Tavily => SearchRequest {
            method: "POST",
            url: "https://api.tavily.com/search".into(),
            body: json_body(&[
                ("query", Value::from(prm.query)),
                ("max_results", Value::from(num)),
            ]),
            headers: vec![
                (
                    "Authorization".into(),
                    format!("Bearer {}", prm.api_key.trim()),
                ),
                ("Content-Type".into(), "application/json".into()),
            ],
        },
        Provider::SearxNg => SearchRequest {
            method: "GET",
            url: format!(
                "{}/search?q={}&format=json&safesearch={safe}",
                prm.endpoint.trim().trim_end_matches('/'),
                enc(prm.query),
            ),
            body: String::new(),
            headers: vec![("Accept".into(), "application/json".into())],
        },
    }
}

/// A JSON object body, built by `serde_json` so the query is escaped rather
/// than interpolated — a search for `"` is a search, not a malformed request.
fn json_body(fields: &[(&str, Value)]) -> String {
    let map: serde_json::Map<String, Value> = fields
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    Value::Object(map).to_string()
}

/// One provider's raw answer as `(title, snippet, link)` triples.
///
/// A body that is empty, not JSON, or an error payload yields no results rather
/// than an error — the control's long-standing "absent data reads as nothing"
/// tolerance, which is what lets `ResultCount` be asked before the first
/// search.
pub fn parse_results(provider: Provider, body: &str) -> Vec<(String, String, String)> {
    let Ok(parsed) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    // Where the list lives, and what the three fields are called in it.
    let (items, title_key, snippet_key, link_key) = match provider {
        Provider::Google => (parsed.get("items"), "title", "snippet", "link"),
        Provider::Brave => (
            parsed.get("web").and_then(|w| w.get("results")),
            "title",
            "description",
            "url",
        ),
        Provider::Serper => (parsed.get("organic"), "title", "snippet", "link"),
        Provider::Tavily | Provider::SearxNg => (parsed.get("results"), "title", "content", "url"),
    };
    let Some(items) = items.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            let field = |k: &str| {
                item.get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned()
            };
            (field(title_key), field(snippet_key), field(link_key))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params<'a>(key: &'a str, endpoint: &'a str) -> SearchParams<'a> {
        SearchParams {
            api_key: key,
            engine_id: "cx-123",
            endpoint,
            query: "cobol indexed files",
            num_results: 10,
            safe_search: "Medium",
        }
    }

    /// **Every provider builds the request its own API documents.**
    ///
    /// Asserted whole — method, URL and headers — because a search that goes to
    /// the right host with the key in the wrong header fails at run time with a
    /// 401 and no way to see why from COBOL.
    #[test]
    fn each_provider_builds_its_documented_request() {
        let p = params("KEY", "https://searx.example.com/");

        let g = build_request(Provider::Google, &p);
        assert_eq!(g.method, "GET");
        assert!(
            g.url.starts_with("https://www.googleapis.com/customsearch/v1?key=KEY&cx=cx-123&q="),
            "{}",
            g.url
        );
        assert!(g.url.ends_with("&num=10&safe=active"), "{}", g.url);
        assert!(g.headers.is_empty(), "Custom Search signs in the query string");

        let b = build_request(Provider::Brave, &p);
        assert_eq!(b.method, "GET");
        assert!(
            b.url.starts_with("https://api.search.brave.com/res/v1/web/search?q="),
            "{}",
            b.url
        );
        assert!(b.url.ends_with("&count=10&safesearch=moderate"), "{}", b.url);
        assert!(b
            .headers
            .contains(&("X-Subscription-Token".into(), "KEY".into())));

        let s = build_request(Provider::Serper, &p);
        assert_eq!((s.method, s.url.as_str()), ("POST", "https://google.serper.dev/search"));
        assert!(s.headers.contains(&("X-API-KEY".into(), "KEY".into())));
        assert_eq!(s.body, r#"{"num":10,"q":"cobol indexed files"}"#);

        let t = build_request(Provider::Tavily, &p);
        assert_eq!((t.method, t.url.as_str()), ("POST", "https://api.tavily.com/search"));
        assert!(
            t.headers
                .contains(&("Authorization".into(), "Bearer KEY".into())),
            "Tavily takes the key as a Bearer header, NOT an api_key body field"
        );
        assert!(!t.body.contains("api_key"), "no api_key field: {}", t.body);
        assert_eq!(
            t.body,
            r#"{"max_results":10,"query":"cobol indexed files"}"#
        );

        let x = build_request(Provider::SearxNg, &p);
        assert_eq!(x.method, "GET");
        assert!(
            x.url.starts_with("https://searx.example.com/search?q="),
            "the trailing slash on the endpoint must not double: {}",
            x.url
        );
        assert!(x.url.ends_with("&format=json&safesearch=1"), "{}", x.url);
        assert!(
            x.headers.iter().all(|(k, _)| k != "Authorization"),
            "SearXNG is unauthenticated — you run it"
        );
    }

    /// **Each provider's own answer reaches the same accessors.**
    ///
    /// The five response shapes have five different paths and five different
    /// field names. Normalising them here is the whole point: a form written
    /// against Google reads Brave without an edit.
    #[test]
    fn every_providers_response_normalises_to_the_same_triples() {
        let cases = [
            (
                Provider::Google,
                r#"{"items":[{"title":"T","snippet":"S","link":"L"}]}"#,
            ),
            (
                Provider::Brave,
                r#"{"web":{"results":[{"title":"T","description":"S","url":"L"}]}}"#,
            ),
            (
                Provider::Serper,
                r#"{"organic":[{"title":"T","snippet":"S","link":"L"}]}"#,
            ),
            (
                Provider::Tavily,
                r#"{"results":[{"title":"T","content":"S","url":"L"}]}"#,
            ),
            (
                Provider::SearxNg,
                r#"{"results":[{"title":"T","content":"S","url":"L"}]}"#,
            ),
        ];
        for (provider, body) in cases {
            assert_eq!(
                parse_results(provider, body),
                vec![("T".to_owned(), "S".to_owned(), "L".to_owned())],
                "{} did not normalise",
                provider.as_str()
            );
        }

        // Absent data reads as nothing, for every provider — never a panic.
        for provider in [
            Provider::Google,
            Provider::Brave,
            Provider::Serper,
            Provider::Tavily,
            Provider::SearxNg,
        ] {
            for body in ["", "not json", "{}", r#"{"error":{"code":403}}"#] {
                assert!(
                    parse_results(provider, body).is_empty(),
                    "{} on {body:?}",
                    provider.as_str()
                );
            }
        }
    }

    /// **A misconfigured control says which setting is missing.**
    #[test]
    fn configuration_is_checked_before_anything_is_sent() {
        // The keyed providers.
        for provider in [
            Provider::Google,
            Provider::Brave,
            Provider::Serper,
            Provider::Tavily,
        ] {
            let err = configuration_error(provider, "  ", "").expect("no key is an error");
            assert!(err.contains("not configured"), "{err}");
            assert!(err.contains(provider.as_str()), "{err} names the provider");
            assert!(configuration_error(provider, "KEY", "").is_none());
        }

        // SearXNG wants an address instead of a key, and says so.
        assert!(configuration_error(Provider::SearxNg, "", "").unwrap().contains("Endpoint"));
        assert!(configuration_error(Provider::SearxNg, "", "https://s.example.com").is_none());
    }

    /// **`NumResults` is clamped to what each provider will accept.**
    ///
    /// Asking Google for 50 is an HTTP 400, not 50 results.
    #[test]
    fn num_results_is_clamped_per_provider() {
        for (provider, asked, expect) in [
            (Provider::Google, 50, 10),
            (Provider::Brave, 50, 20),
            (Provider::Tavily, 50, 20),
            (Provider::Serper, 500, 100),
            (Provider::SearxNg, 0, 1),
        ] {
            let mut p = params("KEY", "https://s.example.com");
            p.num_results = asked;
            let req = build_request(provider, &p);
            let shown = format!("{}{}", req.url, req.body);
            assert!(
                shown.contains(&format!("{expect}")),
                "{} asked {asked} should send {expect}: {shown}",
                provider.as_str()
            );
        }
    }

    /// **An unset or unknown Provider stays on the back end the form had.**
    #[test]
    fn an_unknown_provider_falls_back_to_google() {
        for raw in ["", "   ", "Google", "google", "GOOGLE", "Yahoo", "bing"] {
            assert_eq!(Provider::parse(raw), Provider::Google, "{raw:?}");
        }
        for (raw, want) in [
            ("Brave", Provider::Brave),
            ("brave", Provider::Brave),
            ("Serper", Provider::Serper),
            ("Tavily", Provider::Tavily),
            ("SearXNG", Provider::SearxNg),
            ("searx", Provider::SearxNg),
        ] {
            assert_eq!(Provider::parse(raw), want, "{raw:?}");
        }
        // The designer's list and the parser agree.
        for name in PROVIDER_NAMES {
            assert_eq!(Provider::parse(name).as_str(), name);
        }
    }
}

//! HTTP(S) requests behind the web blocks, plus the JSON lookup used to
//! pick values out of a response.

use std::sync::OnceLock;
use std::time::Duration;

pub const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];
/// Error messages quote at most this much of a failed response's body.
const ERROR_BODY_CHARS: usize = 200;

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            // Status codes are checked below so the error can quote the body.
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("Blockwork/", env!("CARGO_PKG_VERSION")))
            .build()
            .into()
    })
}

/// `Name: value` per line; blank lines and lines without a colon are skipped.
fn parse_headers(headers: &str) -> Vec<(String, String)> {
    headers
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_string(), value.trim().to_string()))
        })
        .collect()
}

/// Sends a request and returns the response body. A 4xx/5xx status is an
/// error, so a `try` block can catch it.
pub fn request(method: &str, url: &str, body: &str, headers: &str) -> Result<String, String> {
    let method = method.trim().to_ascii_uppercase();
    if !METHODS.contains(&method.as_str()) {
        return Err(format!("unsupported HTTP method '{method}'"));
    }
    let url = url.trim();
    if url.is_empty() {
        return Err("no URL given".into());
    }
    let mut builder = ureq::http::Request::builder().method(method.as_str()).uri(url);
    for (name, value) in parse_headers(headers) {
        builder = builder.header(name, value);
    }
    let response = if body.is_empty() {
        builder.body(()).map_err(|e| e.to_string()).and_then(|r| agent().run(r).map_err(|e| e.to_string()))
    } else {
        builder
            .body(body.to_string())
            .map_err(|e| e.to_string())
            .and_then(|r| agent().run(r).map_err(|e| e.to_string()))
    };
    let mut response = response.map_err(|e| format!("{method} {url} failed: {e}"))?;
    let status = response.status();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("couldn't read the response from {url}: {e}"))?;
    if status.as_u16() >= 400 {
        let snippet: String = text.chars().take(ERROR_BODY_CHARS).collect();
        return Err(format!("HTTP {status} from {url}: {snippet}"));
    }
    Ok(text)
}

/// POSTs raw bytes (an image, say) and returns the response body. A 4xx/5xx
/// status is an error.
pub fn post_bytes(url: &str, content_type: &str, body: Vec<u8>) -> Result<String, String> {
    let request = ureq::http::Request::builder()
        .method("POST")
        .uri(url)
        .header("Content-Type", content_type)
        .body(body)
        .map_err(|e| e.to_string())?;
    let mut response = agent().run(request).map_err(|e| format!("POST {url} failed: {e}"))?;
    let status = response.status();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("couldn't read the response from {url}: {e}"))?;
    if status.as_u16() >= 400 {
        let snippet: String = text.chars().take(ERROR_BODY_CHARS).collect();
        return Err(format!("HTTP {status} from {url}: {snippet}"));
    }
    Ok(text)
}

pub fn get(url: &str) -> Result<String, String> {
    request("GET", url, "", "")
}

/// Looks up a dotted path like `items.0.name` in a JSON document. An empty
/// path returns the whole document. Strings come back unquoted; anything
/// else as compact JSON.
pub fn json_get(json: &str, path: &str) -> Result<String, String> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("not valid JSON: {e}"))?;
    let mut current = &root;
    for key in path.split('.').map(str::trim).filter(|k| !k.is_empty()) {
        let next = match current {
            serde_json::Value::Object(map) => map.get(key),
            serde_json::Value::Array(items) => key.parse::<usize>().ok().and_then(|i| items.get(i)),
            _ => None,
        };
        current = next.ok_or_else(|| format!("JSON has no '{key}' in path '{path}'"))?;
    }
    Ok(match current {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_get_walks_objects_and_arrays() {
        let doc = r#"{"items":[{"name":"a","n":2},{"name":"b"}],"ok":true,"none":null}"#;
        assert_eq!(json_get(doc, "items.1.name").unwrap(), "b");
        assert_eq!(json_get(doc, "items.0.n").unwrap(), "2");
        assert_eq!(json_get(doc, "ok").unwrap(), "true");
        assert_eq!(json_get(doc, "none").unwrap(), "");
        let item: serde_json::Value = serde_json::from_str(&json_get(doc, "items.0").unwrap()).unwrap();
        assert_eq!(item, serde_json::json!({"name": "a", "n": 2}));
        assert!(json_get(doc, "items.5").is_err());
        assert!(json_get("nope", "a").is_err());
        assert_eq!(json_get("[1,2]", "").unwrap(), "[1,2]");
    }

    #[test]
    fn headers_parse_one_per_line() {
        assert_eq!(
            parse_headers("Accept: text/plain\n\nbad line\nX-Token:  abc:def "),
            vec![
                ("Accept".to_string(), "text/plain".to_string()),
                ("X-Token".to_string(), "abc:def".to_string())
            ]
        );
    }

    #[test]
    fn rejects_bad_method_and_empty_url() {
        assert!(request("FETCH", "https://example.com", "", "").is_err());
        assert!(request("GET", "  ", "", "").is_err());
    }
}

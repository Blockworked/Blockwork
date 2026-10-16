//! A detector running as a separate local service, so no model runtime has
//! to ship inside Blockwork. Each frame is POSTed as a PNG; the reply lists
//! boxes in that image's pixels:
//!
//! ```json
//! [{"x": 40, "y": 20, "w": 64, "h": 128, "label": "player", "score": 0.87}]
//! ```
//!
//! `{"detections": [...]}` works too, as do `width`/`height`, corner pairs
//! `x1,y1,x2,y2`, `class`/`name` for the label and `confidence` for the score.
//! Only loopback addresses are accepted, so captures never leave the machine.

use super::{Detection, Detector};
use crate::screen::Image;
use serde_json::Value as Json;

pub struct HttpDetector {
    url: String,
}

impl HttpDetector {
    pub fn new(url: &str) -> Result<HttpDetector, String> {
        let url = url.trim();
        if url.is_empty() {
            return Err("no detector given (start a local detector and enter its URL)".into());
        }
        let uri: ureq::http::Uri = url.parse().map_err(|_| format!("'{url}' isn't a detector URL"))?;
        if !matches!(uri.scheme_str(), Some("http" | "https")) {
            return Err(format!("'{url}' isn't a detector URL (use http:// or https://)"));
        }
        let host = uri.host().unwrap_or_default().trim_matches(['[', ']']);
        if !is_loopback(host) {
            return Err(format!("the detector must run on this computer (localhost), not {host}"));
        }
        Ok(HttpDetector { url: url.to_string() })
    }
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

impl Detector for HttpDetector {
    fn detect(&self, image: &Image) -> Result<Vec<Detection>, String> {
        let reply = crate::http::post_bytes(&self.url, "image/png", image.to_png()?)?;
        parse(&reply)
    }
}

/// Reads a detector's reply; see the module docs for the accepted shapes.
pub fn parse(reply: &str) -> Result<Vec<Detection>, String> {
    let root: Json = serde_json::from_str(reply).map_err(|e| format!("the detector's reply isn't JSON: {e}"))?;
    let items = match &root {
        Json::Array(items) => items,
        Json::Object(map) => match ["detections", "predictions", "boxes"].iter().find_map(|k| map.get(*k)) {
            Some(Json::Array(items)) => items,
            _ => return Err("the detector's reply has no list of detections".into()),
        },
        _ => return Err("the detector's reply has no list of detections".into()),
    };
    items.iter().map(one).collect()
}

fn one(item: &Json) -> Result<Detection, String> {
    let num = |keys: &[&str]| keys.iter().find_map(|k| item.get(*k).and_then(Json::as_f64));
    let (x, y, w, h) = match (num(&["x"]), num(&["y"]), num(&["w", "width"]), num(&["h", "height"])) {
        (Some(x), Some(y), Some(w), Some(h)) => (x, y, w, h),
        _ => match (num(&["x1"]), num(&["y1"]), num(&["x2"]), num(&["y2"])) {
            (Some(x1), Some(y1), Some(x2), Some(y2)) => (x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs()),
            _ => return Err(format!("a detection has no box: {item}")),
        },
    };
    let label = ["label", "class", "name"]
        .iter()
        .find_map(|k| match item.get(*k) {
            Some(Json::String(s)) => Some(s.clone()),
            Some(Json::Number(n)) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default();
    let score = num(&["score", "confidence"]).unwrap_or(1.0);
    // Some detectors report percentages.
    let score = if score > 1.0 { score / 100.0 } else { score };
    Ok(Detection { x, y, w, h, label, score: score.clamp(0.0, 1.0), id: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_urls_are_accepted() {
        for ok in ["http://127.0.0.1:8765/detect", "http://localhost:9000", "https://[::1]:1/x", "http://127.4.5.6/"] {
            assert!(HttpDetector::new(ok).is_ok(), "{ok}");
        }
        for bad in ["", "mc-player-320-v1", "http://example.com/detect", "http://192.168.1.4:8765", "ftp://127.0.0.1/"] {
            assert!(HttpDetector::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn posts_the_frame_as_png_and_reads_the_reply() {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let (mut length, mut content_type) = (0, String::new());
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let lower = line.trim().to_ascii_lowercase();
                if lower.is_empty() {
                    break;
                }
                if let Some(v) = lower.strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                if let Some(v) = lower.strip_prefix("content-type:") {
                    content_type = v.trim().to_string();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let reply = r#"{"detections": [{"x": 1, "y": 2, "w": 3, "h": 4, "label": "player", "score": 0.5}]}"#;
            let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
            reader.get_mut().write_all(response.as_bytes()).unwrap();
            (content_type, body)
        });
        let detector = HttpDetector::new(&format!("http://127.0.0.1:{port}/detect")).unwrap();
        let found = detector.detect(&Image::new(8, 6)).unwrap();
        assert_eq!(found, vec![Detection { x: 1.0, y: 2.0, w: 3.0, h: 4.0, label: "player".into(), score: 0.5, id: None }]);
        let (content_type, body) = server.join().unwrap();
        assert_eq!(content_type, "image/png");
        assert_eq!(Image::from_png(&body).map(|i| (i.width, i.height)), Ok((8, 6)));
    }

    /// Against a running `examples/detector/detector.py --color ff0000`; run
    /// by hand with `--ignored` (URL from `BLOCKWORK_TEST_DETECTOR` if set).
    #[test]
    #[ignore]
    fn finds_a_red_square_with_the_example_detector() {
        let url = std::env::var("BLOCKWORK_TEST_DETECTOR").unwrap_or("http://127.0.0.1:8765/detect".into());
        let mut image = Image::new(320, 200);
        for y in 60..100 {
            for x in 200..240 {
                let i = (y * 320 + x) * 4;
                image.rgba[i..i + 3].copy_from_slice(&[255, 0, 0]);
            }
        }
        let found = HttpDetector::new(&url).unwrap().detect(&image).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].center(), (220.0, 80.0));
    }

    #[test]
    fn replies_parse_in_every_supported_shape() {
        let want = Detection { x: 40.0, y: 20.0, w: 64.0, h: 128.0, label: "player".into(), score: 0.87, id: None };
        let plain = r#"[{"x": 40, "y": 20, "w": 64, "h": 128, "label": "player", "score": 0.87}]"#;
        assert_eq!(parse(plain), Ok(vec![want.clone()]));
        let wrapped = r#"{"detections": [{"x": 40, "y": 20, "width": 64, "height": 128, "class": "player", "confidence": 87}]}"#;
        assert_eq!(parse(wrapped), Ok(vec![want.clone()]));
        let corners = r#"{"predictions": [{"x1": 104, "y1": 148, "x2": 40, "y2": 20, "name": "player", "score": 0.87}]}"#;
        assert_eq!(parse(corners), Ok(vec![want]));
        assert_eq!(parse("[]"), Ok(vec![]));
        assert!(parse("nope").is_err());
        assert!(parse(r#"{"other": 1}"#).is_err());
        assert!(parse(r#"[{"x": 1}]"#).is_err());
    }
}

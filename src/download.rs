use std::io::Read;
use std::time::Duration;

pub const USER_AGENT: &str = "ModlandSearch/1.0 (+https://neofastftldev.nl)";

/// Fetch a URL into memory with a sane timeout and size cap.
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(120))
        .call()
        .map_err(|error| format!("{error}"))?;
    let mut data = Vec::new();
    response
        .into_reader()
        .take(128 * 1024 * 1024)
        .read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    Ok(data)
}

/// Fetch a URL and decode it as UTF-8 text (used by the live search APIs).
pub fn fetch_text(url: &str, timeout: Duration) -> Result<String, String> {
    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(timeout)
        .call()
        .map_err(|error| format!("{error}"))?;
    let mut text = String::new();
    response
        .into_reader()
        .take(16 * 1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    Ok(text)
}

/// Percent-encode one URL path segment, leaving unreserved characters alone.
pub fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.as_bytes() {
        let unreserved = byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~');
        if unreserved {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Build the download URL for a catalogue path such as `Fasttracker 2/Elwood/deadlock.xm`.
pub fn module_url(path: &str) -> String {
    let encoded: Vec<String> = path.split('/').map(encode_segment).collect();
    format!("https://modland.com/pub/modules/{}", encoded.join("/"))
}

/// Decode the handful of HTML entities the archive pages emit.
pub fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// Make text safe to use as one segment of a Windows path.
pub fn sanitize_segment(segment: &str) -> String {
    let cleaned: String = segment
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();
    let trimmed = cleaned.trim_matches(|ch: char| ch == ' ' || ch == '.');
    if trimmed.is_empty() {
        "_".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn human_size(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 {
        format!("{:.0} KB", bytes / 1024.0)
    } else {
        format!("{:.0} B", bytes)
    }
}

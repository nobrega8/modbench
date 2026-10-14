//! Network features: the online map library (the mbmap-library repository)
//! and vendor icons fetched from `@vendor_website`, cached on disk.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

const LIBRARY_RAW: &str = "https://raw.githubusercontent.com/nobrega8/mbmap-library/main/";
const MAX_MAP_BYTES: usize = 4 * 1024 * 1024;
const MAX_PAGE_BYTES: usize = 1024 * 1024;
const MAX_ICON_BYTES: usize = 512 * 1024;
/// A site without a usable icon is asked again after this long.
const RETRY_MISSING: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineEntry {
    pub path: String,
    pub vendor: String,
    pub model: String,
    pub description: Option<String>,
    pub vendor_website: Option<String>,
    /// "unverified" for maps converted from other catalogues.
    #[serde(default)]
    pub status: Option<String>,
    pub registers: usize,
}

#[derive(Deserialize)]
struct Index {
    maps: Vec<OnlineEntry>,
}

fn client() -> Result<Client, String> {
    // The process-wide TLS crypto provider; a no-op once one is installed.
    let _ = rustls::crypto::ring::default_provider().install_default();
    Client::builder()
        .user_agent(concat!("Modbench/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// GET with a size cap; returns the final URL, the content type and the body.
async fn get(client: &Client, url: Url, cap: usize) -> Result<(Url, String, Vec<u8>), String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.without_url().to_string())?;
    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        body.extend_from_slice(&chunk);
        if body.len() > cap {
            return Err("response too large".into());
        }
    }
    Ok((final_url, content_type, body))
}

/// The maps published in the online library.
pub async fn index() -> Result<Vec<OnlineEntry>, String> {
    let client = client()?;
    let url = Url::parse(LIBRARY_RAW)
        .and_then(|u| u.join("index.json"))
        .map_err(|e| e.to_string())?;
    let (_, _, body) = get(&client, url, MAX_MAP_BYTES)
        .await
        .map_err(|e| format!("Could not reach the online library: {e}"))?;
    let index: Index = serde_json::from_slice(&body)
        .map_err(|e| format!("The online library index could not be read: {e}"))?;
    Ok(index
        .maps
        .into_iter()
        .filter(|m| is_library_path(&m.path))
        .collect())
}

/// Only `maps/<vendor>/<model>.mbmap` slugs are downloaded, so the index
/// cannot point the app anywhere else.
pub fn is_library_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let slug = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    parts.len() == 3
        && parts[0] == "maps"
        && slug(parts[1])
        && parts[2].strip_suffix(".mbmap").is_some_and(slug)
}

/// Downloads one map of the online library.
pub async fn download(path: &str) -> Result<Vec<u8>, String> {
    if !is_library_path(path) {
        return Err(format!("{path} is not a map of the online library"));
    }
    let client = client()?;
    let url = Url::parse(LIBRARY_RAW)
        .and_then(|u| u.join(path))
        .map_err(|e| e.to_string())?;
    let (_, _, body) = get(&client, url, MAX_MAP_BYTES).await?;
    Ok(body)
}

/// `se.com` or `https://www.se.com/x` → the site's host and its home page.
fn site(website: &str) -> Option<(String, Url)> {
    let text = website.trim();
    let with_scheme = if text.starts_with("http://") || text.starts_with("https://") {
        text.to_string()
    } else {
        format!("https://{text}")
    };
    let url = Url::parse(&with_scheme).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let home = Url::parse(&format!("{}://{}/", url.scheme(), url.authority())).ok()?;
    Some((host, home))
}

/// One `<link>` tag's attributes, lowercased names.
fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
            i += 1;
        }
        let name = tag[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            let value = if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                let from = i + 1;
                let to = tag[from..]
                    .bytes()
                    .position(|b| b == quote)
                    .map_or(tag.len(), |p| from + p);
                i = to + 1;
                &tag[from..to.min(tag.len())]
            } else {
                let from = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                &tag[from..i]
            };
            out.push((name, value.replace("&amp;", "&")));
        } else if !name.is_empty() {
            out.push((name, String::new()));
        }
    }
    out
}

/// Icon links in a page, best first: larger sizes, SVG and touch icons win.
pub fn icon_links(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut found: Vec<(u32, String)> = Vec::new();
    let mut from = 0;
    while let Some(p) = lower[from..].find("<link") {
        let start = from + p;
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        let attrs = attributes(&html[start + 5..end]);
        from = end;
        let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        let rel = get("rel").unwrap_or("").to_ascii_lowercase();
        let Some(href) = get("href").filter(|h| !h.is_empty()) else {
            continue;
        };
        if !rel
            .split_whitespace()
            .any(|r| r == "icon" || r == "apple-touch-icon")
        {
            continue;
        }
        let size = get("sizes")
            .and_then(|s| {
                s.split_whitespace()
                    .filter_map(|d| d.split(['x', 'X']).next()?.parse().ok())
                    .max()
            })
            .unwrap_or(0);
        let svg =
            href.to_ascii_lowercase().ends_with(".svg") || get("type") == Some("image/svg+xml");
        let score = if svg {
            1000
        } else if size > 0 {
            size
        } else if rel.contains("apple-touch-icon") {
            180
        } else {
            16
        };
        found.push((score, href.to_string()));
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found.into_iter().map(|(_, h)| h).collect()
}

/// The image type of an icon file, from its first bytes.
fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        Some("image/jpeg")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
        head.contains("<svg").then_some("image/svg+xml")
    }
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(bytes))
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn cache_file(dir: &Path, host: &str) -> PathBuf {
    let safe: String = host
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(safe)
}

/// The vendor's icon as a `data:` URL, from the cache or the vendor's site.
/// `None` if the site has no icon or cannot be reached.
pub async fn vendor_icon(cache_dir: &Path, website: &str) -> Option<String> {
    let (host, home) = site(website)?;
    let file = cache_file(cache_dir, &host);
    if let Ok(bytes) = std::fs::read(&file) {
        if let Some(mime) = image_type(&bytes) {
            return Some(data_url(mime, &bytes));
        }
        // An empty file marks a site without an icon; ask again later.
        let fresh = std::fs::metadata(&file)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < RETRY_MISSING);
        if fresh {
            return None;
        }
    }
    let found = fetch_icon(&home).await;
    let _ = std::fs::create_dir_all(cache_dir);
    match found {
        Some((mime, bytes)) => {
            let _ = std::fs::write(&file, &bytes);
            Some(data_url(mime, &bytes))
        }
        None => {
            let _ = std::fs::write(&file, b"");
            None
        }
    }
}

async fn fetch_icon(home: &Url) -> Option<(&'static str, Vec<u8>)> {
    let client = client().ok()?;
    if let Some(found) = icon_from_site(&client, home).await {
        return Some(found);
    }
    // Some sites only answer on www. (e.g. daikin.com)
    let host = home.host_str()?;
    if host.starts_with("www.") {
        return None;
    }
    let mut www = home.clone();
    www.set_host(Some(&format!("www.{host}"))).ok()?;
    icon_from_site(&client, &www).await
}

async fn icon_from_site(client: &Client, home: &Url) -> Option<(&'static str, Vec<u8>)> {
    let mut candidates: Vec<Url> = Vec::new();
    if let Ok((page_url, content_type, body)) = get(client, home.clone(), MAX_PAGE_BYTES).await
        && (content_type.contains("html") || content_type.is_empty())
    {
        let html = String::from_utf8_lossy(&body);
        candidates.extend(
            icon_links(&html)
                .iter()
                .filter_map(|h| page_url.join(h).ok()),
        );
    }
    if let Ok(ico) = home.join("/favicon.ico") {
        candidates.push(ico);
    }
    for url in candidates.into_iter().take(4) {
        if !matches!(url.scheme(), "http" | "https") {
            continue;
        }
        if let Ok((_, _, bytes)) = get(client, url, MAX_ICON_BYTES).await
            && let Some(mime) = image_type(&bytes)
        {
            return Some((mime, bytes));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_paths() {
        assert!(is_library_path("maps/deep-sea-electronics/dse335.mbmap"));
        assert!(!is_library_path("maps/../secrets.mbmap"));
        assert!(!is_library_path("examples/x/y.mbmap"));
        assert!(!is_library_path("maps/a/b.txt"));
        assert!(!is_library_path("https://evil.example/maps/a/b.mbmap"));
    }

    #[test]
    fn sites() {
        let (host, home) = site("se.com").unwrap();
        assert_eq!(
            (host.as_str(), home.as_str()),
            ("se.com", "https://se.com/")
        );
        let (host, home) = site("http://www.Daikin.com/en/products").unwrap();
        assert_eq!(
            (host.as_str(), home.as_str()),
            ("www.daikin.com", "http://www.daikin.com/")
        );
    }

    #[test]
    fn icon_link_order() {
        let html = r#"<head>
            <link rel="stylesheet" href="/a.css">
            <link rel="shortcut icon" href="/favicon.ico">
            <LINK REL="icon" type="image/png" sizes="32x32" href="/f32.png">
            <link rel=apple-touch-icon href=/touch.png>
            <link rel="icon" sizes="16x16 48x48" href='/multi.ico'>
        </head>"#;
        assert_eq!(
            icon_links(html),
            ["/touch.png", "/multi.ico", "/f32.png", "/favicon.ico"]
        );
        assert_eq!(icon_links(r#"<link rel="icon" href="/i.svg">"#), ["/i.svg"]);
    }

    #[test]
    fn image_types_and_base64() {
        assert_eq!(image_type(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(image_type(&[0, 0, 1, 0, 1]), Some("image/x-icon"));
        assert_eq!(image_type(b"<?xml?><svg xmlns=''>"), Some("image/svg+xml"));
        assert_eq!(image_type(b"<html>"), None);
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}

//! Minimal HTTP helpers over `ureq` for loopback proxy calls and provider
//! key-validation probes. Returns `(status, body)` so callers can map errors.

use std::time::Duration;

pub struct HttpResult {
    pub status: u16,
    pub body: String,
}

fn agent(timeout_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(timeout_secs))
        .build()
}

pub fn get(url: &str, headers: &[(&str, &str)], timeout_secs: u64) -> Result<HttpResult, String> {
    let mut req = agent(timeout_secs).get(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    exec(req, None)
}

pub fn post_json(
    url: &str,
    headers: &[(&str, &str)],
    body: &serde_json::Value,
    timeout_secs: u64,
) -> Result<HttpResult, String> {
    let mut req = agent(timeout_secs).post(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    exec(req, Some(body.to_string().into_bytes()))
}

fn exec(req: ureq::Request, body: Option<Vec<u8>>) -> Result<HttpResult, String> {
    // HTTP error statuses are DATA (callers map them to friendly messages);
    // only transport failures are Err.
    let resp = match body {
        Some(b) => req.set("Content-Type", "application/json").send_bytes(&b),
        None => req.call(),
    };
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            return Ok(HttpResult { status: code, body: text });
        }
        Err(other) => return Err(format!("network error: {other}")),
    };
    let status = resp.status();
    let text = resp
        .into_string()
        .map_err(|e| format!("response read failed: {e}"))?;
    Ok(HttpResult { status, body: text })
}

/// Map an upstream/provider HTTP status to a user-friendly Turkish message.
pub fn friendly_http_error(status: u16, body: &str) -> String {
    let snippet: String = body.chars().take(300).collect();
    match status {
        401 => format!("Kimlik doğrulama reddedildi (401). API Key veya oturum geçersiz. {snippet}"),
        402 | 403 => format!("Erişim reddedildi ({status}). Anahtar bu kaynağa yetkili değil veya bakiye/kota yetersiz. {snippet}"),
        404 => format!("Endpoint bulunamadı (404). {snippet}"),
        429 => format!("Kota/limit aşıldı (429). {snippet}"),
        503 => format!("Servis geçici olarak kullanılamıyor (503). {snippet}"),
        s if s >= 500 => format!("Sağlayıcı hatası ({s}). {snippet}"),
        s => format!("HTTP {s}: {snippet}"),
    }
}

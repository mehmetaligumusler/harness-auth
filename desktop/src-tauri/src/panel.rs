//! Client for the proxy's token-guarded control panel
//! (`POST /api/control`, see `src/server/panel.ts` and `src/control.ts`).

use serde_json::{json, Value};

use crate::http;

pub struct PanelClient {
    pub port: u16,
    pub token: String,
}

impl PanelClient {
    pub fn new(port: u16, token: &str) -> Self {
        Self {
            port,
            token: token.to_string(),
        }
    }

    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Panel liveness probe (tokenless by design in the proxy).
    pub fn healthz(&self) -> bool {
        http::get(
            &format!("{}/healthz", self.base()),
            &[],
            3,
        )
        .map(|r| r.status == 200)
        .unwrap_or(false)
    }

    /// Send a control command. The panel answers HTTP 200 with an envelope
    /// `{ok: true, ...}` or `{ok: false, error}` — both are returned as `Ok`;
    /// transport problems become `Err`.
    pub fn control(&self, cmd: Value, timeout_secs: u64) -> Result<Value, String> {
        let res = http::post_json(
            &format!("{}/api/control", self.base()),
            &[
                ("X-Panel-Token", self.token.as_str()),
                ("Authorization", &format!("Bearer {}", self.token)),
            ],
            &cmd,
            timeout_secs,
        )?;
        if res.status != 200 {
            return Err(format!(
                "panel HTTP {}: {}",
                res.status,
                res.body.chars().take(200).collect::<String>()
            ));
        }
        serde_json::from_str(&res.body).map_err(|e| format!("panel yanıtı çözümlenemedi: {e}"))
    }

    /// `status` command — provider/plan/proxyPort/loggedIn.
    pub fn status(&self) -> Result<Value, String> {
        let resp = self.control(json!({ "cmd": "status" }), 10)?;
        if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(resp)
        } else {
            Err(resp
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("bilinmeyen panel hatası")
                .to_string())
        }
    }

    pub fn quota(&self) -> Result<Value, String> {
        let resp = self.control(json!({ "cmd": "quota" }), 45)?;
        if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(resp)
        } else {
            Err(resp
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("kota bilgisi alınamadı")
                .to_string())
        }
    }

    pub fn logs(&self, since: i64) -> Result<Value, String> {
        self.control(json!({ "cmd": "getLogs", "since": since }), 10)
    }

    pub fn start_server(&self) -> Result<u16, String> {
        let resp = self.control(json!({ "cmd": "startProxy" }), 30)?;
        if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(resp.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16)
        } else {
            Err(resp
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("başlatılamadı")
                .to_string())
        }
    }

    pub fn stop_server(&self) -> Result<(), String> {
        let resp = self.control(json!({ "cmd": "stopProxy" }), 30)?;
        if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(())
        } else {
            Err(resp
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("durdurulamadı")
                .to_string())
        }
    }

    /// Ask the proxy process to exit cleanly. Returns Ok even if the process
    /// is already gone.
    pub fn shutdown(&self) -> Result<(), String> {
        match self.control(json!({ "cmd": "shutdown" }), 10) {
            Ok(_) => Ok(()),
            // Connection refused => already down.
            Err(e) if e.contains("network error") || e.contains("panel HTTP") => Ok(()),
            Err(e) => Err(e),
        }
    }
}

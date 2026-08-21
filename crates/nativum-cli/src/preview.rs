//! Headless preview server: PNG frames + click/key over HTTP.
//! No windowing system required — useful in CI and cloud agents.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use nativum_core::Appearance;
use nativum_engine::{load_app_dir, Session};

use crate::files_changed;

pub fn serve(path: &Path, bind: SocketAddr, watch: bool) -> Result<()> {
    let session = Arc::new(Mutex::new(open(path)?));
    if watch {
        let session = Arc::clone(&session);
        let path = path.to_path_buf();
        thread::spawn(move || watch_loop(path, session));
    }
    let listener = TcpListener::bind(bind).with_context(|| format!("bind {bind}"))?;
    println!("nativum preview  http://{bind}");
    println!("  GET  /           HTML shell");
    println!("  GET  /frame.png  current surface");
    println!("  GET  /snapshot   accessibility JSON");
    println!("  POST /click      x,y");
    println!("  POST /press      kind");
    println!("  POST /key        key");
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let session = Arc::clone(&session);
                thread::spawn(move || {
                    if let Err(e) = handle(s, &session) {
                        eprintln!("preview: {e:#}");
                    }
                });
            }
            Err(e) => eprintln!("accept: {e}"),
        }
    }
    Ok(())
}

fn open(path: &Path) -> Result<Session> {
    let loaded = load_app_dir(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    Session::from_loaded(loaded).map_err(|e| anyhow::anyhow!("{e}"))
}

fn watch_loop(path: std::path::PathBuf, session: Arc<Mutex<Session>>) {
    let view = path.join("src/app.native");
    let core = path.join("src/core.json");
    let files = vec![view, core, path.join("app.json")];
    let mut last = Vec::new();
    files_changed(&files, &mut last);
    loop {
        thread::sleep(Duration::from_millis(400));
        if files_changed(&files, &mut last) {
            match open(&path) {
                Ok(next) => {
                    *session.lock().expect("session lock") = next;
                    eprintln!("preview: reloaded");
                }
                Err(e) => eprintln!("preview reload: {e:#}"),
            }
        }
    }
}

fn handle(mut stream: TcpStream, session: &Mutex<Session>) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let url = parts.next().unwrap_or("/");
    let (path, query) = match url.split_once('?') {
        Some((p, q)) => (p, q),
        None => (url, ""),
    };
    let body = req.split("\r\n\r\n").nth(1).unwrap_or("");

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            respond(&mut stream, "text/html; charset=utf-8", PAGE.as_bytes())?;
        }
        ("GET", "/frame.png") => {
            let png = session
                .lock()
                .expect("lock")
                .png()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            respond(&mut stream, "image/png", &png)?;
        }
        ("GET", "/snapshot") => {
            let json = {
                let s = session.lock().expect("lock");
                serde_json::to_vec_pretty(&s.snapshot())?
            };
            respond(&mut stream, "application/json", &json)?;
        }
        ("GET", "/model") => {
            let json = {
                let s = session.lock().expect("lock");
                serde_json::to_vec_pretty(&s.model().to_json())?
            };
            respond(&mut stream, "application/json", &json)?;
        }
        ("POST", "/click") => {
            let (x, y) = parse_xy(query, body)?;
            session
                .lock()
                .expect("lock")
                .click(x, y)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            respond(&mut stream, "text/plain", b"ok")?;
        }
        ("POST", "/press") => {
            let kind = param(query, body, "kind").unwrap_or_default();
            session
                .lock()
                .expect("lock")
                .press_kind(&kind, None)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            respond(&mut stream, "text/plain", b"ok")?;
        }
        ("POST", "/key") => {
            let key = param(query, body, "key").unwrap_or_default();
            session
                .lock()
                .expect("lock")
                .key(&key)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            respond(&mut stream, "text/plain", b"ok")?;
        }
        ("POST", "/appearance") => {
            let v = param(query, body, "value").unwrap_or_default();
            let a = if v == "dark" {
                Appearance::Dark
            } else {
                Appearance::Light
            };
            session
                .lock()
                .expect("lock")
                .set_appearance(a)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            respond(&mut stream, "text/plain", b"ok")?;
        }
        _ => {
            let msg = b"not found";
            let header = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                msg.len()
            );
            stream.write_all(header.as_bytes())?;
            stream.write_all(msg)?;
        }
    }
    Ok(())
}

fn parse_xy(query: &str, body: &str) -> Result<(f32, f32)> {
    let x = param(query, body, "x")
        .or_else(|| param(query, body, "x"))
        .context("missing x")?
        .parse::<f32>()?;
    let y = param(query, body, "y")
        .context("missing y")?
        .parse::<f32>()?;
    Ok((x, y))
}

fn param<'a>(query: &'a str, body: &'a str, name: &str) -> Option<String> {
    for hay in [query, body] {
        for pair in hay.split(&['&', '\n', ' '][..]) {
            if let Some((k, v)) = pair.split_once('=') {
                if k.trim() == name {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(hay) {
            if let Some(x) = v.get(name) {
                return Some(match x {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
            }
        }
    }
    None
}

fn respond(stream: &mut TcpStream, ctype: &str, body: &[u8]) -> Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

const PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<meta charset="utf-8">
<title>nativum preview</title>
<style>
  :root { color-scheme: light dark; }
  body { margin: 0; font: 14px/1.4 ui-sans-serif, system-ui, sans-serif;
         background: #141412; color: #f2f1ea; }
  header { display: flex; gap: 12px; align-items: center; padding: 12px 16px;
           border-bottom: 1px solid #3a3933; }
  header strong { letter-spacing: .04em; }
  button, select { background: #1e1e1b; color: inherit; border: 1px solid #3a3933;
                   border-radius: 8px; padding: 6px 10px; cursor: pointer; }
  main { display: flex; gap: 16px; padding: 16px; align-items: flex-start; }
  #frame { image-rendering: pixelated; background: #1e1e1b; border-radius: 12px;
           box-shadow: 0 12px 40px rgba(0,0,0,.4); cursor: crosshair; }
  pre { flex: 1; font: 12px/1.45 ui-monospace, monospace; background: #1e1e1b;
        padding: 12px; border-radius: 12px; overflow: auto; max-height: 80vh; }
</style>
<header>
  <strong>nativum</strong>
  <span>click the frame · keys go to the app when the page is focused</span>
  <select id="theme">
    <option value="light">light</option>
    <option value="dark">dark</option>
  </select>
  <button id="reload">reload png</button>
</header>
<main>
  <img id="frame" alt="app frame" />
  <pre id="snap">…</pre>
</main>
<script>
const img = document.getElementById('frame');
const snap = document.getElementById('snap');
async function refresh() {
  img.src = '/frame.png?' + Date.now();
  const [s, m] = await Promise.all([
    fetch('/snapshot').then(r => r.json()),
    fetch('/model').then(r => r.json()),
  ]);
  snap.textContent = JSON.stringify({model: m, snapshot: s}, null, 2);
}
img.addEventListener('click', async (e) => {
  const r = img.getBoundingClientRect();
  const x = (e.clientX - r.left) * (img.naturalWidth / r.width);
  const y = (e.clientY - r.top) * (img.naturalHeight / r.height);
  await fetch('/click?x=' + x + '&y=' + y, {method: 'POST'});
  refresh();
});
document.getElementById('theme').addEventListener('change', async (e) => {
  await fetch('/appearance?value=' + e.target.value, {method: 'POST'});
  refresh();
});
document.getElementById('reload').addEventListener('click', refresh);
window.addEventListener('keydown', async (e) => {
  if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;
  await fetch('/key?key=' + encodeURIComponent(e.key), {method: 'POST'});
  refresh();
});
refresh();
setInterval(refresh, 1500);
</script>
</html>
"#;

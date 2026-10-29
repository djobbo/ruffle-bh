//! Browser filesystem backend for AIR `File`/`FileStream`.
//!
//! Brawlhalla reads every asset synchronously through `FileStream`, then hands
//! the bytes to `Loader.loadBytes`. Browsers have no synchronous `fetch`, but
//! `XMLHttpRequest` still supports synchronous requests on the main thread, so
//! that is what backs reads here.
//!
//! The page configures the depot with:
//!
//! ```js
//! window.__ruffleDepot = { appDir: "/depot", url: "/depot/" };
//! ```

use js_sys::{Function, Reflect, Uint8Array};
use ruffle_core::backend::fs::{FileEntry, FileSystemBackend};
use std::io;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::XmlHttpRequest;

const DEFAULT_APP_DIR: &str = "/depot";
const DEFAULT_URL: &str = "/depot/";

pub struct WebFileSystem {
    app_dir: String,
    base_url: String,
    storage_dir: String,
}

impl WebFileSystem {
    pub fn new() -> Self {
        let (app_dir, base_url) = depot_config();
        let storage_dir = "/storage".to_string();
        Self {
            app_dir,
            base_url,
            storage_dir,
        }
    }

    fn relative(&self, path: &str) -> Option<String> {
        let path = path.replace('\\', "/");
        let path = path.trim_start_matches('/');
        let app = self.app_dir.trim_start_matches('/');
        let relative = path
            .strip_prefix(app)
            .map(|rest| rest.trim_start_matches('/'))
            .unwrap_or(path);
        if relative.is_empty() {
            None
        } else {
            Some(relative.to_string())
        }
    }

    fn from_memory(&self, path: &str) -> Option<Vec<u8>> {
        let relative = self.relative(path)?;
        let window = web_sys::window()?;
        let files = Reflect::get(&window, &"__ruffleFiles".into()).ok()?;
        if !files.is_object() {
            return None;
        }
        let value = Reflect::get(&files, &relative.into()).ok()?;
        let array = value.dyn_into::<Uint8Array>().ok()?;
        Some(array.to_vec())
    }

    fn request(&self, path: &str) -> io::Result<XmlHttpRequest> {
        let relative = self
            .relative(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.to_string()))?;
        let url = format!("{}{}", self.base_url, encode_path(&relative));

        let xhr = XmlHttpRequest::new().map_err(io_error)?;
        xhr.open_with_async("GET", &url, false).map_err(io_error)?;
        // `responseType` may not be set on synchronous requests, so use the
        // classic "x-user-defined" charset trick and decode the char codes.
        xhr.override_mime_type("text/plain; charset=x-user-defined")
            .map_err(io_error)?;
        xhr.send().map_err(io_error)?;
        Ok(xhr)
    }
}

impl Default for WebFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystemBackend for WebFileSystem {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        if let Some(bytes) = self.from_memory(path) {
            return Ok(bytes);
        }
        let xhr = self.request(path)?;
        let status = xhr.status().unwrap_or(0);
        if status != 200 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{path} (HTTP {status})"),
            ));
        }
        response_bytes(&xhr)
    }

    fn exists(&self, path: &str) -> bool {
        if self.from_memory(path).is_some() {
            return true;
        }
        match self.request(path) {
            Ok(xhr) => xhr.status().map(|status| status == 200).unwrap_or(false),
            Err(_) => false,
        }
    }

    fn is_directory(&self, _path: &str) -> bool {
        false
    }

    fn size(&self, path: &str) -> Option<u64> {
        if let Some(bytes) = self.from_memory(path) {
            return Some(bytes.len() as u64);
        }
        let xhr = self.request(path).ok()?;
        if xhr.status().ok()? != 200 {
            return None;
        }
        xhr.get_response_header("Content-Length")
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
    }

    fn list_directory(&self, _path: &str) -> io::Result<Vec<FileEntry>> {
        Ok(Vec::new())
    }

    fn application_directory(&self) -> String {
        self.app_dir.clone()
    }

    fn application_storage_directory(&self) -> String {
        self.storage_dir.clone()
    }

    fn user_directory(&self) -> String {
        self.storage_dir.clone()
    }

    fn documents_directory(&self) -> String {
        self.storage_dir.clone()
    }

    fn desktop_directory(&self) -> String {
        self.storage_dir.clone()
    }
}

fn depot_config() -> (String, String) {
    let Some(window) = web_sys::window() else {
        return (DEFAULT_APP_DIR.to_string(), DEFAULT_URL.to_string());
    };
    let Ok(config) = Reflect::get(&window, &"__ruffleDepot".into()) else {
        return (DEFAULT_APP_DIR.to_string(), DEFAULT_URL.to_string());
    };
    if !config.is_object() {
        return (DEFAULT_APP_DIR.to_string(), DEFAULT_URL.to_string());
    }
    let app_dir = Reflect::get(&config, &"appDir".into())
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_else(|| DEFAULT_APP_DIR.to_string());
    let url = Reflect::get(&config, &"url".into())
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    (app_dir, url)
}

/// Convert the `x-user-defined` response text to raw bytes in JavaScript.
///
/// `responseText` is a JavaScript string with one code unit per byte. Pulling
/// it into Rust and iterating `chars()` makes wasm-bindgen encode/decode every
/// byte as UTF-8, which is extremely slow for multi-megabyte SWFs. A single JS
/// loop is 1-2 orders of magnitude faster.
fn response_bytes(xhr: &XmlHttpRequest) -> io::Result<Vec<u8>> {
    thread_local! {
        static DECODE: Function = Function::new_with_args(
            "xhr",
            "const s = xhr.responseText; \
             const n = s.length; \
             const a = new Uint8Array(n); \
             for (let i = 0; i < n; i++) a[i] = s.charCodeAt(i) & 0xff; \
             return a;",
        );
    }

    DECODE.with(|decode| {
        let value = decode
            .call1(&JsValue::NULL, xhr.as_ref())
            .map_err(io_error)?;
        value
            .dyn_into::<Uint8Array>()
            .map(|array| array.to_vec())
            .map_err(io_error)
    })
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            js_sys::encode_uri_component(segment)
                .as_string()
                .unwrap_or_else(|| segment.to_string())
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn io_error(error: wasm_bindgen::JsValue) -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        format!("browser filesystem error: {error:?}"),
    )
}

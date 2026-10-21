use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicI32, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use tauri::{webview::Webview, LogicalPosition, LogicalSize, Rect, Runtime, Url};

use crate::{scripts, Config};

/// Mutable observation of a child webview, updated from Tauri and guest events.
#[derive(Clone, Debug)]
pub struct WebviewState {
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub waiting_for_response: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub history_index: i32,
    pub crashed: bool,
    pub audio_muted: bool,
    pub audible: bool,
    pub user_agent: String,
    pub zoom_factor: f64,
    pub zoom_level: f64,
    pub web_contents_id: i32,
    pub devtools_focused: bool,
    pub last_finished_url: String,
}

impl Default for WebviewState {
    fn default() -> Self {
        Self {
            url: String::new(),
            title: String::new(),
            loading: false,
            waiting_for_response: false,
            can_go_back: false,
            can_go_forward: false,
            history_index: 0,
            crashed: false,
            audio_muted: false,
            audible: false,
            user_agent: String::new(),
            zoom_factor: 1.0,
            zoom_level: 0.0,
            web_contents_id: 0,
            devtools_focused: false,
            last_finished_url: String::new(),
        }
    }
}

/// Shared handle to a webview state, usable from builder callbacks.
#[derive(Clone, Default)]
pub(crate) struct SharedState(Arc<Mutex<WebviewState>>);

impl SharedState {
    pub(crate) fn update(&self, update: impl FnOnce(&mut WebviewState)) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        update(&mut state);
    }

    pub(crate) fn snapshot(&self) -> WebviewState {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

/// State managed by the plugin. Holds every child webview created through it.
pub struct Registry<R: Runtime> {
    webviews: Mutex<HashMap<String, Webview<R>>>,
    states: Mutex<HashMap<String, SharedState>>,
    pending: Mutex<HashSet<String>>,
    css_sequence: Arc<AtomicU64>,
    contents_sequence: Arc<AtomicI32>,
    config: Config,
}

impl<R: Runtime> Registry<R> {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            webviews: Mutex::new(HashMap::new()),
            states: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashSet::new()),
            css_sequence: Arc::new(AtomicU64::new(0)),
            contents_sequence: Arc::new(AtomicI32::new(0)),
            config,
        }
    }

    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn state(&self, label: &str) -> SharedState {
        self.states
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entry(label.to_string())
            .or_default()
            .clone()
    }

    pub(crate) fn remove_state(&self, label: &str) -> Option<SharedState> {
        self.states
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(label)
    }

    pub(crate) fn next_contents_id(&self) -> i32 {
        self.contents_sequence.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Marks a label as owned by the plugin while its native webview is created.
    pub(crate) fn mark_child(&self, label: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(label.to_string());
    }

    pub(crate) fn unmark_child(&self, label: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(label);
    }

    /// Whether `label` belongs to a webview embedded by this plugin.
    pub fn is_child(&self, label: &str) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains(label)
            || self
                .webviews
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .contains_key(label)
    }

    pub(crate) fn insert(&self, label: &str, webview: Webview<R>) {
        self.webviews
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(label.to_string(), webview);
    }

    pub(crate) fn remove(&self, label: &str) -> Option<Webview<R>> {
        self.webviews
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(label)
    }

    /// Returns a handle for the child webview registered with `label`.
    pub fn get(&self, label: &str) -> Option<CefWebview<R>> {
        self.webviews
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(label)
            .cloned()
            .map(|webview| CefWebview {
                webview,
                css_sequence: Arc::clone(&self.css_sequence),
            })
    }

    /// Labels of every child webview currently managed by the plugin.
    pub fn labels(&self) -> Vec<String> {
        self.webviews
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .keys()
            .cloned()
            .collect()
    }
}

/// Handle to a child webview created by the plugin.
#[derive(Clone)]
pub struct CefWebview<R: Runtime> {
    pub(crate) webview: Webview<R>,
    css_sequence: Arc<AtomicU64>,
}

impl<R: Runtime> CefWebview<R> {
    /// Label of the child webview.
    pub fn label(&self) -> &str {
        self.webview.label()
    }

    /// Access the underlying Tauri webview.
    pub fn inner(&self) -> &Webview<R> {
        &self.webview
    }

    /// Evaluates JavaScript in the child webview.
    pub fn eval(&self, js: impl Into<String>) -> tauri::Result<()> {
        self.webview.eval(js)
    }

    /// Injects a stylesheet into the child webview and returns its key.
    pub fn insert_css(&self, css: &str) -> tauri::Result<String> {
        let key = format!(
            "cwv-css-{}",
            self.css_sequence.fetch_add(1, Ordering::Relaxed)
        );
        self.webview.eval(scripts::insert_style(&key, css))?;
        Ok(key)
    }

    /// Removes a stylesheet previously inserted with [`Self::insert_css`].
    pub fn remove_inserted_css(&self, key: &str) -> tauri::Result<()> {
        self.webview.eval(scripts::remove_style(key))
    }

    /// Navigates the child webview to `url`.
    pub fn navigate(&self, url: &str) -> Result<(), String> {
        let url = Url::parse(url).map_err(|error| error.to_string())?;
        self.webview
            .navigate(url)
            .map_err(|error| error.to_string())
    }

    /// Reloads the child webview.
    pub fn reload(&self) -> tauri::Result<()> {
        self.webview.reload()
    }

    /// Opens the child webview devtools (debug builds only).
    #[cfg(any(debug_assertions, feature = "devtools"))]
    pub fn open_devtools(&self) {
        self.webview.open_devtools();
    }

    /// No-op when devtools support is not compiled in.
    #[cfg(not(any(debug_assertions, feature = "devtools")))]
    pub fn open_devtools(&self) {}

    /// Moves and resizes the child webview. Values are logical pixels.
    pub fn set_rect(&self, x: f64, y: f64, width: f64, height: f64) -> tauri::Result<()> {
        self.webview.set_bounds(Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(width, height).into(),
        })
    }

    /// Shows or hides the child webview.
    pub fn set_visible(&self, visible: bool) -> tauri::Result<()> {
        if visible {
            self.webview.show()
        } else {
            self.webview.hide()
        }
    }

    /// Closes the child webview.
    pub fn close(&self) -> tauri::Result<()> {
        self.webview.close()
    }
}

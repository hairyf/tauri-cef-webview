//! Embed native child webviews in a Tauri application and control them from
//! the frontend with a plain `<webview>` element.

mod commands;
mod registry;
mod scripts;

pub use registry::{CefWebview, Registry};

use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Manager, Runtime,
};

/// Installed into every application page so `<webview>` works without imports.
/// Registered as a plugin initialization script, so it runs before any page
/// script in every webview, including windows created from the app config.
const HOST_RUNTIME: &str = include_str!("element.js.inc");

/// Plugin configuration.
#[derive(Clone, Debug, Default)]
pub struct Config {
    /// JavaScript injected into every child webview before page scripts run.
    pub js: Vec<String>,
    /// CSS injected into every child webview.
    pub css: Vec<String>,
}

impl Config {
    /// Adds a JavaScript snippet applied to every child webview.
    pub fn js(mut self, script: impl Into<String>) -> Self {
        self.js.push(script.into());
        self
    }

    /// Adds a stylesheet applied to every child webview.
    pub fn css(mut self, css: impl Into<String>) -> Self {
        self.css.push(css.into());
        self
    }
}

/// Initializes the plugin with the default configuration.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    init_with(Config::default())
}

/// Initializes the plugin with a custom configuration.
pub fn init_with<R: Runtime>(config: Config) -> TauriPlugin<R> {
    PluginBuilder::new("cef-webview")
        .initialization_script(HOST_RUNTIME)
        .invoke_handler(tauri::generate_handler![
            commands::create,
            commands::set_rect,
            commands::set_visible,
            commands::destroy,
            commands::eval,
            commands::insert_css,
            commands::remove_inserted_css,
            commands::navigate,
            commands::reload,
            commands::open_devtools,
            commands::call,
        ])
        .setup(move |app, _api| {
            app.manage(Registry::<R>::new(config));
            Ok(())
        })
        .build()
}

/// Access the plugin registry from a Tauri app handle.
pub trait CefWebviewExt<R: Runtime> {
    /// Returns the plugin registry, which exposes [`Registry::get`].
    fn cef_webviews(&self) -> tauri::State<'_, Registry<R>>;
}

impl<R: Runtime, M: Manager<R>> CefWebviewExt<R> for M {
    fn cef_webviews(&self) -> tauri::State<'_, Registry<R>> {
        self.state::<Registry<R>>()
    }
}

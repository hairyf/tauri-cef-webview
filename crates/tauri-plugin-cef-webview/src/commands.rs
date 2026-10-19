use tauri::{
    webview::{WebviewBuilder, Webview},
    LogicalPosition, LogicalSize, Runtime, State, WebviewUrl,
};

use crate::{
    registry::{CefWebview, Registry},
    scripts,
};

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn handle<R: Runtime>(registry: &Registry<R>, label: &str) -> Result<CefWebview<R>, String> {
    registry
        .get(label)
        .ok_or_else(|| format!("cef webview `{label}` not found"))
}

/// Creates (or re-attaches) a child webview inside the caller's window.
#[tauri::command]
pub fn create<R: Runtime>(
    caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    js: Option<Vec<String>>,
    css: Option<Vec<String>>,
) -> Result<(), String> {
    let window = caller.window();

    if let Some(existing) = window
        .webviews()
        .into_iter()
        .find(|webview| webview.label() == label)
    {
        existing
            .set_bounds(tauri::Rect {
                position: LogicalPosition::new(x, y).into(),
                size: LogicalSize::new(width, height).into(),
            })
            .map_err(error)?;
        registry.insert(&label, existing);
        return Ok(());
    }

    let url = url
        .parse()
        .map_err(|cause| format!("invalid url `{url}`: {cause}"))?;

    let mut builder = WebviewBuilder::new(label.clone(), WebviewUrl::External(url))
        .position(LogicalPosition::new(x, y))
        .size(LogicalSize::new(width, height));

    for script in registry.config().js.iter() {
        builder = builder.initialization_script(script.clone());
    }
    for script in js.into_iter().flatten() {
        builder = builder.initialization_script(script);
    }
    for (index, css) in registry.config().css.iter().enumerate() {
        builder = builder.initialization_script(scripts::insert_style(
            &format!("cwv-init-{index}"),
            css,
        ));
    }
    for (index, css) in css.into_iter().flatten().enumerate() {
        builder =
            builder.initialization_script(scripts::insert_style(&format!("cwv-element-{index}"), &css));
    }

    let webview = window.add_child(builder).map_err(error)?;
    registry.insert(&label, webview);

    Ok(())
}

/// Moves and resizes a child webview. Values are logical pixels.
#[tauri::command]
pub fn set_rect<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    handle(&registry, &label)?
        .set_rect(x, y, width, height)
        .map_err(error)
}

/// Shows or hides a child webview.
#[tauri::command]
pub fn set_visible<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    visible: bool,
) -> Result<(), String> {
    handle(&registry, &label)?
        .set_visible(visible)
        .map_err(error)
}

/// Closes and unregisters a child webview.
#[tauri::command]
pub fn destroy<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
) -> Result<(), String> {
    if let Some(webview) = registry.remove(&label) {
        webview.close().map_err(error)?;
    }
    Ok(())
}

/// Evaluates JavaScript in a child webview.
#[tauri::command]
pub fn eval<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    js: String,
) -> Result<(), String> {
    handle(&registry, &label)?.eval(js).map_err(error)
}

/// Injects a stylesheet into a child webview and returns its key.
#[tauri::command]
pub fn insert_css<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    css: String,
) -> Result<String, String> {
    handle(&registry, &label)?
        .insert_css(&css)
        .map_err(error)
}

/// Removes a stylesheet previously inserted with [`insert_css`].
#[tauri::command]
pub fn remove_inserted_css<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    key: String,
) -> Result<(), String> {
    handle(&registry, &label)?
        .remove_inserted_css(&key)
        .map_err(error)
}

/// Navigates a child webview to `url`.
#[tauri::command]
pub fn navigate<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    url: String,
) -> Result<(), String> {
    handle(&registry, &label)?.navigate(&url)
}

/// Reloads a child webview.
#[tauri::command]
pub fn reload<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
) -> Result<(), String> {
    handle(&registry, &label)?.reload().map_err(error)
}

/// Opens the devtools of a child webview.
#[tauri::command]
pub fn open_devtools<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
) -> Result<(), String> {
    handle(&registry, &label)?.open_devtools();
    Ok(())
}

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{
    webview::{NewWindowResponse, PageLoadEvent, Webview, WebviewBuilder},
    Emitter, LogicalPosition, LogicalSize, Runtime, State, Url, WebviewUrl,
};

use crate::{
    registry::{CefWebview, Registry, SharedState},
    scripts,
};

const PLUGIN_EVENT: &str = "cef-webview://event";
const GUEST_RUNTIME: &str = include_str!("guest.js.inc");

/// Options accepted by the `create` command.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOptions {
    pub user_agent: Option<String>,
    pub preload: Option<String>,
    pub allow_popups: Option<bool>,
}

#[derive(Clone, serde::Serialize)]
struct PluginEvent {
    label: String,
    event: String,
    args: Value,
}

#[derive(Deserialize)]
struct GuestMessage {
    n: String,
    a: Value,
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn handle<R: Runtime>(registry: &Registry<R>, label: &str) -> Result<CefWebview<R>, String> {
    registry
        .get(label)
        .ok_or_else(|| format!("cef webview `{label}` not found"))
}

fn emit_event<R: Runtime>(webview: &Webview<R>, label: &str, event: &str, args: Value) {
    let _ = webview.emit(
        PLUGIN_EVENT,
        PluginEvent {
            label: label.to_string(),
            event: event.to_string(),
            args,
        },
    );
}

fn snapshot(state: &SharedState) -> Value {
    let state = state.snapshot();
    json!({
        "url": state.url,
        "title": state.title,
        "loading": state.loading,
        "waitingForResponse": state.waiting_for_response,
        "canGoBack": state.can_go_back,
        "canGoForward": state.can_go_forward,
        "historyIndex": state.history_index,
        "crashed": state.crashed,
        "audioMuted": state.audio_muted,
        "audible": state.audible,
        "userAgent": state.user_agent,
        "zoomFactor": state.zoom_factor,
        "zoomLevel": state.zoom_level,
        "webContentsId": state.web_contents_id,
        "offscreen": state.offscreen,
    })
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).expect("strings are always JSON serializable")
}

fn guest<R: Runtime>(webview: &CefWebview<R>, expression: String) -> Result<(), String> {
    webview
        .eval(format!(
            "try {{ window.__cefWebview && ({expression}) }} catch (error) {{ console.error(error) }}"
        ))
        .map_err(error)
}

fn arg<'a>(args: &'a Value, key: &str) -> Result<&'a Value, String> {
    args.get(key)
        .ok_or_else(|| format!("missing `{key}` argument"))
}

fn string_arg(args: &Value, key: &str) -> Result<String, String> {
    arg(args, key)?
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| format!("`{key}` must be a string"))
}

fn number_arg(args: &Value, key: &str) -> Result<f64, String> {
    arg(args, key)?
        .as_f64()
        .ok_or_else(|| format!("`{key}` must be a number"))
}

fn integer_arg(args: &Value, key: &str) -> Result<i64, String> {
    arg(args, key)?
        .as_i64()
        .ok_or_else(|| format!("`{key}` must be an integer"))
}

fn boolean_arg(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// Attaches the off-screen renderer when the plugin asks for it.
#[cfg(feature = "offscreen")]
fn set_frame_rate<R: Runtime>(webview: &Webview<R>, offscreen: bool) {
    if offscreen {
        crate::offscreen::set_frame_rate(webview);
    }
}

#[cfg(not(feature = "offscreen"))]
fn set_frame_rate<R: Runtime>(_webview: &Webview<R>, _offscreen: bool) {}

#[cfg(feature = "offscreen")]
fn attach_offscreen<R: Runtime>(
    registry: &Registry<R>,
    label: &str,
    builder: &mut WebviewBuilder<R>,
) -> bool {
    crate::offscreen::attach(builder, registry.frames(label))
}

/// Off-screen rendering is compiled out.
#[cfg(not(feature = "offscreen"))]
fn attach_offscreen<R: Runtime>(
    _registry: &Registry<R>,
    _label: &str,
    _builder: &mut WebviewBuilder<R>,
) -> bool {
    false
}

#[cfg(feature = "offscreen")]
fn forward_input<R: Runtime>(webview: &CefWebview<R>, event: Value) -> Result<Value, String> {
    crate::offscreen::send_input(webview.inner(), event);
    Ok(Value::Null)
}

#[cfg(not(feature = "offscreen"))]
fn forward_input<R: Runtime>(_webview: &CefWebview<R>, _event: Value) -> Result<Value, String> {
    Err("sendInputEvent requires the `offscreen` feature".to_string())
}

fn read_preload(preload: &str) -> Result<String, String> {
    let path = match Url::parse(preload) {
        Ok(url) if url.scheme() == "file" => url
            .to_file_path()
            .map_err(|_| format!("invalid preload path `{preload}`"))?,
        Ok(_) => return Err(format!("preload must be a file: URL, got `{preload}`")),
        Err(_) => std::path::PathBuf::from(preload),
    };
    std::fs::read_to_string(&path)
        .map_err(|cause| format!("failed to read `{}`: {cause}", path.display()))
}

/// Forwards the events batched by the guest runtime and cancels the bridge navigation.
fn forward_guest_events<R: Runtime>(
    webview: &Webview<R>,
    label: &str,
    state: &SharedState,
    url: &Url,
) {
    let Some(data) = url
        .query_pairs()
        .find(|(key, _)| key.as_ref() == "d")
        .map(|(_, value)| value.into_owned())
    else {
        return;
    };

    let Ok(messages) = serde_json::from_str::<Vec<GuestMessage>>(&data) else {
        return;
    };

    for message in messages {
        if message.n == "state" {
            state.update(|state| {
                if let Some(user_agent) = message.a.get("userAgent").and_then(Value::as_str) {
                    state.user_agent = user_agent.to_string();
                }
                if let Some(audible) = message.a.get("audible").and_then(Value::as_bool) {
                    state.audible = audible;
                }
            });
        }
        emit_event(webview, label, &message.n, message.a);
    }
}

/// Creates (or re-attaches) a child webview inside the caller's window.
#[tauri::command]
pub async fn create<R: Runtime>(
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
    options: Option<CreateOptions>,
) -> Result<Value, String> {
    let window = caller.window();
    // The host runtime is injected into child webviews too, so a page loaded
    // inside one could otherwise ask for another level of nesting.
    if registry.is_child(caller.label()) {
        return Err("cef-webview: cannot create a webview from a child webview".into());
    }
    let CreateOptions {
        user_agent,
        preload,
        allow_popups,
    } = options.unwrap_or_default();
    let state = registry.state(&label);

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
        state.update(|state| {
            if state.url.is_empty() {
                state.url = url;
            }
            if state.web_contents_id == 0 {
                state.web_contents_id = registry.next_contents_id();
            }
        });
        emit_event(&caller, &label, "did-attach", json!({}));
        emit_event(&caller, &label, "state", snapshot(&state));
        return Ok(snapshot(&state));
    }

    let contents_id = registry.next_contents_id();
    state.update(|state| {
        state.url = url.clone();
        state.web_contents_id = contents_id;
        if let Some(user_agent) = &user_agent {
            state.user_agent = user_agent.clone();
        }
    });

    let parsed = url
        .parse::<Url>()
        .map_err(|cause| format!("invalid url `{url}`: {cause}"))?;

    if registry.config().offscreen && !cfg!(feature = "offscreen") {
        return Err(
            "cef-webview: the `offscreen` config requires the `offscreen` feature".to_string(),
        );
    }

    let mut builder = WebviewBuilder::new(label.clone(), WebviewUrl::External(parsed))
        .position(LogicalPosition::new(x, y))
        .size(LogicalSize::new(width, height))
        .initialization_script(GUEST_RUNTIME);

    if let Some(user_agent) = &user_agent {
        builder = builder.user_agent(user_agent);
    }

    if let Some(preload) = &preload {
        builder = builder.initialization_script(read_preload(preload)?);
    }

    for script in registry.config().js.iter() {
        builder = builder.initialization_script(script.clone());
    }
    for script in js.into_iter().flatten() {
        builder = builder.initialization_script(script);
    }
    for (index, css) in registry.config().css.iter().enumerate() {
        builder =
            builder.initialization_script(scripts::insert_style(&format!("cwv-init-{index}"), css));
    }
    for (index, css) in css.into_iter().flatten().enumerate() {
        builder = builder
            .initialization_script(scripts::insert_style(&format!("cwv-element-{index}"), &css));
    }

    let offscreen = if registry.config().offscreen {
        let offscreen = attach_offscreen(&registry, &label, &mut builder);
        state.update(|state| state.offscreen = offscreen);
        offscreen
    } else {
        false
    };

    let allow_popups = allow_popups.unwrap_or(false);
    builder = builder.on_new_window(move |_url, _features| {
        if allow_popups {
            NewWindowResponse::Allow
        } else {
            NewWindowResponse::Deny
        }
    });

    let nav_state = state.clone();
    let nav_label = label.clone();
    let nav_emitter = caller.clone();
    builder = builder.on_navigation(move |url| {
        if url.scheme() == "cef-webview" && url.host_str() == Some("event") {
            forward_guest_events(&nav_emitter, &nav_label, &nav_state, url);
            return false;
        }

        let href = url.to_string();
        nav_state.update(|state| {
            state.url = href.clone();
            state.waiting_for_response = true;
        });
        emit_event(
            &nav_emitter,
            &nav_label,
            "will-navigate",
            json!({ "url": href }),
        );
        emit_event(
            &nav_emitter,
            &nav_label,
            "will-frame-navigate",
            json!({
                "url": href,
                "isMainFrame": true,
                "frameProcessId": 0,
                "frameRoutingId": 0,
            }),
        );
        true
    });

    let title_state = state.clone();
    let title_label = label.clone();
    builder = builder.on_document_title_changed(move |webview, title| {
        title_state.update(|state| {
            state.title = title.clone();
        });
        emit_event(
            &webview,
            &title_label,
            "page-title-updated",
            json!({ "title": title, "explicitSet": true }),
        );
    });

    let load_state = state.clone();
    let load_label = label.clone();
    builder = builder.on_page_load(move |webview, payload| {
        // Bridge navigations are cancelled in `on_navigation`, but CEF still
        // reports a page load for them; they must not reach the page state. The
        // runtime also finishes its own placeholder document before the real URL.
        let href = payload.url().as_str();
        if href.starts_with("cef-webview:") || href.contains("data-tauri-cef-internal") {
            return;
        }
        match payload.event() {
            PageLoadEvent::Started => {
                let url = payload.url().to_string();
                load_state.update(|state| {
                    state.loading = true;
                    state.waiting_for_response = true;
                    state.url = url.clone();
                });
                emit_event(
                    &webview,
                    &load_label,
                    "load-commit",
                    json!({
                        "url": url,
                        "isMainFrame": true,
                    }),
                );
                emit_event(&webview, &load_label, "did-start-loading", json!({}));
                emit_event(
                    &webview,
                    &load_label,
                    "did-start-navigation",
                    json!({
                        "url": url,
                        "isInPlace": false,
                        "isMainFrame": true,
                        "frameProcessId": 0,
                        "frameRoutingId": 0,
                    }),
                );
                emit_event(&webview, &load_label, "state", snapshot(&load_state));
            }
            PageLoadEvent::Finished => {
                let url = payload.url().to_string();
                load_state.update(|state| {
                    if state.last_finished_url != url {
                        state.history_index += 1;
                        state.last_finished_url = url.clone();
                    }
                    state.loading = false;
                    state.waiting_for_response = false;
                    state.url = url.clone();
                });
                emit_event(
                    &webview,
                    &load_label,
                    "did-frame-finish-load",
                    json!({ "isMainFrame": true }),
                );
                emit_event(&webview, &load_label, "did-finish-load", json!({}));
                emit_event(&webview, &load_label, "did-stop-loading", json!({}));
                emit_event(
                    &webview,
                    &load_label,
                    "did-frame-navigate",
                    json!({
                        "url": url,
                        "httpResponseCode": 0,
                        "httpStatusText": "",
                        "isMainFrame": true,
                        "frameProcessId": 0,
                        "frameRoutingId": 0,
                    }),
                );
                emit_event(&webview, &load_label, "did-navigate", json!({ "url": url }));
                emit_event(&webview, &load_label, "state", snapshot(&load_state));

                // `can_go_back`/`can_go_forward` are synchronous round-trips to the
                // runtime, and this handler runs on the CEF UI thread, so calling them
                // here deadlocks the whole app. Ask from a worker thread instead.
                let refresh = webview.clone();
                let refresh_state = load_state.clone();
                let refresh_label = load_label.clone();
                tauri::async_runtime::spawn(async move {
                    let can_go_back = refresh.can_go_back().unwrap_or(false);
                    let can_go_forward = refresh.can_go_forward().unwrap_or(false);
                    refresh_state.update(|state| {
                        state.can_go_back = can_go_back;
                        state.can_go_forward = can_go_forward;
                    });
                    emit_event(&refresh, &refresh_label, "state", snapshot(&refresh_state));
                });
            }
        }
    });

    registry.mark_child(&label);
    let added = window.add_child(builder);
    let webview = match added {
        Ok(webview) => webview,
        Err(cause) => {
            registry.unmark_child(&label);
            return Err(error(cause));
        }
    };
    set_frame_rate(&webview, offscreen);
    registry.insert(&label, webview);
    registry.unmark_child(&label);

    emit_event(&caller, &label, "did-attach", json!({}));
    emit_event(&caller, &label, "state", snapshot(&state));

    Ok(snapshot(&state))
}

/// Moves and resizes a child webview. Values are logical pixels.
#[tauri::command]
pub async fn set_rect<R: Runtime>(
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
pub async fn set_visible<R: Runtime>(
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
pub async fn destroy<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
) -> Result<(), String> {
    if let Some(webview) = registry.remove(&label) {
        webview.close().map_err(error)?;
    }
    registry.remove_state(&label);
    registry.remove_frames(&label);
    Ok(())
}

/// Returns the rectangles painted since the previous poll, empty when idle.
#[tauri::command]
pub async fn frame<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    full: Option<bool>,
) -> Result<tauri::ipc::Response, String> {
    Ok(tauri::ipc::Response::new(
        registry.frame_packet(&label, full.unwrap_or(false)),
    ))
}

/// Evaluates JavaScript in a child webview.
#[tauri::command]
pub async fn eval<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    js: String,
) -> Result<(), String> {
    handle(&registry, &label)?.eval(js).map_err(error)
}

/// Injects a stylesheet into a child webview and returns its key.
#[tauri::command]
pub async fn insert_css<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    css: String,
) -> Result<String, String> {
    handle(&registry, &label)?.insert_css(&css).map_err(error)
}

/// Removes a stylesheet previously inserted with [`insert_css`].
#[tauri::command]
pub async fn remove_inserted_css<R: Runtime>(
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
pub async fn navigate<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    url: String,
) -> Result<(), String> {
    handle(&registry, &label)?.navigate(&url)
}

/// Reloads a child webview.
#[tauri::command]
pub async fn reload<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
) -> Result<(), String> {
    handle(&registry, &label)?.reload().map_err(error)
}

/// Opens the devtools of a child webview.
#[tauri::command]
pub async fn open_devtools<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
) -> Result<(), String> {
    handle(&registry, &label)?.open_devtools();
    Ok(())
}

/// Runs one method of the Electron `<webview>` API against a child webview.
#[tauri::command]
pub async fn call<R: Runtime>(
    _caller: Webview<R>,
    registry: State<'_, Registry<R>>,
    label: String,
    method: String,
    args: Value,
) -> Result<Value, String> {
    let webview = handle(&registry, &label)?;
    let state = registry.state(&label);

    match method.as_str() {
        "loadURL" | "navigate" => {
            let url = string_arg(&args, "url")?;
            if args
                .get("options")
                .and_then(|options| options.get("postData"))
                .is_some()
            {
                return Err("loadURL: `postData` is not supported".to_string());
            }
            webview.navigate(&url)?;
            state.update(|state| {
                state.url = url;
                state.waiting_for_response = true;
            });
            Ok(Value::Null)
        }
        "downloadURL" => {
            let url = string_arg(&args, "url")?;
            guest(
                &webview,
                format!("window.__cefWebview.download({})", js_string(&url)),
            )?;
            Ok(Value::Null)
        }
        "stop" => {
            guest(&webview, "window.__cefWebview.stop()".to_string())?;
            Ok(Value::Null)
        }
        "reload" => {
            webview.inner().reload().map_err(error)?;
            Ok(Value::Null)
        }
        "reloadIgnoringCache" => {
            guest(
                &webview,
                "window.__cefWebview.reloadIgnoringCache()".to_string(),
            )?;
            Ok(Value::Null)
        }
        "clearHistory" => {
            guest(&webview, "window.__cefWebview.clearHistory()".to_string())?;
            Ok(Value::Null)
        }
        "goBack" => {
            webview.inner().go_back().map_err(error)?;
            Ok(Value::Null)
        }
        "goForward" => {
            webview.inner().go_forward().map_err(error)?;
            Ok(Value::Null)
        }
        "goToOffset" => {
            let offset = integer_arg(&args, "offset")?;
            guest(&webview, format!("window.__cefWebview.go({offset})"))?;
            Ok(Value::Null)
        }
        "goToIndex" => {
            let index = integer_arg(&args, "index")?;
            let current = i64::from(state.snapshot().history_index);
            guest(
                &webview,
                format!("window.__cefWebview.go({})", index - current),
            )?;
            Ok(Value::Null)
        }
        "setUserAgent" => Err(
            "setUserAgent is not supported after creation; set the `useragent` attribute instead"
                .to_string(),
        ),
        "insertCSS" => {
            let css = string_arg(&args, "css")?;
            Ok(json!(webview.insert_css(&css).map_err(error)?))
        }
        "removeInsertedCSS" => {
            let key = string_arg(&args, "key")?;
            webview.remove_inserted_css(&key).map_err(error)?;
            Ok(Value::Null)
        }
        "executeJavaScript" => {
            let id = integer_arg(&args, "id")?;
            let code = string_arg(&args, "code")?;
            let gesture = boolean_arg(&args, "userGesture", false);
            guest(
                &webview,
                format!(
                    "window.__cefWebview.executeJavaScript({id}, {}, {gesture})",
                    js_string(&code)
                ),
            )?;
            Ok(Value::Null)
        }
        "openDevTools" => {
            webview.inner().open_devtools();
            Ok(Value::Null)
        }
        "closeDevTools" => {
            webview.inner().close_devtools();
            Ok(Value::Null)
        }
        "isDevToolsOpened" => Ok(json!(webview.inner().is_devtools_open())),
        "inspectElement" => {
            webview.inner().open_devtools();
            guest(&webview, "window.__cefWebview.inspect()".to_string())?;
            Ok(Value::Null)
        }
        "inspectSharedWorker" | "inspectServiceWorker" => Ok(Value::Null),
        "setAudioMuted" => {
            let muted = boolean_arg(&args, "muted", false);
            state.update(|state| {
                state.audio_muted = muted;
            });
            guest(&webview, format!("window.__cefWebview.mute({muted})"))?;
            Ok(Value::Null)
        }
        "edit" => {
            let command = string_arg(&args, "command")?;
            let value = args.get("value").cloned().unwrap_or(Value::Null);
            guest(
                &webview,
                format!("window.__cefWebview.edit({}, {value})", js_string(&command)),
            )?;
            Ok(Value::Null)
        }
        "findInPage" => {
            let request_id = integer_arg(&args, "requestId")?;
            let text = string_arg(&args, "text")?;
            let options = args.get("options").cloned().unwrap_or_else(|| json!({}));
            guest(
                &webview,
                format!(
                    "window.__cefWebview.findInPage({request_id}, {}, {options})",
                    js_string(&text)
                ),
            )?;
            Ok(json!(request_id))
        }
        "stopFindInPage" => {
            let action = string_arg(&args, "action")?;
            guest(
                &webview,
                format!("window.__cefWebview.stopFindInPage({})", js_string(&action)),
            )?;
            Ok(Value::Null)
        }
        "print" => {
            #[cfg(desktop)]
            webview.inner().print().map_err(error)?;
            #[cfg(not(desktop))]
            return Err("print is not supported on this platform".to_string());
            Ok(Value::Null)
        }
        "printToPDF" => Err("printToPDF is not supported by the CEF runtime".to_string()),
        "capturePage" => Err("capturePage is not supported by the CEF runtime".to_string()),
        "send" | "sendToFrame" => {
            let channel = string_arg(&args, "channel")?;
            let list = args.get("args").cloned().unwrap_or_else(|| json!([]));
            guest(
                &webview,
                format!(
                    "window.__cefWebview.deliver({}, {list})",
                    js_string(&channel)
                ),
            )?;
            Ok(Value::Null)
        }
        "sendInputEvent" => {
            let event = arg(&args, "event")?.clone();
            if state.snapshot().offscreen {
                forward_input(&webview, event)
            } else {
                guest(&webview, format!("window.__cefWebview.input({event})"))?;
                Ok(Value::Null)
            }
        }
        "setZoomFactor" => {
            let factor = number_arg(&args, "factor")?;
            webview.inner().set_zoom(factor).map_err(error)?;
            state.update(|state| {
                state.zoom_factor = factor;
                state.zoom_level = factor.ln() / 1.2_f64.ln();
            });
            Ok(Value::Null)
        }
        "setZoomLevel" => {
            let level = number_arg(&args, "level")?;
            let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(1.2);
            let factor = scale.powf(level);
            webview.inner().set_zoom(factor).map_err(error)?;
            state.update(|state| {
                state.zoom_factor = factor;
                state.zoom_level = level;
            });
            Ok(Value::Null)
        }
        "setVisualZoomLevelLimits" => {
            guest(
                &webview,
                "window.__cefWebview.visualZoomLimits()".to_string(),
            )?;
            Ok(Value::Null)
        }
        "showDefinitionForSelection" => Ok(Value::Null),
        "getURL" => Ok(json!(state.snapshot().url)),
        "getTitle" => Ok(json!(state.snapshot().title)),
        "isLoading" => Ok(json!(state.snapshot().loading)),
        "isLoadingMainFrame" => Ok(json!(state.snapshot().loading)),
        "isWaitingForResponse" => Ok(json!(state.snapshot().waiting_for_response)),
        "canGoBack" => Ok(json!(webview.inner().can_go_back().unwrap_or(false))),
        "canGoForward" => Ok(json!(webview.inner().can_go_forward().unwrap_or(false))),
        "canGoToOffset" => {
            let offset = integer_arg(&args, "offset")?;
            let snapshot = state.snapshot();
            Ok(json!(if offset < 0 {
                snapshot.can_go_back
            } else if offset > 0 {
                snapshot.can_go_forward
            } else {
                true
            }))
        }
        "isCrashed" => Ok(json!(state.snapshot().crashed)),
        "isDevToolsFocused" => Ok(json!(state.snapshot().devtools_focused)),
        "isAudioMuted" => Ok(json!(state.snapshot().audio_muted)),
        "isCurrentlyAudible" => Ok(json!(state.snapshot().audible)),
        "getUserAgent" => Ok(json!(state.snapshot().user_agent)),
        "getZoomFactor" => Ok(json!(state.snapshot().zoom_factor)),
        "getZoomLevel" => Ok(json!(state.snapshot().zoom_level)),
        "getWebContentsId" => Ok(json!(state.snapshot().web_contents_id)),
        other => Err(format!("unknown cef-webview method `{other}`")),
    }
}

# tauri-cef-webview

Embed native CEF child webviews in a Tauri v3 application and control them with a plain `<webview>` element.

```html
<webview id="news" src="https://example.com/"></webview>
```

The element keeps the place in the layout; the actual page is rendered by a native child webview drawn above it. Size and position follow `getBoundingClientRect()`.

No frontend import is required. The plugin registers the element API as a Tauri initialization script, so it exists before any page script runs, in every window, including windows created from `tauri.conf.json`.

> [!WARNING]
> Requires **Tauri v3 alpha** (`tauri 3.0.0-alpha.5`, `tauri-runtime-cef 3.0.0-alpha.6`, `tauri-plugin 3.0.0-alpha.4`). Alpha APIs change without notice. Building also needs CMake and Ninja.

## Install

```bash
cargo add tauri-plugin-cef-webview
```

Register the runtime and the plugin once:

```rust
pub fn run() {
  tauri::Builder::default()
    .runtime(tauri_runtime_cef::Cef::default())
    .plugin(tauri_plugin_cef_webview::init())
    .build(tauri::generate_context!())
    .expect("error while building tauri application")
    .run(|_, _| {});
}
```

Add the permission to your capability:

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": ["core:default", "cef-webview:default"]
}
```

TypeScript types are optional:

```bash
pnpm add -D tauri-cef-webview
```

## Attributes

| Attribute | Type | Description |
| --- | --- | --- |
| `src` | string | Page to load. Writing navigates; assigning the current value reloads. |
| `id` | string | Native webview label, so Rust can address it by the same name. Generated when omitted. |
| `js` | string | JavaScript run before page scripts on every navigation. |
| `css` | string | Stylesheet applied on every navigation. |
| `useragent` | string | User agent for the first navigation. |
| `preload` | string | `file:` URL evaluated before page scripts, without Node APIs. |
| `allowpopups` | boolean | Allows `window.open` and `target="_blank"` popups. |

## Methods

| Method | Description |
| --- | --- |
| `loadURL(url[, options])` | Navigates. Resolves when the navigation is dispatched. |
| `getURL()`, `getTitle()`, `isLoading()`, `isLoadingMainFrame()`, `isWaitingForResponse()` | Synchronous, backed by state pushed from Rust. |
| `reload()`, `stop()` | |
| `goBack()`, `goForward()`, `canGoBack()`, `canGoForward()` | |
| `executeJavaScript(code[, userGesture])` | Resolves with the value returned by the code; rejects when it throws or rejects. |
| `insertCSS(css)`, `removeInsertedCSS(key)` | |
| `openDevTools()`, `closeDevTools()`, `isDevToolsOpened()` | |
| `send(channel, ...args)` | Delivers to the guest `ipcRenderer` shim injected by the plugin. |
| `sendInputEvent(event)` | Dispatches a synthetic mouse, wheel or keyboard event in the guest. |
| `findInPage(text[, options])`, `stopFindInPage(action)` | Guest-side search; emits `found-in-page`. |
| `setZoomFactor(factor)`, `setZoomLevel(level[, scale])`, `getZoomFactor()`, `getZoomLevel()` | |
| `print([options])` | Native print dialog. |

## Events

All events are dispatched on the `<webview>` element, with payload fields copied onto the event object.

| Event | Payload |
| --- | --- |
| `did-attach` | |
| `did-start-loading`, `did-stop-loading`, `did-finish-load` | |
| `load-commit` | `url`, `isMainFrame` |
| `dom-ready` | |
| `page-title-updated` | `title`, `explicitSet` |
| `will-navigate` | `url` |
| `did-navigate` | `url` |
| `did-navigate-in-page` | `isMainFrame`, `url` |
| `console-message` | `level`, `message`, `line`, `sourceId` |
| `context-menu` | `params` |
| `ipc-message` | `frameId`, `channel`, `args` |
| `destroyed` | |

## Documentation

Full attribute, method, event, option and Rust API reference: [docs/api.md](docs/api.md).

## Limitations

* The native webview is drawn above the page. CSS `transform`, `opacity`, `border-radius`, `overflow: hidden` clipping and `z-index` stacking do not affect it.
* The `<webview>` tag name has no hyphen, so it cannot be registered with `customElements.define`. The plugin enhances the element in place.
* Only the CEF runtime (`tauri-runtime-cef`) is supported.

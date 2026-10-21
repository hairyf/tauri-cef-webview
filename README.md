# tauri-cef-webview

Embed native CEF child webviews in a Tauri v3 application and control them with a plain `<webview>` element.

```html
<webview id="news" src="https://example.com/"></webview>
```

The element keeps the place in the layout; the actual page is rendered by a native child webview drawn above it. Size and position are kept in sync with `getBoundingClientRect()`.

The element exposes the same attributes, methods and DOM events as Electron's [`<webview>` tag](https://github.com/electron/electron/blob/main/docs/api/webview-tag.md). No frontend import is required: the plugin injects the element API into every application page.

> [!WARNING]
> Requires **Tauri v3 alpha** (`tauri 3.0.0-alpha.5`, `tauri-runtime-cef 3.0.0-alpha.6`, `tauri-plugin 3.0.0-alpha.4`). The plugin is built against alpha APIs, which change without notice. Pin your Tauri versions together with this package and expect breakage between alpha releases. Building also needs CMake and Ninja.

## Install

```bash
cargo add tauri-plugin-cef-webview
```

Add the runtime and register the plugin once. That is all the Rust code required.

```rust
pub fn run() {
  let builder = tauri::Builder::default()
    .runtime(tauri_runtime_cef::Cef::default());

  builder
    .plugin(tauri_plugin_cef_webview::init())
    .build(tauri::generate_context!())
    .expect("error while building tauri application")
    .run(|_, _| {});
}
```

Inject scripts or styles into every embedded webview:

```rust
tauri_plugin_cef_webview::init_with(
  tauri_plugin_cef_webview::Config::default()
    .js("document.documentElement.dataset.embedded = 'true'")
    .css("body { font-family: system-ui; }"),
)
```

Add the permission to your capability:

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": ["core:default", "cef-webview:default"]
}
```

Optionally install the frontend package for TypeScript types:

```bash
pnpm add -D tauri-cef-webview
```

## Attributes

Read once, when the element is mounted.

| Attribute | Type | Support |
| --- | --- | --- |
| `src` | string | **Supported.** Writing navigates; assigning the same value reloads. |
| `useragent` | string | **Supported.** Used for the first navigation; see `setUserAgent` for later changes. |
| `preload` | string | **Supported.** Must be a `file:` URL. Runs before page scripts, without Node APIs. |
| `allowpopups` | boolean | **Supported.** Allows `window.open` and `target=_blank` popups. |
| `nodeintegration` | boolean | Accepted and ignored. CEF has no Node.js integration. |
| `nodeintegrationinsubframes` | boolean | Accepted and ignored. |
| `plugins` | boolean | Accepted and ignored. |
| `httpreferrer` | string | Accepted and ignored. |
| `disablewebsecurity` | boolean | Accepted and ignored. Immutable after attach. |
| `partition` | string | Accepted and ignored. Immutable after attach. |
| `webpreferences` | string | Accepted and ignored. |
| `enableblinkfeatures` | string | Accepted and ignored. |
| `disableblinkfeatures` | string | Accepted and ignored. |
| `id` | string | Becomes the native webview label, so Rust can address it by the same name. Generated when omitted. |
| `js` | string | JavaScript evaluated before page scripts on every navigation. |
| `css` | string | Stylesheet applied on every navigation. |

Unsupported attributes are read and ignored with a console warning. `partition` and `disablewebsecurity` throw a `DOMException` when changed after attach, matching Electron.

## Methods

| Method | Notes |
| --- | --- |
| `loadURL(url[, options])` | `postData` is not supported. Resolves when the navigation is dispatched. |
| `downloadURL(url[, options])` | Downloads without navigating. |
| `getURL()`, `getTitle()`, `isLoading()`, `isLoadingMainFrame()`, `isWaitingForResponse()` | Synchronous, backed by state pushed from Rust. |
| `stop()`, `reload()`, `reloadIgnoringCache()` | |
| `canGoBack()`, `canGoForward()`, `canGoToOffset(offset)` | |
| `clearHistory()`, `goBack()`, `goForward()`, `goToIndex(index)`, `goToOffset(offset)` | History navigation runs in the guest. `clearHistory` is best-effort. |
| `isCrashed()` | Always `false` until a crash event is available. |
| `setUserAgent(userAgent)` | Not supported after creation; set the `useragent` attribute before mounting. |
| `getUserAgent()` | |
| `insertCSS(css)`, `removeInsertedCSS(key)` | |
| `executeJavaScript(code[, userGesture])` | Resolves with the value returned by the code; rejects when the code throws or rejects. |
| `openDevTools()`, `closeDevTools()`, `isDevToolsOpened()` | |
| `isDevToolsFocused()` | Always `false`. |
| `inspectElement(x, y)`, `inspectSharedWorker()`, `inspectServiceWorker()` | Opens devtools; the inspected target is best-effort. |
| `setAudioMuted(muted)`, `isAudioMuted()`, `isCurrentlyAudible()` | Muting applies to media elements in the guest. |
| Editing: `undo()`, `redo()`, `cut()`, `copy()`, `centerSelection()`, `paste()`, `pasteAndMatchStyle()`, `delete()`, `selectAll()`, `unselect()`, `scrollToTop()`, `scrollToBottom()`, `adjustSelection(options)`, `replace(text)`, `replaceMisspelling(text)`, `insertText(text)` | Runs in the guest document. |
| `findInPage(text[, options])`, `stopFindInPage(action)` | Guest-side search; emits `found-in-page`. |
| `print([options])` | Uses the native print dialog. |
| `printToPDF(options)`, `capturePage([rect])` | Rejects: the CEF runtime does not expose a screenshot/PDF seam. |
| `send(channel, ...args)`, `sendToFrame(frameId, channel, ...args)` | Delivers to the guest `ipcRenderer` shim injected by the plugin. |
| `sendInputEvent(event)` | Dispatches a synthetic mouse, wheel or keyboard event in the guest. |
| `setZoomFactor(factor)`, `setZoomLevel(level[, scale])`, `getZoomFactor()`, `getZoomLevel()` | |
| `setVisualZoomLevelLimits(min, max)` | No-op; resolves. |
| `showDefinitionForSelection()` | macOS only in Electron; no-op here. |
| `getWebContentsId()` | |

Compatibility aliases kept from earlier versions: `navigate()` (same as `loadURL`), `eval()` (same as `executeJavaScript`), and `openDevtools()` (same as `openDevTools`). `ready` resolves once the native webview exists; `label` is the native label.

## Events

All events are dispatched on the `<webview>` element with Electron's payload fields copied onto the event object.

| Event | Payload | Source |
| --- | --- | --- |
| `load-commit` | `url`, `isMainFrame` | native |
| `did-finish-load`, `did-start-loading`, `did-stop-loading` | | native |
| `did-frame-finish-load` | `isMainFrame` | native |
| `did-start-navigation` | `url`, `isInPlace`, `isMainFrame`, `frameProcessId`, `frameRoutingId` | native |
| `did-frame-navigate` | `url`, `httpResponseCode`, `httpStatusText`, `isMainFrame`, `frameProcessId`, `frameRoutingId` | native |
| `did-navigate` | `url` | native |
| `will-navigate`, `will-frame-navigate` | `url`, frame ids | native; cannot be prevented, like Electron |
| `page-title-updated` | `title`, `explicitSet` | native |
| `did-attach` | | native |
| `dom-ready`, `page-favicon-updated`, `did-change-theme-color`, `update-target-url` | | guest |
| `console-message` | `level`, `message`, `line`, `sourceId` | guest |
| `context-menu` | `params` | guest |
| `enter-html-full-screen`, `leave-html-full-screen` | | guest |
| `media-started-playing`, `media-paused` | | guest |
| `did-navigate-in-page` | `isMainFrame`, `url` | guest |
| `ipc-message` | `frameId`, `channel`, `args` | guest (from `ipcRenderer.sendToHost`) |
| `found-in-page` | `result` | guest |
| `close` | | guest (`window.close`) |
| `destroyed` | | native |

Declared but not emitted yet, because the runtime exposes no seam for them: `did-fail-load`, `did-redirect-navigation`, `render-process-gone`, `devtools-open-url`, `devtools-search-query`, `devtools-opened`, `devtools-closed`, `devtools-focused`.

Guest events are batched into a `cef-webview://event/` custom-scheme navigation every 16ms. The plugin cancels that navigation and forwards the events to the application page. The bridge is main-frame only.

## Rust API

```rust
use tauri_plugin_cef_webview::CefWebviewExt;

#[tauri::command]
fn highlight(app: tauri::AppHandle) -> tauri::Result<()> {
  if let Some(webview) = app.cef_webviews().get("news") {
    webview.insert_css("body { outline: 2px solid red }")?;
    webview.eval("console.log('hello from Rust')")?;
  }
  Ok(())
}
```

`Registry::get` returns a handle with `eval`, `insert_css`, `remove_inserted_css`, `navigate`, `reload`, `open_devtools`, `set_rect`, `set_visible` and `close`.

## How it works

* The plugin injects `element.js` into every application page. It enhances `<webview>` elements, including elements added later, and talks to the plugin over Tauri IPC.
* On mount the element calls the `create` command, passing its `getBoundingClientRect()`.
* A shared `requestAnimationFrame` loop pushes position changes (scroll, resize, layout) to the native layer.
* `ResizeObserver` triggers the same loop; `IntersectionObserver` hides the native webview when the element leaves the viewport or collapses to zero size.
* Removing the element from the DOM closes the native webview.
* The plugin injects `guest.js` into every child webview. It reports the events the runtime does not surface and provides an `ipcRenderer` shim for `send`/`ipc-message`.
* JS/CSS from `Config`, `js` and `css` attributes are registered as CEF initialization scripts, so they apply on every navigation without flashing.

## Limitations

* The native webview is drawn above the page. CSS `transform`, `opacity`, `border-radius`, `overflow: hidden` clipping and `z-index` stacking do not affect it. An element that must be covered cannot be placed over the webview.
* Position is synced on a best-effort basis; scrolling may show a few frames of lag.
* The `<webview>` tag name has no hyphen, so it cannot be registered with `customElements.define`. The plugin enhances the element in place instead: the DOM node stays `<webview>` and is not an instance of a custom element class.
* Only the CEF runtime (`tauri-runtime-cef`) is supported.
* Sandboxing, `partition`, Node.js integration and Blink/Chromium preferences are process-wide in CEF and cannot be configured per element.

# API reference

The `<webview>` element, its attributes, methods, DOM events and the Rust side of `tauri-plugin-cef-webview`.

- [Element lifecycle](#element-lifecycle)
- [Attributes](#attributes)
- [Methods](#methods)
- [Events](#events)
- [Rust API](#rust-api)
- [How it works](#how-it-works)
- [Limitations](#limitations)

## Element lifecycle

```html
<webview id="news" src="https://example.com/"></webview>
```

The element occupies its normal place in the document layout. The page itself is rendered by a native CEF child webview painted above the application page, positioned from the element's `getBoundingClientRect()`.

* The element API is installed at document start in every window by the plugin's initialization script. Elements are enhanced when parsed, when added to the DOM, and when created with `document.createElement('webview')`.
* Mounting calls the Rust `create` command with the label, URL and rect.
* A shared `requestAnimationFrame` loop pushes position changes caused by scroll, resize or layout.
* `ResizeObserver` re-runs the loop; `IntersectionObserver` hides the native webview when the element is off-screen or has zero size.
* Removing the element from the DOM closes the native webview and fires `destroyed`.

`label` is the native label (`id`, or a generated `cef-webview-<n>`), and `ready` resolves once the native webview exists.

Compatibility aliases: `navigate()` (`loadURL`), `eval()` (`executeJavaScript`), `openDevtools()` (`openDevTools`).

## Attributes

Read once, when the element is mounted. Unsupported attributes are read and ignored with a console warning.

| Attribute | Type | Behavior |
| --- | --- | --- |
| `src` | string | Navigates on write. Assigning the current value reloads. `data:` URLs are allowed. |
| `id` | string | Native webview label. Generated when omitted. |
| `js` | string | JavaScript evaluated before page scripts on every navigation. |
| `css` | string | Stylesheet applied on every navigation. |
| `useragent` | string | User agent for the first navigation. Use `setUserAgent` afterwards. |
| `preload` | string | Must be a `file:` URL. Evaluated before page scripts, without Node APIs. |
| `allowpopups` | boolean | Allows `window.open` and `target="_blank"`; the popup becomes its own native webview. |
| `nodeintegration` | boolean | Accepted and ignored. CEF has no Node.js integration. |
| `nodeintegrationinsubframes` | boolean | Accepted and ignored. |
| `plugins` | boolean | Accepted and ignored. |
| `httpreferrer` | string | Accepted and ignored. |
| `disablewebsecurity` | boolean | Accepted and ignored. Immutable after attach. |
| `partition` | string | Accepted and ignored. Immutable after attach. |
| `webpreferences` | string | Accepted and ignored. |
| `enableblinkfeatures` | string | Accepted and ignored. |
| `disableblinkfeatures` | string | Accepted and ignored. |

`partition` and `disablewebsecurity` throw a `DOMException` when changed after attach.

## Methods

### Navigation

| Method | Returns | Behavior |
| --- | --- | --- |
| `loadURL(url[, options])` | `Promise<void>` | Navigates. Resolves when the navigation is dispatched. |
| `downloadURL(url[, options])` | `void` | Downloads without navigating. |
| `getURL()` | `string` | Current top-level URL. |
| `getTitle()` | `string` | Current page title. |
| `isLoading()` | `boolean` | |
| `isLoadingMainFrame()` | `boolean` | |
| `isWaitingForResponse()` | `boolean` | |
| `stop()` | `void` | Stops the current navigation. |
| `reload()` | `void` | |
| `reloadIgnoringCache()` | `void` | |
| `canGoBack()` | `boolean` | |
| `canGoForward()` | `boolean` | |
| `canGoToOffset(offset)` | `boolean` | |
| `clearHistory()` | `void` | Best-effort; guest history length is clamped. |
| `goBack()` | `void` | |
| `goForward()` | `void` | |
| `goToIndex(index)` | `void` | |
| `goToOffset(offset)` | `void` | |
| `isCrashed()` | `boolean` | Always `false` until a crash signal is available. |

`loadURL` options:

```ts
interface LoadURLOptions {
  httpReferrer?: string
  userAgent?: string
  extraHeaders?: string
  postData?: UploadData[]
  baseURLForDataURL?: string
}
```

`downloadURL` options:

```ts
interface DownloadURLOptions {
  headers?: Record<string, string>
}
```

### User agent

| Method | Behavior |
| --- | --- |
| `setUserAgent(userAgent)` | Throws: the user agent is fixed at webview creation; set the `useragent` attribute instead. |
| `getUserAgent()` | Returns the user agent configured at creation. |

### Scripts and styles

| Method | Returns | Behavior |
| --- | --- | --- |
| `insertCSS(css)` | `Promise<string>` | Inserts a stylesheet and resolves with its key. |
| `removeInsertedCSS(key)` | `Promise<void>` | Removes a stylesheet by key. |
| `executeJavaScript(code[, userGesture])` | `Promise<any>` | Resolves with the value returned by the code; rejects when the code throws or returns a rejected promise. |

### DevTools

| Method | Behavior |
| --- | --- |
| `openDevTools()` | Opens the CEF devtools window. |
| `closeDevTools()` | |
| `isDevToolsOpened()` | |
| `isDevToolsFocused()` | Always `false`. |
| `inspectElement(x, y)` | Opens devtools; the inspected target is best-effort. |
| `inspectSharedWorker()` | Opens devtools; no target selection. |
| `inspectServiceWorker()` | Opens devtools; no target selection. |

### Audio

| Method | Behavior |
| --- | --- |
| `setAudioMuted(muted)` | Mutes media elements in the guest. |
| `isAudioMuted()` | |
| `isCurrentlyAudible()` | |

### Editing

All editing methods run in the guest document and return `void`.

`undo()`, `redo()`, `cut()`, `copy()`, `centerSelection()`, `paste()`, `pasteAndMatchStyle()`, `delete()`, `selectAll()`, `unselect()`, `scrollToTop()`, `scrollToBottom()`, `adjustSelection(options)`, `replace(text)`, `replaceMisspelling(text)`, `insertText(text)`.

`adjustSelection` options: `{ start?: number, end?: number }`.

### Find in page

| Method | Returns | Behavior |
| --- | --- | --- |
| `findInPage(text[, options])` | `number` | Starts a search and returns a request id. Results arrive as `found-in-page`. |
| `stopFindInPage(action)` | `void` | `'clearSelection'`, `'keepSelection'` or `'activateSelection'`. |

`findInPage` options: `{ forward?: boolean, findNext?: boolean, matchCase?: boolean }` (defaults: `true`, `false`, `false`).

### Printing

| Method | Returns | Behavior |
| --- | --- | --- |
| `print([options])` | `Promise<void>` | Opens the native print dialog. |
| `printToPDF(options)` | `Promise<Uint8Array>` | Rejects: no PDF seam in the CEF runtime. |

`print` options: `silent`, `printBackground`, `deviceName`, `color`, `margins` (`marginType`, `top`, `bottom`, `left`, `right`), `landscape`, `scaleFactor`, `pagesPerSheet`, `collate`, `copies`, `pageRanges` (`{ from, to }[]`), `duplexMode`, `dpi` (`horizontal`, `vertical`), `header`, `footer`, `pageSize`, `usePrinterDefaultPageSize`.

### Input and IPC

| Method | Returns | Behavior |
| --- | --- | --- |
| `send(channel, ...args)` | `Promise<void>` | Delivers to the guest `ipcRenderer` shim. |
| `sendToFrame(frameId, channel, ...args)` | `Promise<void>` | Same, no frame targeting; the main frame receives it. |
| `sendInputEvent(event)` | `Promise<void>` | Dispatches a synthetic mouse, wheel or keyboard event in the guest. |

### Capture

`capturePage([rect])` rejects: the CEF runtime exposes no screenshot seam.

### Zoom

| Method | Behavior |
| --- | --- |
| `setZoomFactor(factor)` | Guest-side zoom via `document.documentElement.style.zoom`. |
| `setZoomLevel(level[, scale])` | `factor = scale ** level`, default `scale` is `1.2`. |
| `getZoomFactor()` | |
| `getZoomLevel()` | |
| `setVisualZoomLevelLimits(min, max)` | No-op; resolves. |
| `showDefinitionForSelection()` | No-op. |

### Misc

| Method | Returns |
| --- | --- |
| `getWebContentsId()` | `number` |

## Events

Events are dispatched on the `<webview>` element. Payload fields are copied onto the event object. Event objects support `addEventListener`, `on<event>` properties, and `removeEventListener`.

| Event | Payload | Source |
| --- | --- | --- |
| `did-attach` | | native |
| `load-commit` | `url`, `isMainFrame` | native |
| `did-start-loading` | | native |
| `did-stop-loading` | | native |
| `did-finish-load` | | native |
| `did-frame-finish-load` | `isMainFrame` | native |
| `did-start-navigation` | `url`, `isInPlace`, `isMainFrame`, `frameProcessId`, `frameRoutingId` | native |
| `did-frame-navigate` | `url`, `httpResponseCode`, `httpStatusText`, `isMainFrame`, `frameProcessId`, `frameRoutingId` | native |
| `did-navigate` | `url` | native |
| `will-navigate` | `url` | native; cannot be prevented |
| `will-frame-navigate` | `url`, `isMainFrame`, `frameProcessId`, `frameRoutingId` | native; cannot be prevented |
| `page-title-updated` | `title`, `explicitSet` | native |
| `dom-ready` | | guest |
| `page-favicon-updated` | `favicons` | guest |
| `did-change-theme-color` | `themeColor` | guest |
| `update-target-url` | `url` | guest |
| `console-message` | `level`, `message`, `line`, `sourceId` | guest |
| `context-menu` | `params` | guest |
| `enter-html-full-screen` | | guest |
| `leave-html-full-screen` | | guest |
| `media-started-playing` | | guest |
| `media-paused` | | guest |
| `did-navigate-in-page` | `isMainFrame`, `url` | guest |
| `ipc-message` | `frameId`, `channel`, `args` | guest, from `ipcRenderer.sendToHost` |
| `found-in-page` | `result` | guest |
| `close` | | guest, from `window.close` |
| `destroyed` | | native |

Declared but not emitted, because the runtime exposes no seam: `did-fail-load`, `did-redirect-navigation`, `render-process-gone`, `devtools-open-url`, `devtools-search-query`, `devtools-opened`, `devtools-closed`, `devtools-focused`.

### `context-menu` params

`x`, `y`, `linkURL`, `linkText`, `pageURL`, `frameURL`, `srcURL`, `mediaType` (`'none' | 'image' | 'audio' | 'video' | 'canvas' | 'file' | 'plugin'`), `hasImageContents`, `isEditable`, `selectionText`, `titleText`, `altText`, `suggestedFilename`, `selectionRect` (`'x' | 'y' | 'width' | 'height'`), `selectionStartOffset`, `referrerPolicy` (`{ url, policy }`), `misspelledWord`, `dictionarySuggestions`, `frameCharset`, `formControlType`, `spellcheckEnabled`, `menuSourceType`, `mediaFlags` (`inError`, `isPaused`, `isMuted`, `hasAudio`, `isLooping`, `isControlsVisible`, `canToggleControls`, `canPrint`, `canSave`, `canShowPictureInPicture`, `isShowingPictureInPicture`, `canRotate`, `canLoop`), `editFlags` (`canUndo`, `canRedo`, `canCut`, `canCopy`, `canPaste`, `canDelete`, `canSelectAll`, `canEditRichly`).

### `ipc-message` shim

The guest runtime installs a `window.ipcRenderer` shim:

| Member | Behavior |
| --- | --- |
| `on(channel, listener)`, `once(...)`, `off(...)`, `removeListener(...)`, `removeAllListeners(...)` | Receives `send`/`sendToFrame` payloads. |
| `send(channel, ...args)` | Sends to the host as `ipc-message` with `frameId: 0`. |
| `sendToHost(channel, ...args)` | Alias of `send`. |
| `invoke(...)` | Rejects: no guest-to-host command channel. |

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

`Registry::get` returns a handle with `label`, `inner`, `eval`, `insert_css`, `remove_inserted_css`, `navigate`, `reload`, `open_devtools`, `set_rect`, `set_visible` and `close`.

Global scripts and styles for every embedded webview:

```rust
tauri_plugin_cef_webview::init_with(
  tauri_plugin_cef_webview::Config::default()
    .js("document.documentElement.dataset.embedded = 'true'")
    .css("body { font-family: system-ui; }"),
)
```

Events reach the frontend as native Tauri events named `cef-webview://event`:

```rust
use tauri::Listener;

app.listen("cef-webview://event", |event| {
  println!("{}", event.payload());
});
```

Payload shape: `{ label, event, args }`, where `event` is the DOM event name and `args` its payload array.

## How it works

* The plugin registers `element.js` as a Tauri initialization script, so it runs at document start before any page script, including in windows created from `tauri.conf.json`.
* The element talks to the plugin through Tauri IPC (`plugin:cef-webview|*` commands): `create`, `set_rect`, `set_visible`, `destroy`, `call` and the per-operation commands.
* The plugin injects `guest.js` into every child webview. It reports events the CEF runtime does not surface, applies zoom, mute and editing, and provides the `ipcRenderer` shim.
* Guest events are batched into a `cef-webview://event/` custom-scheme navigation every 16ms. The plugin cancels that navigation in `on_navigation` and forwards the batch to the application page. The bridge is main-frame only.
* `js`/`css` from `Config` and from element attributes are registered as CEF initialization scripts, so they apply on every navigation without flashing.
* The plugin refuses to create a webview from a child webview, because the host element API is injected into children too.

## Limitations

* The native webview is drawn above the page. CSS `transform`, `opacity`, `border-radius`, `overflow: hidden` clipping and `z-index` stacking do not affect it. An element that must be covered cannot be placed over the webview.
* Position is synced on a best-effort basis; scrolling may show a few frames of lag.
* The `<webview>` tag name has no hyphen, so it cannot be registered with `customElements.define`. The plugin enhances the element in place: the DOM node stays `<webview>` and is not an instance of a custom element class.
* Only the CEF runtime (`tauri-runtime-cef`) is supported.
* Sandboxing, `partition`, Node.js integration and Blink/Chromium preferences are process-wide in CEF and cannot be configured per element.
* `did-fail-load` and crash reporting are unavailable: the runtime does not deliver load errors or renderer termination.

## TypeScript

`src/types.ts` mirrors the element API for TypeScript users:

```ts
import type { WebviewElement } from 'tauri-cef-webview'

const webview = document.querySelector('webview') as WebviewElement
await webview.loadURL('https://example.com/')
```

The package exports types only; the runtime is injected by the plugin.

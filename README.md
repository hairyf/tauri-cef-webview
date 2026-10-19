# tauri-cef-webview

Embed native CEF child webviews in a Tauri v3 application and control them with a plain `<webview>` element.

```html
<webview id="news" src="https://example.com/"></webview>
```

The element is only a layout box: the actual page is rendered by a native child webview positioned on top of it. Size and position are kept in sync with `getBoundingClientRect()`.

## Install

```bash
pnpm add tauri-cef-webview @tauri-apps/api
cargo add tauri-plugin-cef-webview
```

```toml
# src-tauri/Cargo.toml
[dependencies]
tauri-plugin-cef-webview = "0.2.0"
```

## Setup

Register the plugin once. That is all the Rust code required.

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

You may also inject scripts/styles into every embedded webview:

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

Then import the frontend package once, e.g. in your entry file:

```ts
import 'tauri-cef-webview'
```

It installs the API on every `<webview>` element in the document, including elements added later.

## Element API

```ts
import type { WebviewElement } from 'tauri-cef-webview'

const el = document.querySelector<WebviewElement>('webview')!

await el.ready // native webview created
await el.eval('document.title') // evaluate JavaScript
const key = await el.insertCSS('body { background: #111 }')
await el.removeInsertedCSS(key)
await el.navigate('https://example.org/')
await el.reload()
await el.openDevtools() // debug builds only
```

Attributes are read once, when the element is mounted:

| Attribute | Description |
| --- | --- |
| `src` | Required. Initial URL. |
| `id` | Optional. Becomes the native webview label, so Rust can address it by the same name. Generated when omitted. |
| `js` | JavaScript evaluated before page scripts on every navigation. |
| `css` | Stylesheet applied on every navigation. |

For dynamic content prefer `eval()`, `insertCSS()` or the Rust-side API below. They act on the current document only; styles and scripts that must survive a navigation belong in the `css`/`js` attributes or in the Rust `Config`.

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

* On mount the element calls the plugin's `create` command, passing its `getBoundingClientRect()`.
* A shared `requestAnimationFrame` loop pushes position changes (scroll, resize, layout) to the native layer.
* `ResizeObserver` triggers the same loop, `IntersectionObserver` hides the native webview when the element leaves the viewport or collapses to zero size.
* Removing the element from the DOM closes the native webview.
* JS/CSS from `Config`, `js` and `css` attributes are registered as CEF initialization scripts, so they apply on every navigation without flashing.

## Limitations

* The native webview is drawn above the page. CSS `transform`, `opacity`, `border-radius`, `overflow: hidden` clipping and `z-index` stacking do not affect it. An element that must be covered cannot be placed over the webview.
* Position is synced on a best-effort basis; scrolling may show a few frames of lag.
* The `<webview>` tag name has no hyphen, so it cannot be registered with `customElements.define`. The package enhances the element in place instead: the DOM node stays `<webview>` and is not an instance of a custom element class.
* Only the CEF runtime (`tauri-runtime-cef`) is supported.

/// JavaScript that inserts (or updates) a `<style>` element identified by `key`.
///
/// The style is applied as soon as `document.head` exists, so this can be used
/// both at document-start (as an initialization script) and at runtime.
pub(crate) fn insert_style(key: &str, css: &str) -> String {
    let key = serde_json::to_string(key).expect("style key is always a valid string");
    let css = serde_json::to_string(css).expect("css is always a valid string");
    format!(
        r#"(() => {{
  const key = {key};
  const css = {css};
  const apply = () => {{
    const head = document.head || document.documentElement;
    if (!head) return false;
    let el = document.querySelector('style[data-cwv-style="' + key + '"]');
    if (!el) {{
      el = document.createElement('style');
      el.setAttribute('data-cwv-style', key);
      head.appendChild(el);
    }}
    if (el.textContent !== css) el.textContent = css;
    return true;
  }};
  if (apply()) return;
  const observer = new MutationObserver(() => {{
    if (apply()) observer.disconnect();
  }});
  observer.observe(document, {{ childList: true, subtree: true }});
}})();"#,
    )
}

/// JavaScript that removes the `<style>` element created by [`insert_style`].
pub(crate) fn remove_style(key: &str) -> String {
    let key = serde_json::to_string(key).expect("style key is always a valid string");
    format!(
        r#"(() => {{
  const el = document.querySelector('style[data-cwv-style="' + {key} + '"]');
  if (el) el.remove();
}})();"#,
    )
}

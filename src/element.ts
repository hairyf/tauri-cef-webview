import { invoke } from '@tauri-apps/api/core'

const PLUGIN_NAME = 'cef-webview'

function command(name: string): string {
  return `plugin:${PLUGIN_NAME}|${name}`
}

/** Position and size of an embedded webview, in logical pixels. */
export interface WebviewRect {
  x: number
  y: number
  width: number
  height: number
}

/**
 * Public API installed on every `<webview>` element.
 *
 * The element keeps its place in the DOM and is used as the layout box of the
 * native child webview.
 */
export interface WebviewElement extends HTMLElement {
  /** Native label of the embedded webview. Matches the element `id` when set. */
  readonly label: string
  /** Resolves once the native webview has been created. */
  readonly ready: Promise<WebviewElement>
  /** Evaluates JavaScript inside the embedded webview. */
  eval: (js: string) => Promise<void>
  /** Injects a stylesheet and resolves with the key used to remove it. */
  insertCSS: (css: string) => Promise<string>
  /** Removes a stylesheet previously added with {@link WebviewElement.insertCSS}. */
  removeInsertedCSS: (key: string) => Promise<void>
  /** Navigates the embedded webview. */
  navigate: (url: string) => Promise<void>
  /** Reloads the embedded webview. */
  reload: () => Promise<void>
  /** Opens the embedded webview devtools (debug builds only). */
  openDevtools: () => Promise<void>
}

function noop(): void {}

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

function nearlyEqual(a: WebviewRect, b: WebviewRect): boolean {
  return (
    Math.abs(a.x - b.x) < 0.5
    && Math.abs(a.y - b.y) < 0.5
    && Math.abs(a.width - b.width) < 0.5
    && Math.abs(a.height - b.height) < 0.5
  )
}

const controllers = new Set<Controller>()
const mounted = new WeakMap<HTMLElement, Controller>()
let sequence = 0
let frame = 0

function schedule(): void {
  if (frame || controllers.size === 0)
    return
  frame = requestAnimationFrame(() => {
    frame = 0
    for (const controller of controllers)
      controller.sync()
  })
}

class Controller {
  readonly label: string
  readonly ready: Promise<WebviewElement>

  private queue: Promise<unknown>
  private rect: WebviewRect | null = null
  private visible = true
  private intersecting = true
  private destroyed = false
  private readonly resizeObserver: ResizeObserver
  private readonly intersectionObserver: IntersectionObserver

  constructor(
    private readonly element: HTMLElement,
    label: string,
  ) {
    this.label = label

    const src = element.getAttribute('src')
    if (!src)
      throw new Error('<webview> requires a `src` attribute')

    const js = element.getAttribute('js')
    const css = element.getAttribute('css')
    const rect = this.measure()

    this.queue = invoke(command('create'), {
      label,
      url: new URL(src, document.baseURI).href,
      x: rect.x,
      y: rect.y,
      width: Math.max(rect.width, 1),
      height: Math.max(rect.height, 1),
      js: js ? [js] : [],
      css: css ? [css] : [],
    })
    this.queue.catch((error) => {
      console.error(`[cef-webview] failed to create webview "${label}"`, error)
    })

    this.ready = this.queue.then(() => element as WebviewElement)
    this.ready.catch(noop)

    this.resizeObserver = new ResizeObserver(() => schedule())
    this.resizeObserver.observe(element)

    this.intersectionObserver = new IntersectionObserver(([entry]) => {
      this.intersecting = entry.isIntersecting
      schedule()
    })
    this.intersectionObserver.observe(element)

    schedule()
  }

  /** Pushes the current layout box to the native webview. */
  sync(): void {
    if (this.destroyed || !this.element.isConnected)
      return

    const next = this.measure()
    const empty = next.width <= 0 || next.height <= 0

    this.applyVisible(!empty && this.intersecting)
    if (empty)
      return

    const previous = this.rect
    this.rect = next
    if (previous && nearlyEqual(previous, next))
      return

    this.run(() => invoke(command('set_rect'), { label: this.label, ...next }))
      .catch(noop)
  }

  eval(js: string): Promise<void> {
    return this.run(() => invoke(command('eval'), { label: this.label, js }))
  }

  insertCSS(css: string): Promise<string> {
    return this.run(() => invoke<string>(command('insert_css'), { label: this.label, css }))
  }

  removeInsertedCSS(key: string): Promise<void> {
    return this.run(() => invoke(command('remove_inserted_css'), { label: this.label, key }))
  }

  navigate(url: string): Promise<void> {
    return this.run(() => invoke(command('navigate'), { label: this.label, url }))
  }

  reload(): Promise<void> {
    return this.run(() => invoke(command('reload'), { label: this.label }))
  }

  openDevtools(): Promise<void> {
    return this.run(() => invoke(command('open_devtools'), { label: this.label }))
  }

  destroy(): void {
    if (this.destroyed)
      return
    this.destroyed = true
    controllers.delete(this)
    mounted.delete(this.element)
    this.resizeObserver.disconnect()
    this.intersectionObserver.disconnect()
    this.run(() => invoke(command('destroy'), { label: this.label })).catch(noop)
  }

  private measure(): WebviewRect {
    const box = this.element.getBoundingClientRect()
    return {
      x: box.left,
      y: box.top,
      width: box.width,
      height: box.height,
    }
  }

  private applyVisible(visible: boolean): void {
    if (visible === this.visible)
      return
    this.visible = visible
    this.run(() => invoke(command('set_visible'), { label: this.label, visible }))
      .catch(noop)
  }

  private run<T>(task: () => Promise<T>): Promise<T> {
    const result = this.queue.then(task)
    this.queue = result.catch(noop)
    return result
  }
}

/** Installs the `<webview>` API on an element. */
function enhance(element: HTMLElement): WebviewElement {
  if (mounted.has(element))
    return element as WebviewElement

  const label = element.id || `cef-webview-${++sequence}`
  const controller = new Controller(element, label)
  mounted.set(element, controller)
  controllers.add(controller)

  Object.defineProperties(element, {
    label: { value: label, configurable: true },
    ready: { value: controller.ready, configurable: true },
  })

  const api = element as WebviewElement
  api.eval = js => controller.eval(js)
  api.insertCSS = css => controller.insertCSS(css)
  api.removeInsertedCSS = key => controller.removeInsertedCSS(key)
  api.navigate = url => controller.navigate(url)
  api.reload = () => controller.reload()
  api.openDevtools = () => controller.openDevtools()

  return api
}

function scan(root: Node): void {
  if (root instanceof HTMLElement && root.localName === 'webview')
    enhance(root)

  if (!(root instanceof Document) && !(root instanceof Element))
    return

  for (const element of root.querySelectorAll('webview')) {
    if (element instanceof HTMLElement)
      enhance(element)
  }
}

function scanRemoved(root: Node): void {
  if (!(root instanceof Element))
    return

  const destroy = (element: Element): void => {
    if (!(element instanceof HTMLElement))
      return
    const controller = mounted.get(element)
    if (!controller)
      return
    queueMicrotask(() => {
      if (!element.isConnected)
        controller.destroy()
    })
  }

  destroy(root)
  for (const element of root.querySelectorAll('webview'))
    destroy(element)
}

let installed = false

/**
 * Starts managing every `<webview>` element in the document.
 *
 * Called automatically when this module is imported inside a Tauri webview.
 */
export function installWebviews(): void {
  if (installed || !isTauri() || typeof document === 'undefined')
    return
  installed = true

  scan(document)

  const observer = new MutationObserver((records) => {
    for (const record of records) {
      for (const node of record.addedNodes)
        scan(node)
      for (const node of record.removedNodes)
        scanRemoved(node)
    }
  })
  observer.observe(document.documentElement, { childList: true, subtree: true })

  window.addEventListener('scroll', schedule, { capture: true, passive: true })
  window.addEventListener('resize', schedule, { passive: true })
}

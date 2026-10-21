// Injected into the pages of the Tauri application by the cef-webview plugin.
//
// It installs the Electron-compatible `<webview>` element API. Page scripts do
// not import anything: the element gains the methods and properties as soon as
// this script runs.
(() => {
  'use strict'

  if (window.__cefWebviewInstalled)
    return
  if (!window.__TAURI_INTERNALS__)
    return

  window.__cefWebviewInstalled = true

  const PLUGIN = 'cef-webview'
  const EVENT = 'cef-webview://event'
  const BOOLEAN_ATTRIBUTES = [
    'nodeintegration',
    'nodeintegrationinsubframes',
    'plugins',
    'disablewebsecurity',
    'allowpopups',
  ]
  const STRING_ATTRIBUTES = [
    'preload',
    'httpreferrer',
    'useragent',
    'partition',
    'webpreferences',
    'enableblinkfeatures',
    'disableblinkfeatures',
  ]
  const UNSUPPORTED_ATTRIBUTES = [
    'nodeintegration',
    'nodeintegrationinsubframes',
    'plugins',
    'httpreferrer',
    'disablewebsecurity',
    'partition',
    'webpreferences',
    'enableblinkfeatures',
    'disableblinkfeatures',
  ]
  const IMMUTABLE_ATTRIBUTES = new Set(['partition', 'disablewebsecurity'])

  const internals = window.__TAURI_INTERNALS__
  const controllers = new Set()
  const mounted = new WeakMap()
  const byLabel = new Map()
  let sequence = 0
  let frame = 0

  function command(name) {
    return `plugin:${PLUGIN}|${name}`
  }

  function invoke(name, args) {
    return internals.invoke(command(name), args)
  }

  function noop() {}

  function isTauri() {
    return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
  }

  function nearlyEqual(a, b) {
    return (
      Math.abs(a.x - b.x) < 0.5
      && Math.abs(a.y - b.y) < 0.5
      && Math.abs(a.width - b.width) < 0.5
      && Math.abs(a.height - b.height) < 0.5
    )
  }

  function defaultState() {
    return {
      url: '',
      title: '',
      loading: false,
      waitingForResponse: false,
      canGoBack: false,
      canGoForward: false,
      historyIndex: 0,
      crashed: false,
      audioMuted: false,
      audible: false,
      userAgent: '',
      zoomFactor: 1,
      zoomLevel: 0,
      webContentsId: 0,
    }
  }

  function schedule() {
    if (frame || controllers.size === 0)
      return
    frame = requestAnimationFrame(() => {
      frame = 0
      for (const controller of controllers)
        controller.sync()
    })
  }

  function subscribe() {
    const id = internals.transformCallback((event) => {
      const payload = event && event.payload
      if (!payload)
        return
      const controller = byLabel.get(payload.label)
      if (controller)
        controller.receive(payload.event, payload.args || {})
    })

    internals.invoke('plugin:event|listen', {
      event: EVENT,
      target: { kind: 'Any' },
      handler: id,
    }).catch((error) => {
      console.error('[cef-webview] failed to listen for webview events', error)
    })
  }

  function createEvent(type, args) {
    const event = new Event(type)
    for (const key of Object.keys(args)) {
      Object.defineProperty(event, key, {
        value: args[key],
        enumerable: true,
        configurable: true,
      })
    }
    return event
  }

  function parseJson(value) {
    if (typeof value !== 'string')
      return value
    try {
      return JSON.parse(value)
    }
    catch {
      return value
    }
  }

  class Controller {
    constructor(element, label) {
      this.element = element
      this.label = label
      this.state = defaultState()
      this.queue = Promise.resolve()
      this.rect = null
      this.visible = true
      this.intersecting = true
      this.destroyed = false
      this.devToolsOpen = false
      this.executeSequence = 0
      this.findSequence = 0
      this.pending = new Map()

      const src = element.getAttribute('src')
      if (!src)
        throw new Error('<webview> requires a `src` attribute')

      this.warnUnsupported()
      const js = element.getAttribute('js')
      const css = element.getAttribute('css')
      const rect = this.measure()

      this.queue = invoke('create', {
        label,
        url: new URL(src, document.baseURI).href,
        x: rect.x,
        y: rect.y,
        width: Math.max(rect.width, 1),
        height: Math.max(rect.height, 1),
        js: js ? [js] : [],
        css: css ? [css] : [],
        options: this.createOptions(),
      }).then((state) => {
        this.applyState(state)
      })
      this.queue.catch((error) => {
        console.error(`[cef-webview] failed to create webview "${label}"`, error)
      })

      this.ready = this.queue.then(() => element)

      this.resizeObserver = new ResizeObserver(() => schedule())
      this.resizeObserver.observe(element)

      this.intersectionObserver = new IntersectionObserver(([entry]) => {
        this.intersecting = entry.isIntersecting
        schedule()
      })
      this.intersectionObserver.observe(element)

      schedule()
    }

    receive(event, args) {
      if (event === 'state') {
        this.applyState(args)
        return
      }

      if (event === 'execute-javascript') {
        const id = Number(args.id)
        const pending = this.pending.get(id)
        if (!pending)
          return
        this.pending.delete(id)
        if (typeof args.error === 'string')
          pending.reject(new Error(args.error))
        else
          pending.resolve(parseJson(args.value))
        return
      }

      this.element.dispatchEvent(createEvent(event, args))
    }

    sync() {
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

      this.run(() => invoke('set_rect', { label: this.label, ...next })).catch(noop)
    }

    call(method, args) {
      return invoke('call', {
        label: this.label,
        method,
        args: { label: this.label, ...(args || {}) },
      })
    }

    fire(method, args) {
      this.call(method, args).catch(noop)
    }

    loadURL(url, options) {
      return this.call('loadURL', { url, options: options || null })
    }

    navigate(url) {
      return this.loadURL(url)
    }

    setUserAgent(userAgent) {
      this.call('setUserAgent', { userAgent }).catch((error) => {
        console.warn(`[cef-webview] ${error}`)
      })
    }

    executeJavaScript(code, userGesture) {
      const id = ++this.executeSequence
      return new Promise((resolve, reject) => {
        this.pending.set(id, { resolve, reject })
        this.call('executeJavaScript', { id, code, userGesture: !!userGesture }).catch((error) => {
          this.pending.delete(id)
          reject(error)
        })
      })
    }

    openDevTools() {
      this.devToolsOpen = true
      this.fire('openDevTools')
    }

    closeDevTools() {
      this.devToolsOpen = false
      this.fire('closeDevTools')
    }

    setAudioMuted(muted) {
      this.state.audioMuted = muted
      this.fire('setAudioMuted', { muted })
    }

    edit(command, value) {
      this.fire('edit', { command, value: value === undefined ? null : value })
    }

    findInPage(text, options) {
      const requestId = ++this.findSequence
      this.fire('findInPage', { requestId, text, options: options || {} })
      return requestId
    }

    setZoomFactor(factor) {
      this.state.zoomFactor = factor
      this.state.zoomLevel = Math.log(factor) / Math.log(1.2)
      this.fire('setZoomFactor', { factor })
    }

    setZoomLevel(level, scale) {
      const factor = (scale || 1.2) ** level
      this.state.zoomFactor = factor
      this.state.zoomLevel = level
      this.fire('setZoomLevel', { level, scale: scale || 1.2 })
    }

    getWebContentsId() {
      return this.state.webContentsId
    }

    getState() {
      return this.state
    }

    destroy() {
      if (this.destroyed)
        return
      this.destroyed = true
      controllers.delete(this)
      mounted.delete(this.element)
      byLabel.delete(this.label)
      this.resizeObserver.disconnect()
      this.intersectionObserver.disconnect()
      this.element.dispatchEvent(createEvent('destroyed', {}))
      for (const pending of this.pending.values())
        pending.reject(new Error('webview was destroyed'))
      this.pending.clear()
      this.run(() => invoke('destroy', { label: this.label })).catch(noop)
    }

    createOptions() {
      const options = {}
      const userAgent = this.element.getAttribute('useragent')
      if (userAgent)
        options.userAgent = userAgent
      const preload = this.element.getAttribute('preload')
      if (preload)
        options.preload = preload
      if (this.element.hasAttribute('allowpopups'))
        options.allowPopups = true
      return options
    }

    warnUnsupported() {
      for (const name of UNSUPPORTED_ATTRIBUTES) {
        if (this.element.getAttribute(name) === null)
          continue
        console.warn(
          `[cef-webview] the \`${name}\` attribute is not supported by the CEF runtime and is ignored`,
        )
      }
    }

    applyState(state) {
      this.state = { ...this.state, ...state }
    }

    measure() {
      const box = this.element.getBoundingClientRect()
      return {
        x: box.left,
        y: box.top,
        width: box.width,
        height: box.height,
      }
    }

    applyVisible(visible) {
      if (visible === this.visible)
        return
      this.visible = visible
      this.run(() => invoke('set_visible', { label: this.label, visible })).catch(noop)
    }

    run(task) {
      const result = this.queue.then(task)
      this.queue = result.catch(noop)
      return result
    }
  }

  function defineAttribute(element, controller, name, boolean) {
    Object.defineProperty(element, name, {
      configurable: true,
      enumerable: true,
      get: () => {
        if (!boolean)
          return element.getAttribute(name) || ''
        return element.hasAttribute(name)
      },
      set: (value) => {
        if (IMMUTABLE_ATTRIBUTES.has(name) && controller.getState().webContentsId !== 0) {
          throw new DOMException(
            `<webview> \`${name}\` cannot be changed after the webview is attached`,
            'InvalidStateError',
          )
        }
        if (boolean) {
          if (value)
            element.setAttribute(name, '')
          else
            element.removeAttribute(name)
        }
        else {
          element.setAttribute(name, String(value))
        }
      },
    })
  }

  function enhance(element) {
    if (mounted.has(element))
      return element

    const label = element.id || `cef-webview-${++sequence}`
    const controller = new Controller(element, label)
    mounted.set(element, controller)
    controllers.add(controller)
    byLabel.set(label, controller)

    const state = () => controller.getState()

    Object.defineProperties(element, {
      label: { get: () => label, configurable: true },
      ready: { get: () => controller.ready, configurable: true },
      src: {
        configurable: true,
        enumerable: true,
        get: () => element.getAttribute('src') || '',
        set: (value) => {
          const previous = element.getAttribute('src')
          element.setAttribute('src', value)
          if (controller.getState().webContentsId === 0)
            return
          if (previous === value)
            controller.fire('reload')
          else
            controller.navigate(value).catch(noop)
        },
      },
    })

    for (const name of BOOLEAN_ATTRIBUTES)
      defineAttribute(element, controller, name, true)
    for (const name of STRING_ATTRIBUTES)
      defineAttribute(element, controller, name, false)

    element.loadURL = (url, options) => controller.loadURL(url, options)
    element.downloadURL = (url, options) => controller.fire('downloadURL', { url, options: options || null })
    element.getURL = () => state().url
    element.getTitle = () => state().title
    element.isLoading = () => state().loading
    element.isLoadingMainFrame = () => state().loading
    element.isWaitingForResponse = () => state().waitingForResponse
    element.stop = () => controller.fire('stop')
    element.reload = () => controller.fire('reload')
    element.reloadIgnoringCache = () => controller.fire('reloadIgnoringCache')
    element.canGoBack = () => state().canGoBack
    element.canGoForward = () => state().canGoForward
    element.canGoToOffset = offset => offset < 0
      ? state().canGoBack
      : offset > 0
        ? state().canGoForward
        : true
    element.clearHistory = () => controller.fire('clearHistory')
    element.goBack = () => controller.fire('goBack')
    element.goForward = () => controller.fire('goForward')
    element.goToIndex = index => controller.fire('goToIndex', { index })
    element.goToOffset = offset => controller.fire('goToOffset', { offset })
    element.isCrashed = () => state().crashed
    element.setUserAgent = userAgent => controller.setUserAgent(userAgent)
    element.getUserAgent = () => state().userAgent
    element.insertCSS = css => controller.call('insertCSS', { css })
    element.removeInsertedCSS = key => controller.call('removeInsertedCSS', { key })
    element.executeJavaScript = (code, userGesture) => controller.executeJavaScript(code, userGesture)
    element.openDevTools = () => controller.openDevTools()
    element.closeDevTools = () => controller.closeDevTools()
    element.isDevToolsOpened = () => controller.devToolsOpen
    element.isDevToolsFocused = () => false
    element.inspectElement = (x, y) => {
      controller.devToolsOpen = true
      controller.fire('inspectElement', { x, y })
    }
    element.inspectSharedWorker = () => controller.fire('inspectSharedWorker')
    element.inspectServiceWorker = () => controller.fire('inspectServiceWorker')
    element.setAudioMuted = muted => controller.setAudioMuted(muted)
    element.isAudioMuted = () => state().audioMuted
    element.isCurrentlyAudible = () => state().audible

    element.undo = () => controller.edit('undo')
    element.redo = () => controller.edit('redo')
    element.cut = () => controller.edit('cut')
    element.copy = () => controller.edit('copy')
    element.centerSelection = () => controller.edit('centerSelection')
    element.paste = () => controller.edit('paste')
    element.pasteAndMatchStyle = () => controller.edit('pasteAndMatchStyle')
    element.delete = () => controller.edit('delete')
    element.selectAll = () => controller.edit('selectAll')
    element.unselect = () => controller.edit('unselect')
    element.scrollToTop = () => controller.edit('scrollToTop')
    element.scrollToBottom = () => controller.edit('scrollToBottom')
    element.adjustSelection = options => controller.edit('adjustSelection', options)
    element.replace = text => controller.edit('replace', text)
    element.replaceMisspelling = text => controller.edit('replaceMisspelling', text)
    element.insertText = text => controller.edit('insertText', text)

    element.findInPage = (text, options) => controller.findInPage(text, options)
    element.stopFindInPage = action => controller.fire('stopFindInPage', { action })
    element.print = options => controller.call('print', { options: options || null })
    element.printToPDF = options => controller.call('printToPDF', { options: options || null })
    element.capturePage = rect => controller.call('capturePage', { rect: rect || null })
    element.send = (channel, ...args) => controller.call('send', { channel, args })
    element.sendToFrame = (frameId, channel, ...args) => controller.call('sendToFrame', { frameId, channel, args })
    element.sendInputEvent = event => controller.call('sendInputEvent', { event })
    element.setZoomFactor = factor => controller.setZoomFactor(factor)
    element.setZoomLevel = (level, scale) => controller.setZoomLevel(level, scale)
    element.getZoomFactor = () => state().zoomFactor
    element.getZoomLevel = () => state().zoomLevel
    element.setVisualZoomLevelLimits = (min, max) => controller.call('setVisualZoomLevelLimits', { min, max })
    element.showDefinitionForSelection = () => controller.fire('showDefinitionForSelection')
    element.getWebContentsId = () => controller.getWebContentsId()

    element.navigate = url => controller.navigate(url)
    element.eval = js => controller.executeJavaScript(js).then(noop)
    element.openDevtools = () => controller.openDevTools()

    return element
  }

  function scan(root) {
    if (root instanceof HTMLElement && root.localName === 'webview') {
      try {
        enhance(root)
      }
      catch (error) {
        console.error('[cef-webview] failed to enhance <webview>', error)
      }
    }

    if (!(root instanceof Document) && !(root instanceof Element))
      return

    for (const element of root.querySelectorAll('webview')) {
      if (element instanceof HTMLElement) {
        try {
          enhance(element)
        }
        catch (error) {
          console.error('[cef-webview] failed to enhance <webview>', error)
        }
      }
    }
  }

  function scanRemoved(root) {
    if (!(root instanceof Element))
      return

    const destroy = (element) => {
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

  function start() {
    subscribe()
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

  if (!isTauri())
    return

  if (document.readyState === 'loading')
    document.addEventListener('DOMContentLoaded', start, { once: true })
  else
    start()
})()

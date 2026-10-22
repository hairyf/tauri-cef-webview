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

  // `createElement` runs synchronously while the observer runs on a microtask,
  // so programmatic elements must be enhanced here to be usable right away.
  const createElement = document.createElement.bind(document)
  document.createElement = function (name, options) {
    const element = createElement(name, options)
    if (typeof name === 'string' && name.toLowerCase() === 'webview')
      enhance(element)
    return element
  }

  const PLUGIN = 'cef-webview'
  const EVENT = 'cef-webview://event'
  // CEF cursor types (`cef_cursor_type_t`) to CSS cursors.
  const CURSORS = {
    1: 'crosshair',
    2: 'pointer',
    3: 'text',
    4: 'wait',
    5: 'help',
    6: 'e-resize',
    7: 'n-resize',
    8: 'ne-resize',
    9: 'nw-resize',
    10: 's-resize',
    11: 'se-resize',
    12: 'sw-resize',
    13: 'w-resize',
    14: 'ns-resize',
    15: 'ew-resize',
    16: 'nesw-resize',
    17: 'nwse-resize',
    18: 'col-resize',
    19: 'row-resize',
    20: 'all-scroll',
    21: 'e-resize',
    22: 'n-resize',
    23: 'ne-resize',
    24: 'nw-resize',
    25: 's-resize',
    26: 'se-resize',
    27: 'sw-resize',
    28: 'w-resize',
    29: 'move',
    30: 'vertical-text',
    31: 'cell',
    32: 'context-menu',
    33: 'alias',
    34: 'progress',
    35: 'no-drop',
    36: 'copy',
    37: 'none',
    38: 'not-allowed',
    39: 'zoom-in',
    40: 'zoom-out',
    41: 'grab',
    42: 'grabbing',
  }
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
      offscreen: false,
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
      this.surface = null
      this.context = null
      this.input = null
      this.animation = 0
      this.painting = false
      this.painted = false
      this.paintedAt = 0
      this.composing = false
      this.point = { x: 0, y: 0 }

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
        this.mountSurface()
        // The native webview only becomes clippable now; force the next
        // overlay tick to re-send instead of trusting the earlier empty one.
        overlaySignature = null
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
      cancelAnimationFrame(this.animation)
      if (this.surface)
        this.surface.remove()
      if (this.input)
        this.input.remove()
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

    mountSurface() {
      if (!this.state.offscreen || this.surface || this.destroyed)
        return
      if (getComputedStyle(this.element).position === 'static')
        this.element.style.position = 'relative'

      const canvas = document.createElement('canvas')
      canvas.setAttribute('part', 'surface')
      Object.assign(canvas.style, {
        position: 'absolute',
        inset: '0',
        width: '100%',
        height: '100%',
        pointerEvents: 'none',
      })

      const input = document.createElement('textarea')
      input.tabIndex = 0
      input.spellcheck = false
      input.setAttribute('aria-label', 'webview content')
      input.setAttribute('autocomplete', 'off')
      Object.assign(input.style, {
        position: 'absolute',
        inset: '0',
        width: '100%',
        height: '100%',
        margin: '0',
        padding: '0',
        border: '0',
        outline: 'none',
        resize: 'none',
        overflow: 'hidden',
        opacity: '0',
        background: 'transparent',
        color: 'transparent',
        caretColor: 'transparent',
        cursor: 'default',
        fontSize: '16px',
        zIndex: '1',
      })

      this.element.appendChild(canvas)
      this.element.appendChild(input)
      this.surface = canvas
      this.context = canvas.getContext('2d')
      this.input = input
      this.bindInput(input)
      this.render()
    }

    bindInput(input) {
      const canvas = this.surface
      const send = event => this.call('sendInputEvent', { event }).catch(noop)
      const modifiers = (event) => {
        const list = []
        if (event.shiftKey)
          list.push('shift')
        if (event.ctrlKey)
          list.push('control')
        if (event.altKey)
          list.push('alt')
        if (event.metaKey)
          list.push('meta')
        if (event.getModifierState && event.getModifierState('CapsLock'))
          list.push('capsLock')
        if (event.getModifierState && event.getModifierState('NumLock'))
          list.push('numLock')
        if (event.location === 1)
          list.push('isLeft')
        if (event.location === 2)
          list.push('isRight')
        if (event.location === 3)
          list.push('isKeypad')
        if (event.repeat)
          list.push('isAutoRepeat')
        if (event.buttons & 1)
          list.push('leftButtonDown')
        if (event.buttons & 4)
          list.push('middleButtonDown')
        if (event.buttons & 2)
          list.push('rightButtonDown')
        return list
      }
      const where = (event) => {
        const box = canvas.getBoundingClientRect()
        this.point = {
          x: Math.round(event.clientX - box.left),
          y: Math.round(event.clientY - box.top),
        }
        return this.point
      }
      const button = event => event.button === 2 ? 'right' : event.button === 1 ? 'middle' : 'left'
      const mouse = (type, event, extra) => send({
        type,
        ...where(event),
        modifiers: modifiers(event),
        ...extra,
      })

      input.addEventListener('mousedown', (event) => {
        event.preventDefault()
        input.focus({ preventScroll: true })
        send({ type: 'focus' })
        mouse('mouseDown', event, { button: button(event), clickCount: event.detail || 1 })
      })
      input.addEventListener('mouseup', (event) => {
        event.preventDefault()
        mouse('mouseUp', event, { button: button(event), clickCount: event.detail || 1 })
      })
      input.addEventListener('mousemove', event => mouse('mouseMove', event))
      input.addEventListener('mouseleave', () => send({
        type: 'mouseLeave',
        ...this.point,
        modifiers: [],
      }))
      input.addEventListener('wheel', (event) => {
        event.preventDefault()
        const step = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? canvas.clientHeight : 1
        send({
          type: 'mouseWheel',
          ...where(event),
          deltaX: Math.round(event.deltaX * step),
          deltaY: Math.round(event.deltaY * step),
          modifiers: modifiers(event),
        })
      }, { passive: false })
      input.addEventListener('contextmenu', event => event.preventDefault())
      input.addEventListener('blur', () => send({ type: 'blur' }))
      input.addEventListener('keydown', (event) => {
        if (event.isComposing || this.composing)
          return
        event.preventDefault()
        send({ type: 'keyDown', keyCode: event.keyCode, modifiers: modifiers(event) })
      })
      input.addEventListener('keypress', (event) => {
        if (event.isComposing || this.composing || !event.key)
          return
        event.preventDefault()
        send({
          type: 'char',
          keyCode: event.keyCode,
          character: event.key,
          modifiers: modifiers(event),
        })
      })
      input.addEventListener('keyup', (event) => {
        if (event.isComposing || this.composing)
          return
        event.preventDefault()
        send({ type: 'keyUp', keyCode: event.keyCode, modifiers: modifiers(event) })
      })
      input.addEventListener('compositionstart', () => {
        this.composing = true
      })
      input.addEventListener('compositionupdate', (event) => {
        const text = event.data || ''
        send({
          type: 'imeSetComposition',
          text,
          selectionStart: text.length,
          selectionEnd: text.length,
        })
      })
      input.addEventListener('compositionend', (event) => {
        this.composing = false
        send({ type: 'imeCommitText', text: event.data || '' })
        input.value = ''
      })
    }

    render() {
      if (this.destroyed || !this.surface)
        return
      this.animation = requestAnimationFrame(() => this.render())
      if (!this.visible || !this.intersecting || document.hidden)
        return
      const now = performance.now()
      if (this.painting || now - this.paintedAt < 16)
        return
      this.paintedAt = now
      this.painting = true
      invoke('frame', { label: this.label, full: !this.painted }).then((buffer) => {
        this.painting = false
        this.paint(buffer)
      }, () => {
        this.painting = false
      })
    }

    paint(buffer) {
      if (!this.context || !buffer)
        return
      const bytes = buffer instanceof ArrayBuffer ? buffer : buffer.buffer
      if (!(bytes instanceof ArrayBuffer) || bytes.byteLength < 24)
        return
      const header = new Uint32Array(bytes, 0, 6)
      const cursor = header[5]
      if (this.cursor !== cursor) {
        this.cursor = cursor
        this.input.style.cursor = CURSORS[cursor] || 'default'
      }
      const width = header[1]
      const height = header[2]
      const count = header[4]
      if (!width || !height || !count)
        return
      const canvas = this.surface
      if (canvas.width !== width)
        canvas.width = width
      if (canvas.height !== height)
        canvas.height = height
      const rects = new Uint32Array(bytes, 24, count * 4)
      let offset = 24 + count * 16
      for (let index = 0; index < count; index++) {
        const x = rects[index * 4]
        const y = rects[index * 4 + 1]
        const w = rects[index * 4 + 2]
        const h = rects[index * 4 + 3]
        const image = new ImageData(new Uint8ClampedArray(bytes, offset, w * h * 4), w, h)
        offset += w * h * 4
        this.context.putImageData(image, x, y)
      }
      this.painted = true
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

  // Overlay clipping: elements tagged with `data-cef-overlay` are carved out of
  // the native child webviews, so they can cover and receive clicks over them.
  // ponytail: polls the DOM every frame while a webview exists; switch to a
  // MutationObserver + ResizeObserver sweep if this shows up in profiles.
  const OVERLAY_ATTRIBUTE = 'data-cef-overlay'
  // null (not '') so the first tick always syncs: a region left over by the
  // previous page load must be cleared even when the new page has no overlay.
  let overlaySignature = null
  let overlayBusy = false

  function overlayRects() {
    const rects = []
    for (const element of document.querySelectorAll(`[${OVERLAY_ATTRIBUTE}]`)) {
      const rect = element.getBoundingClientRect()
      if (rect.width < 1 || rect.height < 1)
        continue
      const style = getComputedStyle(element)
      if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0')
        continue
      // CSS px: the plugin derives the CSS → device scale from each webview
      // element box versus its native window, so no `devicePixelRatio` here.
      rects.push([rect.left, rect.top, rect.right, rect.bottom])
    }
    return rects
  }

  // Page scrollbars are painted by the host page, not by an element, so a
  // native webview that reaches the viewport edge covers them. Carve the strip
  // they occupy so the scrollbar stays visible and clickable.
  function scrollbarRects() {
    const root = document.documentElement
    const width = window.innerWidth - root.clientWidth
    const height = window.innerHeight - root.clientHeight
    const rects = []
    if (width > 0)
      rects.push([root.clientWidth, 0, root.clientWidth + width, window.innerHeight])
    if (height > 0)
      rects.push([0, root.clientHeight, window.innerWidth, root.clientHeight + height])
    return rects
  }

  function overlayViews() {
    const views = []
    for (const controller of controllers) {
      if (controller.destroyed || !controller.element.isConnected)
        continue
      const box = controller.measure()
      if (box.width < 1 || box.height < 1)
        continue
      views.push([controller.label, [box.x, box.y, box.width, box.height]])
    }
    return views
  }

  function syncOverlayClip() {
    if (overlayBusy)
      return
    const rects = overlayRects().concat(scrollbarRects())
    const views = overlayViews()
    const signature = JSON.stringify([views, rects])
    if (signature === overlaySignature)
      return
    overlaySignature = signature
    overlayBusy = true
    invoke('set_overlay_clip', { views, rects })
      .catch(noop)
      .finally(() => {
        overlayBusy = false
      })
  }

  function overlayTick() {
    requestAnimationFrame(overlayTick)
    if (controllers.size > 0)
      syncOverlayClip()
  }

  function start() {
    subscribe()
    scan(document)
    overlayTick()

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

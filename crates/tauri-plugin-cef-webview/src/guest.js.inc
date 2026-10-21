// Injected into every <webview> guest page before the page scripts run.
//
// The guest reports events that the Tauri/CEF runtime does not surface by
// navigating to a custom scheme. The embedder intercepts the navigation in
// `on_navigation`, forwards it to JavaScript and cancels it, so the page never
// actually leaves. Messages are batched into one navigation every 16ms.
(() => {
  'use strict'

  if (window.top !== window)
    return
  if (location.href === 'about:blank')
    return
  if (window.__cefWebview)
    return

  const BRIDGE = 'cef-webview://event/?d='
  const queue = []
  let ready = false
  let timer = 0

  function post(name, args) {
    queue.push({ n: name, a: args || {} })
    if (ready && !timer)
      timer = setTimeout(flush, 16)
  }

  function flush() {
    timer = 0
    if (!queue.length)
      return

    const batch = []
    let size = 0
    while (queue.length) {
      const length = JSON.stringify(queue[0]).length
      if (batch.length && size + length > 48000)
        break
      batch.push(queue.shift())
      size += length
    }

    try {
      location.href = BRIDGE + encodeURIComponent(JSON.stringify(batch))
    }
    catch {}

    if (queue.length && !timer)
      timer = setTimeout(flush, 16)
  }

  const nativeConsole = {}
  const levels = { debug: 0, log: 1, info: 1, warn: 2, error: 3 }

  function format(value) {
    if (typeof value === 'string')
      return value
    try {
      const json = JSON.stringify(value)
      return json === undefined ? String(value) : json
    }
    catch {
      return String(value)
    }
  }

  for (const method of Object.keys(levels)) {
    nativeConsole[method] = console[method]
    console[method] = function (...args) {
      try {
        post('console-message', {
          level: levels[method],
          message: args.map(format).join(' '),
          line: 0,
          sourceId: location.href,
        })
      }
      catch {}
      return nativeConsole[method].apply(console, args)
    }
  }

  window.addEventListener('error', (event) => {
    post('console-message', {
      level: 3,
      message: event.message || 'Uncaught error',
      line: event.lineno || 0,
      sourceId: event.filename || location.href,
    })
  })

  window.addEventListener('unhandledrejection', (event) => {
    post('console-message', {
      level: 3,
      message: `Unhandled rejection: ${format(event.reason)}`,
      line: 0,
      sourceId: location.href,
    })
  })

  const listeners = new Map()

  const ipc = {
    on(channel, listener) {
      if (!listeners.has(channel))
        listeners.set(channel, new Set())
      listeners.get(channel).add(listener)
      return ipc
    },
    once(channel, listener) {
      const wrap = (...args) => {
        ipc.removeListener(channel, wrap)
        listener(...args)
      }
      return ipc.on(channel, wrap)
    },
    off(channel, listener) {
      return ipc.removeListener(channel, listener)
    },
    removeListener(channel, listener) {
      listeners.get(channel)?.delete(listener)
      return ipc
    },
    removeAllListeners(channel) {
      if (channel === undefined)
        listeners.clear()
      else
        listeners.delete(channel)
      return ipc
    },
    send(channel, ...args) {
      post('ipc-message', { frameId: [0, 0], channel, args })
      return ipc
    },
    sendToHost(channel, ...args) {
      post('ipc-message', { frameId: [0, 0], channel, args })
      return ipc
    },
    invoke() {
      return Promise.reject(new Error('ipcRenderer.invoke is not supported by <webview>'))
    },
  }

  try {
    window.ipcRenderer = ipc
  }
  catch {}

  function deliver(channel, args) {
    const set = listeners.get(channel)
    if (!set)
      return
    for (const listener of [...set]) {
      try {
        listener({ senderId: 0, frameId: 0, senderIsMainFrame: true, channel, args }, ...args)
      }
      catch (error) {
        nativeConsole.error(error)
      }
    }
  }

  let lastFavicons = ''
  let lastTheme = '__cef-webview-unset__'
  let lastTarget = ''

  function faviconUrls() {
    const links = document.querySelectorAll('link[rel~="icon"], link[rel="shortcut icon"]')
    return [...links].map(link => link.href).filter(Boolean)
  }

  function reportFavicons() {
    const favicons = faviconUrls()
    const key = favicons.join('\n')
    if (key === lastFavicons)
      return
    lastFavicons = key
    post('page-favicon-updated', { favicons })
  }

  function reportThemeColor() {
    const meta = document.querySelector('meta[name="theme-color"]')
    const themeColor = meta ? meta.getAttribute('content') || null : null
    if (themeColor === lastTheme)
      return
    lastTheme = themeColor
    post('did-change-theme-color', { themeColor })
  }

  let lastUrl = ''

  function reportInPageNavigation() {
    if (location.href === lastUrl)
      return
    lastUrl = location.href
    post('did-navigate-in-page', { url: location.href, isMainFrame: true })
  }

  for (const method of ['pushState', 'replaceState']) {
    const original = history[method]
    history[method] = function (...args) {
      const result = original.apply(this, args)
      reportInPageNavigation()
      return result
    }
  }

  window.addEventListener('popstate', reportInPageNavigation)
  window.addEventListener('hashchange', reportInPageNavigation)

  document.addEventListener('fullscreenchange', () => {
    post(document.fullscreenElement ? 'enter-html-full-screen' : 'leave-html-full-screen', {})
  })

  let audible = false
  let muted = false

  function mediaElements() {
    return document.querySelectorAll('audio, video')
  }

  function reportAudible() {
    const playing = [...mediaElements()].some(media => !media.paused && !media.ended)
    if (playing === audible)
      return
    audible = playing
    post('state', { audible })
  }

  function applyMuted() {
    for (const media of mediaElements())
      media.muted = muted
  }

  document.addEventListener('play', () => {
    applyMuted()
    post('media-started-playing', {})
    reportAudible()
  }, true)

  document.addEventListener('pause', () => {
    post('media-paused', {})
    reportAudible()
  }, true)

  document.addEventListener('ended', () => {
    post('media-paused', {})
    reportAudible()
  }, true)

  const nativeClose = window.close.bind(window)
  window.close = function () {
    post('close', {})
    try {
      nativeClose()
    }
    catch {}
  }

  document.addEventListener('mouseover', (event) => {
    const anchor = event.target.closest ? event.target.closest('a[href]') : null
    const url = anchor ? anchor.href : ''
    if (url === lastTarget)
      return
    lastTarget = url
    post('update-target-url', { url })
  }, true)

  document.addEventListener('mouseout', (event) => {
    if (event.relatedTarget)
      return
    if (!lastTarget)
      return
    lastTarget = ''
    post('update-target-url', { url: '' })
  }, true)

  function formControlType(element) {
    if (!element || !element.tagName)
      return 'none'
    switch (element.tagName) {
      case 'BUTTON':
        return element.type === 'submit' ? 'submit-button' : element.type === 'reset' ? 'reset-button' : 'button-button'
      case 'SELECT':
        return element.multiple ? 'select-multiple' : 'select-one'
      case 'TEXTAREA':
        return 'text-area'
      case 'OUTPUT':
        return 'output'
      case 'FIELDSET':
        return 'field-set'
      case 'INPUT': {
        const type = element.type || 'text'
        return `input-${type === 'tel' ? 'telephone' : type}`
      }
      default:
        return 'none'
    }
  }

  function mediaFlags(media) {
    if (!media || (media.tagName !== 'VIDEO' && media.tagName !== 'AUDIO')) {
      return {
        inError: false,
        isPaused: true,
        isMuted: false,
        hasAudio: false,
        isLooping: false,
        isControlsVisible: false,
        canToggleControls: false,
        canPrint: false,
        canSave: false,
        canShowPictureInPicture: false,
        isShowingPictureInPicture: false,
        canRotate: false,
        canLoop: false,
      }
    }
    return {
      inError: !!media.error,
      isPaused: media.paused,
      isMuted: media.muted,
      hasAudio: media.mozHasAudio !== false,
      isLooping: media.loop,
      isControlsVisible: media.controls,
      canToggleControls: true,
      canPrint: false,
      canSave: true,
      canShowPictureInPicture: typeof media.requestPictureInPicture === 'function',
      isShowingPictureInPicture: document.pictureInPictureElement === media,
      canRotate: false,
      canLoop: true,
    }
  }

  function editFlags() {
    const can = command => {
      try {
        return document.queryCommandEnabled(command)
      }
      catch {
        return false
      }
    }
    return {
      canUndo: can('undo'),
      canRedo: can('redo'),
      canCut: can('cut'),
      canCopy: can('copy'),
      canPaste: can('paste'),
      canDelete: can('delete'),
      canSelectAll: can('selectAll'),
      canEditRichly: can('insertText'),
    }
  }

  function selectionRect() {
    const selection = getSelection()
    if (!selection || !selection.rangeCount)
      return { x: 0, y: 0, width: 0, height: 0 }
    const rect = selection.getRangeAt(0).getBoundingClientRect()
    return {
      x: rect.x + scrollX,
      y: rect.y + scrollY,
      width: rect.width,
      height: rect.height,
    }
  }

  function suggestedFilename(url) {
    try {
      const path = new URL(url, location.href).pathname
      const name = path.split('/').filter(Boolean).pop()
      return name ? decodeURIComponent(name) : ''
    }
    catch {
      return ''
    }
  }

  document.addEventListener('contextmenu', (event) => {
    const target = event.target
    const anchor = target.closest ? target.closest('a[href]') : null
    const media = target.closest ? target.closest('video, audio, img, canvas') : null
    const editable = !!(target.isContentEditable || target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')
    const selection = getSelection()
    const mediaType = !media
      ? (target.tagName === 'INPUT' && target.type === 'file' ? 'file' : 'none')
      : media.tagName === 'IMG' ? 'image' : media.tagName === 'VIDEO' ? 'video' : media.tagName === 'AUDIO' ? 'audio' : 'canvas'
    const srcUrl = media ? (media.currentSrc || media.src || '') : ''

    post('context-menu', {
      params: {
        x: event.clientX,
        y: event.clientY,
        linkURL: anchor ? anchor.href : '',
        linkText: anchor ? (anchor.innerText || '').trim() : '',
        pageURL: location.href,
        frameURL: location.href,
        srcURL: srcUrl,
        mediaType,
        hasImageContents: mediaType === 'image' ? !!(media.complete && media.naturalWidth > 0) : false,
        isEditable: editable,
        selectionText: selection ? selection.toString() : '',
        titleText: anchor ? anchor.title || '' : '',
        altText: media && media.alt ? media.alt : '',
        suggestedFilename: suggestedFilename(anchor ? anchor.href : srcUrl),
        selectionRect: selectionRect(),
        selectionStartOffset: editable && 'selectionStart' in target ? target.selectionStart || 0 : (selection ? selection.anchorOffset : 0),
        referrerPolicy: { url: document.referrer, policy: document.referrerPolicy || 'default' },
        misspelledWord: '',
        dictionarySuggestions: [],
        frameCharset: document.characterSet,
        formControlType: formControlType(target),
        spellcheckEnabled: !!target.spellcheck,
        menuSourceType: event.detail === 0 ? 'keyboard' : 'mouse',
        mediaFlags: mediaFlags(media),
        editFlags: editFlags(),
      },
    })
  }, true)

  const find = { text: '', caseSensitive: false, matches: 0, ordinal: 0 }

  function countMatches(text, caseSensitive) {
    if (!text)
      return 0
    const needle = caseSensitive ? text : text.toLowerCase()
    const walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const parent = node.parentElement
        if (!parent || /^(SCRIPT|STYLE|NOSCRIPT|TEXTAREA|IFRAME)$/.test(parent.tagName))
          return NodeFilter.FILTER_REJECT
        return NodeFilter.FILTER_ACCEPT
      },
    })
    let count = 0
    let node = walker.nextNode()
    while (node) {
      const haystack = caseSensitive ? node.data : node.data.toLowerCase()
      let index = 0
      while ((index = haystack.indexOf(needle, index)) !== -1) {
        count += 1
        index += needle.length || 1
      }
      node = walker.nextNode()
    }
    return count
  }

  function findInPage(requestId, text, options) {
    options = options || {}
    const caseSensitive = !!options.matchCase
    const forward = options.forward !== false
    if (text !== find.text || caseSensitive !== find.caseSensitive) {
      find.text = text
      find.caseSensitive = caseSensitive
      find.ordinal = 0
      find.matches = countMatches(text, caseSensitive)
    }
    else if (find.matches === 0) {
      find.matches = countMatches(text, caseSensitive)
    }

    let found = false
    if (text) {
      try {
        found = window.find(text, caseSensitive, !forward, true, false, false, false)
      }
      catch {}
    }

    if (found && find.matches)
      find.ordinal = (find.ordinal % find.matches) + 1
    else if (!found)
      find.ordinal = 0

    post('found-in-page', {
      result: {
        requestId,
        activeMatchOrdinal: find.ordinal,
        matches: find.matches,
        selectionArea: selectionRect(),
        finalUpdate: true,
      },
    })
  }

  function stopFindInPage(action) {
    if (action === 'clearSelection') {
      const selection = getSelection()
      if (selection)
        selection.removeAllRanges()
    }
  }

  function setZoom(factor) {
    document.documentElement.style.zoom = String(Number.isFinite(factor) ? factor : 1)
  }

  function download(url) {
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = ''
    anchor.rel = 'noopener'
    anchor.style.display = 'none'
    document.body.appendChild(anchor)
    anchor.click()
    anchor.remove()
  }

  function setMuted(value) {
    muted = !!value
    applyMuted()
  }

  function edit(command, value) {
    switch (command) {
      case 'undo':
        document.execCommand('undo')
        break
      case 'redo':
        document.execCommand('redo')
        break
      case 'cut':
        document.execCommand('cut')
        break
      case 'copy':
        document.execCommand('copy')
        break
      case 'paste':
        document.execCommand('paste')
        break
      case 'delete':
        document.execCommand('delete')
        break
      case 'selectAll':
        document.execCommand('selectAll')
        break
      case 'insertText':
      case 'replace':
      case 'replaceMisspelling':
        document.execCommand('insertText', false, String(value === undefined ? '' : value))
        break
      case 'pasteAndMatchStyle':
        navigator.clipboard.readText().then(text => document.execCommand('insertText', false, text)).catch(() => document.execCommand('paste'))
        break
      case 'unselect': {
        const selection = getSelection()
        if (selection)
          selection.removeAllRanges()
        break
      }
      case 'centerSelection': {
        const rect = selectionRect()
        window.scrollTo({ top: rect.y + scrollY - innerHeight / 2, left: rect.x + scrollX - innerWidth / 2 })
        break
      }
      case 'scrollToTop':
        window.scrollTo(0, 0)
        break
      case 'scrollToBottom':
        window.scrollTo(0, document.body ? document.body.scrollHeight : 0)
        break
      case 'adjustSelection': {
        const element = document.activeElement
        if (value && element && 'selectionStart' in element) {
          if (typeof value.start === 'number')
            element.selectionStart = value.start
          if (typeof value.end === 'number')
            element.selectionEnd = value.end
        }
        break
      }
    }
  }

  function input(payload) {
    if (!payload || typeof payload.type !== 'string')
      return
    const type = payload.type
    const target = document.elementFromPoint(payload.x || 0, payload.y || 0) || document.body
    if (type.startsWith('mouse')) {
      const names = {
        mouseDown: 'mousedown',
        mouseUp: 'mouseup',
        mouseMove: 'mousemove',
        mouseEnter: 'mouseover',
        mouseLeave: 'mouseout',
        contextMenu: 'contextmenu',
      }
      if (type === 'mouseWheel') {
        target.dispatchEvent(new WheelEvent('wheel', {
          clientX: payload.x || 0,
          clientY: payload.y || 0,
          deltaX: payload.deltaX || 0,
          deltaY: payload.deltaY || 0,
          bubbles: true,
          cancelable: true,
          view: window,
        }))
        return
      }
      target.dispatchEvent(new MouseEvent(names[type] || 'mousemove', {
        clientX: payload.x || 0,
        clientY: payload.y || 0,
        button: payload.button || 0,
        buttons: payload.buttons || 0,
        clickCount: payload.clickCount || 1,
        bubbles: true,
        cancelable: true,
        view: window,
      }))
      return
    }

    const keyNames = { keyDown: 'keydown', keyUp: 'keyup', char: 'keypress' }
    target.dispatchEvent(new KeyboardEvent(keyNames[type] || 'keydown', {
      key: payload.key || '',
      code: payload.code || '',
      bubbles: true,
      cancelable: true,
      view: window,
    }))
  }

  function start() {
    if (ready)
      return
    ready = true
    lastUrl = location.href
    post('dom-ready', {})
    post('state', { userAgent: navigator.userAgent, audible })
    reportFavicons()
    reportThemeColor()
    applyMuted()
    if (!timer)
      timer = setTimeout(flush, 0)
  }

  if (document.readyState === 'loading')
    document.addEventListener('DOMContentLoaded', start, { once: true })
  else
    start()

  const headObserver = new MutationObserver(() => {
    reportFavicons()
    reportThemeColor()
  })

  function observeHead() {
    if (document.head) {
      headObserver.observe(document.head, { childList: true, subtree: true, attributes: true, attributeFilter: ['href', 'rel', 'content', 'name'] })
    }
    else {
      setTimeout(observeHead, 50)
    }
  }
  observeHead()

  function executeJavaScript(id, code, userGesture) {
    let result
    try {
      result = (0, eval)(code)
    }
    catch (error) {
      post('execute-javascript', { id, error: String((error && error.stack) || error) })
      return
    }

    Promise.resolve(result).then(
      (value) => {
        let serialized
        try {
          serialized = JSON.stringify(value === undefined ? null : value)
        }
        catch {
          serialized = 'null'
        }
        post('execute-javascript', { id, value: serialized })
      },
      error => post('execute-javascript', { id, error: String((error && error.stack) || error) }),
    )

    if (userGesture && document.activeElement) {
      // The evaluation above already runs in the page; focus keeps edit commands working.
      document.activeElement.dispatchEvent(new Event('focus'))
    }
  }

  window.__cefWebview = {
    deliver,
    executeJavaScript,
    edit,
    findInPage,
    stopFindInPage,
    go: offset => history.go(offset),
    stop: () => window.stop(),
    reloadIgnoringCache: () => location.reload(),
    clearHistory: () => {},
    download,
    input,
    mute: setMuted,
    zoom: setZoom,
    visualZoomLimits: () => {},
    inspect: () => {},
  }
})()

/** Position and size of an embedded webview, in logical pixels. */
export interface WebviewRect {
  x: number
  y: number
  width: number
  height: number
}

/** Resource uploaded with `loadURL`. */
export interface UploadRawData {
  type: 'rawData'
  bytes: Uint8Array
}

export interface UploadFile {
  type: 'file'
  filePath: string
  offset: number
  length: number
  modificationTime: number
}

export interface UploadBlob {
  type: 'blob'
  blobUUID: string
}

export type UploadData = UploadRawData | UploadFile | UploadBlob

export interface LoadURLOptions {
  /**
   * HTTP referrer URL. A string is sent as-is; an object can also set the
   * referrer policy.
   */
  httpReferrer?: string | { url: string, policy?: string }
  /** User agent originating the request. */
  userAgent?: string
  /** Extra headers, separated by `\n`. */
  extraHeaders?: string
  /** Resource body of a POST request. */
  postData?: UploadData[]
  /** Base URL for data URLs loaded with `loadURL`. */
  baseURLForDataURL?: string
}

export interface DownloadURLOptions {
  /** HTTP headers to send with the download request. */
  headers?: Record<string, string>
}

export type MenuSourceType
  = | 'none'
    | 'mouse'
    | 'keyboard'
    | 'touch'
    | 'touchMenu'
    | 'longPress'
    | 'longTap'
    | 'touchHandle'
    | 'stylus'
    | 'adjustSelection'
    | 'adjustSelectionReset'

export interface ContextMenuMediaFlags {
  inError: boolean
  isPaused: boolean
  isMuted: boolean
  hasAudio: boolean
  isLooping: boolean
  isControlsVisible: boolean
  canToggleControls: boolean
  canPrint: boolean
  canSave: boolean
  canShowPictureInPicture: boolean
  isShowingPictureInPicture: boolean
  canRotate: boolean
  canLoop: boolean
}

export interface ContextMenuEditFlags {
  canUndo: boolean
  canRedo: boolean
  canCut: boolean
  canCopy: boolean
  canPaste: boolean
  canDelete: boolean
  canSelectAll: boolean
  canEditRichly: boolean
}

/** Parameters of the `context-menu` event, as provided by Electron. */
export interface ContextMenuParams {
  x: number
  y: number
  linkURL: string
  linkText: string
  pageURL: string
  frameURL: string
  srcURL: string
  mediaType: 'none' | 'image' | 'audio' | 'video' | 'canvas' | 'file' | 'plugin'
  hasImageContents: boolean
  isEditable: boolean
  selectionText: string
  titleText: string
  altText: string
  suggestedFilename: string
  selectionRect: WebviewRect
  selectionStartOffset: number
  referrerPolicy: { url: string, policy: string }
  misspelledWord: string
  dictionarySuggestions: string[]
  frameCharset: string
  formControlType: string
  spellcheckEnabled: boolean
  menuSourceType: MenuSourceType
  mediaFlags: ContextMenuMediaFlags
  editFlags: ContextMenuEditFlags
}

export interface FoundInPageResult {
  requestId: number
  activeMatchOrdinal: number
  matches: number
  selectionArea: WebviewRect
  finalUpdate: boolean
}

export interface FindInPageOptions {
  /** Whether to search forward or backward. Defaults to `true`. */
  forward?: boolean
  /** Whether the operation is the first request or a follow-up. Defaults to `false`. */
  findNext?: boolean
  /** Whether to match case. Defaults to `false`. */
  matchCase?: boolean
}

export type StopFindInPageAction = 'clearSelection' | 'keepSelection' | 'activateSelection'

export type RenderProcessGoneReason
  = | 'clean-exit'
    | 'abnormal-exit'
    | 'killed'
    | 'crashed'
    | 'oom'
    | 'launch-failed'
    | 'integrity-failure'

export interface RenderProcessGoneDetails {
  reason: RenderProcessGoneReason
  exitCode: number
}

export type InputModifier
  = | 'shift'
    | 'control'
    | 'ctrl'
    | 'alt'
    | 'meta'
    | 'command'
    | 'cmd'
    | 'isKeypad'
    | 'isAutoRepeat'
    | 'leftButtonDown'
    | 'middleButtonDown'
    | 'rightButtonDown'
    | 'capsLock'
    | 'numLock'
    | 'left'
    | 'right'

export interface MouseInputEvent {
  type: 'mouseDown' | 'mouseUp' | 'mouseMove' | 'mouseEnter' | 'mouseLeave' | 'contextMenu'
  x: number
  y: number
  button?: 'left' | 'middle' | 'right'
  globalX?: number
  globalY?: number
  movementX?: number
  movementY?: number
  clickCount?: number
  modifiers?: InputModifier[]
}

export interface MouseWheelInputEvent {
  type: 'mouseWheel'
  x: number
  y: number
  deltaX?: number
  deltaY?: number
  wheelTicksX?: number
  wheelTicksY?: number
  accelerationRatioX?: number
  accelerationRatioY?: number
  hasPreciseScrollingDeltas?: boolean
  canScroll?: boolean
  modifiers?: InputModifier[]
}

export interface KeyboardInputEvent {
  type: 'keyDown' | 'keyUp' | 'char'
  keyCode: string
  modifiers?: InputModifier[]
}

export type WebviewInputEvent = MouseInputEvent | MouseWheelInputEvent | KeyboardInputEvent

export interface WebviewPrintOptions {
  silent?: boolean
  printBackground?: boolean
  deviceName?: string
  color?: boolean
  margins?: {
    marginType?: 'default' | 'none' | 'printableArea' | 'custom'
    top?: number
    bottom?: number
    left?: number
    right?: number
  }
  landscape?: boolean
  scaleFactor?: number
  pagesPerSheet?: number
  collate?: boolean
  copies?: number
  pageRanges?: Array<{ from: number, to: number }>
  duplexMode?: 'simplex' | 'shortEdge' | 'longEdge' | 'duplexShortEdge' | 'duplexLongEdge'
  dpi?: { horizontal?: number, vertical?: number }
  header?: string
  footer?: string
  pageSize?: string | { width: number, height: number }
  usePrinterDefaultPageSize?: boolean
}

export interface PrintToPDFOptions {
  landscape?: boolean
  displayHeaderFooter?: boolean
  printBackground?: boolean
  scale?: number
  pageSize?: string | { width: number, height: number }
  margins?: {
    marginType?: 'default' | 'none' | 'printableArea' | 'custom'
    top?: number
    bottom?: number
    left?: number
    right?: number
  }
  pageRanges?: string | Array<{ from: number, to: number }>
  headerTemplate?: string
  footerTemplate?: string
  preferCSSPageSize?: boolean
  generateTaggedPDF?: boolean
  generateDocumentOutline?: boolean
}

/** The image type returned by `capturePage`. */
export interface NativeImage {
  toPNG: () => Uint8Array
  toJPEG: (quality: number) => Uint8Array
  toDataURL: () => string
  getSize: () => { width: number, height: number }
  isEmpty: () => boolean
  resize: (options: {
    width?: number
    height?: number
    quality?: 'good' | 'better' | 'best'
  }) => NativeImage
}

export interface WebviewLoadCommitEvent extends Event {
  url: string
  isMainFrame: boolean
}

export interface WebviewDidFailLoadEvent extends Event {
  errorCode: number
  errorDescription: string
  validatedURL: string
  isMainFrame: boolean
}

export interface WebviewDidFrameFinishLoadEvent extends Event {
  isMainFrame: boolean
}

export interface WebviewPageTitleUpdatedEvent extends Event {
  title: string
  explicitSet: boolean
}

export interface WebviewPageFaviconUpdatedEvent extends Event {
  favicons: string[]
}

export interface WebviewConsoleMessageEvent extends Event {
  /** 0 verbose, 1 info, 2 warning, 3 error. */
  level: number
  message: string
  line: number
  sourceId: string
}

export interface WebviewFoundInPageEvent extends Event {
  result: FoundInPageResult
}

export interface WebviewWillNavigateEvent extends Event {
  url: string
}

export interface WebviewWillFrameNavigateEvent extends Event {
  url: string
  isMainFrame: boolean
  frameProcessId: number
  frameRoutingId: number
}

export interface WebviewDidStartNavigationEvent extends WebviewWillFrameNavigateEvent {
  isInPlace: boolean
}

export interface WebviewDidFrameNavigateEvent extends WebviewWillFrameNavigateEvent {
  httpResponseCode: number
  httpStatusText: string
}

export interface WebviewDidNavigateEvent extends Event {
  url: string
}

export interface WebviewDidNavigateInPageEvent extends Event {
  isMainFrame: boolean
  url: string
}

export interface WebviewIpcMessageEvent extends Event {
  frameId: [number, number]
  channel: string
  args: unknown[]
}

export interface WebviewRenderProcessGoneEvent extends Event {
  details: RenderProcessGoneDetails
}

export interface WebviewDidChangeThemeColorEvent extends Event {
  themeColor: string | null
}

export interface WebviewUpdateTargetUrlEvent extends Event {
  url: string
}

export interface WebviewContextMenuEvent extends Event {
  params: ContextMenuParams
}

export interface WebviewDevtoolsOpenUrlEvent extends Event {
  url: string
}

export interface WebviewDevtoolsSearchQueryEvent extends Event {
  query: string
}

/** DOM events emitted by the `<webview>` element. */
export interface WebviewElementEventMap {
  'load-commit': WebviewLoadCommitEvent
  'did-finish-load': Event
  'did-fail-load': WebviewDidFailLoadEvent
  'did-frame-finish-load': WebviewDidFrameFinishLoadEvent
  'did-start-loading': Event
  'did-stop-loading': Event
  'did-attach': Event
  'dom-ready': Event
  'page-title-updated': WebviewPageTitleUpdatedEvent
  'page-favicon-updated': WebviewPageFaviconUpdatedEvent
  'enter-html-full-screen': Event
  'leave-html-full-screen': Event
  'console-message': WebviewConsoleMessageEvent
  'found-in-page': WebviewFoundInPageEvent
  'will-navigate': WebviewWillNavigateEvent
  'will-frame-navigate': WebviewWillFrameNavigateEvent
  'did-start-navigation': WebviewDidStartNavigationEvent
  'did-redirect-navigation': WebviewWillFrameNavigateEvent
  'did-navigate': WebviewDidNavigateEvent
  'did-frame-navigate': WebviewDidFrameNavigateEvent
  'did-navigate-in-page': WebviewDidNavigateInPageEvent
  'close': Event
  'ipc-message': WebviewIpcMessageEvent
  'render-process-gone': WebviewRenderProcessGoneEvent
  'destroyed': Event
  'media-started-playing': Event
  'media-paused': Event
  'did-change-theme-color': WebviewDidChangeThemeColorEvent
  'update-target-url': WebviewUpdateTargetUrlEvent
  'devtools-open-url': WebviewDevtoolsOpenUrlEvent
  'devtools-search-query': WebviewDevtoolsSearchQueryEvent
  'devtools-opened': Event
  'devtools-closed': Event
  'devtools-focused': Event
  'context-menu': WebviewContextMenuEvent
}

/** Attributes accepted by the `<webview>` element. */
export interface WebviewAttributes {
  /** URL of the page to load. Writing to it navigates the webview. */
  src: string
  /** Enables Node.js integration in the guest. Not supported by CEF. */
  nodeintegration: boolean
  /** Enables Node.js integration in subframes. Not supported by CEF. */
  nodeintegrationinsubframes: boolean
  /** Enables plugins in the guest. Not supported by CEF. */
  plugins: boolean
  /** Script evaluated in the guest before page scripts. Must be a `file:` URL. */
  preload: string
  /** HTTP referrer sent with the initial request. Not supported by CEF. */
  httpreferrer: string
  /** User agent used for the initial navigation. */
  useragent: string
  /** Disables web security. Not supported by CEF. */
  disablewebsecurity: boolean
  /** Session partition. Not supported by CEF. */
  partition: string
  /** Allows `window.open` and `target=_blank` popups. */
  allowpopups: boolean
  /** Comma separated Chromium preferences. Not supported by CEF. */
  webpreferences: string
  /** Comma separated Blink features to enable. Not supported by CEF. */
  enableblinkfeatures: string
  /** Comma separated Blink features to disable. Not supported by CEF. */
  disableblinkfeatures: string
}

/**
 * Public API installed on every `<webview>` element.
 *
 * The element keeps its place in the DOM and is used as the layout box of the
 * native child webview.
 */
export interface WebviewElement extends HTMLElement, WebviewAttributes {
  /** Native label of the embedded webview. Matches the element `id` when set. */
  readonly label: string
  /** Resolves once the native webview has been created. */
  readonly ready: Promise<WebviewElement>

  addEventListener: {
    <K extends keyof WebviewElementEventMap>(
      type: K,
      listener: (this: WebviewElement, event: WebviewElementEventMap[K]) => unknown,
      options?: boolean | AddEventListenerOptions,
    ): void
    (
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | AddEventListenerOptions,
    ): void
  }
  removeEventListener: {
    <K extends keyof WebviewElementEventMap>(
      type: K,
      listener: (this: WebviewElement, event: WebviewElementEventMap[K]) => unknown,
      options?: boolean | EventListenerOptions,
    ): void
    (
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | EventListenerOptions,
    ): void
  }

  /** Loads `url` in the webview. */
  loadURL: (url: string, options?: LoadURLOptions) => Promise<void>
  /** Downloads `url` without navigating. */
  downloadURL: (url: string, options?: DownloadURLOptions) => void
  /** Returns the current URL. */
  getURL: () => string
  /** Returns the page title. */
  getTitle: () => string
  /** Returns whether the webview is loading a page. */
  isLoading: () => boolean
  /** Returns whether the main frame is loading. */
  isLoadingMainFrame: () => boolean
  /** Returns whether the webview is waiting for a response. */
  isWaitingForResponse: () => boolean
  /** Stops loading. */
  stop: () => void
  /** Reloads the current page. */
  reload: () => void
  /** Reloads the current page, ignoring the cache. */
  reloadIgnoringCache: () => void
  /** Returns whether the webview can navigate back. */
  canGoBack: () => boolean
  /** Returns whether the webview can navigate forward. */
  canGoForward: () => boolean
  /** Returns whether the webview can navigate to the given offset. */
  canGoToOffset: (offset: number) => boolean
  /** Clears the navigation history. */
  clearHistory: () => void
  /** Navigates back. */
  goBack: () => void
  /** Navigates forward. */
  goForward: () => void
  /** Navigates to the given history index. */
  goToIndex: (index: number) => void
  /** Navigates to the given history offset. */
  goToOffset: (offset: number) => void
  /** Returns whether the renderer process has crashed. */
  isCrashed: () => boolean
  /** Sets the user agent for later navigations. */
  setUserAgent: (userAgent: string) => void
  /** Returns the user agent of the guest. */
  getUserAgent: () => string
  /** Injects a stylesheet and resolves with the key used to remove it. */
  insertCSS: (css: string) => Promise<string>
  /** Removes a stylesheet previously added with `insertCSS`. */
  removeInsertedCSS: (key: string) => Promise<void>
  /** Evaluates JavaScript in the guest and resolves with its result. */
  executeJavaScript: <T = unknown>(code: string, userGesture?: boolean) => Promise<T>
  /** Opens the guest devtools. */
  openDevTools: () => void
  /** Closes the guest devtools. */
  closeDevTools: () => void
  /** Returns whether the guest devtools are open. */
  isDevToolsOpened: () => boolean
  /** Returns whether the guest devtools are focused. */
  isDevToolsFocused: () => boolean
  /** Opens devtools and inspects the element at the given coordinates. */
  inspectElement: (x: number, y: number) => void
  /** Opens devtools for a shared worker. */
  inspectSharedWorker: () => void
  /** Opens devtools for a service worker. */
  inspectServiceWorker: () => void
  /** Mutes or unmutes audio. */
  setAudioMuted: (muted: boolean) => void
  /** Returns whether audio is muted. */
  isAudioMuted: () => boolean
  /** Returns whether the guest is currently playing audio. */
  isCurrentlyAudible: () => boolean

  undo: () => void
  redo: () => void
  cut: () => void
  copy: () => void
  centerSelection: () => void
  paste: () => void
  pasteAndMatchStyle: () => void
  delete: () => void
  selectAll: () => void
  unselect: () => void
  scrollToTop: () => void
  scrollToBottom: () => void
  adjustSelection: (options: { start?: number, end?: number }) => void
  replace: (text: string) => void
  replaceMisspelling: (text: string) => void
  insertText: (text: string) => void

  /** Starts a find request and returns its id. */
  findInPage: (text: string, options?: FindInPageOptions) => number
  /** Stops the current find request. */
  stopFindInPage: (action: StopFindInPageAction) => void
  /** Prints the guest page. */
  print: (options?: WebviewPrintOptions) => Promise<void>
  /** Prints the guest page to PDF. */
  printToPDF: (options?: PrintToPDFOptions) => Promise<Uint8Array>
  /** Captures a snapshot of the guest page. */
  capturePage: (rect?: WebviewRect) => Promise<NativeImage>
  /** Sends an IPC message to the guest. */
  send: (channel: string, ...args: unknown[]) => Promise<void>
  /** Sends an IPC message to the guest frame. */
  sendToFrame: (frameId: [number, number], channel: string, ...args: unknown[]) => Promise<void>
  /** Sends an input event to the guest. */
  sendInputEvent: (event: WebviewInputEvent) => Promise<void>
  /** Sets the zoom factor, where 1.0 is the original size. */
  setZoomFactor: (factor: number) => void
  /** Sets the zoom level, where `scale^level` is the zoom factor. */
  setZoomLevel: (level: number, scale?: number) => void
  /** Returns the zoom factor. */
  getZoomFactor: () => number
  /** Returns the zoom level. */
  getZoomLevel: () => number
  /** Sets the minimum and maximum visual zoom levels. */
  setVisualZoomLevelLimits: (min: number, max: number) => Promise<void>
  /** Shows the definition for the selected word (macOS). */
  showDefinitionForSelection: () => void
  /** Returns the id of the guest web contents. */
  getWebContentsId: () => number

  /** @deprecated Use `loadURL`. */
  navigate: (url: string) => Promise<void>
  /** @deprecated Use `executeJavaScript`. */
  eval: (js: string) => Promise<void>
  /** @deprecated Use `openDevTools`. */
  openDevtools: () => Promise<void>
}

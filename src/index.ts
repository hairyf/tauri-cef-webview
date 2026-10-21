export type * from './types'

declare global {
  interface HTMLElementTagNameMap {
    webview: import('./types').WebviewElement
  }
}

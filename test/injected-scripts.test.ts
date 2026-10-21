import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const root = fileURLToPath(new URL('..', import.meta.url))

const scripts = [
  'crates/tauri-plugin-cef-webview/src/element.js.inc',
  'crates/tauri-plugin-cef-webview/src/guest.js.inc',
]

describe('injected scripts', () => {
  it.each(scripts)('%s parses and has no imports', (path) => {
    const source = readFileSync(join(root, path), 'utf8')
    expect(source).not.toMatch(/^\s*import\s/m)

    const file = join(mkdtempSync(join(tmpdir(), 'cef-webview-')), 'script.js')
    writeFileSync(file, source)
    expect(() => execFileSync(process.execPath, ['--check', file])).not.toThrow()
  })
})

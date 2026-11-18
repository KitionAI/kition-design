/**
 * Library files (team libraries from files; `bindings/wasm/src/libraries.rs`): a design names the
 * `.kdesign` files whose published libraries it uses by workspace paths relative to its own folder
 * (`query('library.files')`). The engine reads no files, so the editor reads each one with the
 * host (`host/read`) and lends the bytes to the engine (`lendLibrary`), and watches the file's
 * version (`host/stamp`): when it changes (someone published the library again and saved it), the
 * new bytes are lent and the Assets panel shows "Library updates available"; when it is gone, the
 * engine says so (`library.list` errors, `design.missing-component` in the check). Upstream re-reads
 * library files the same way, on a timer while the Assets panel is open.
 */
import { useEffect, useRef } from 'react'
import type { DesignKit } from '../kit/context'
import { useDesignKit, useQuery } from '../kit/hooks'

type Files = { files: Array<{ id: string; name: string; path: string }> }

/** How often the library files' versions are asked while the editor is shown. */
export const WATCH_MS = 2000
/** A check still running after this long no longer holds the next one back. */
const STUCK_MS = 15000

/** The folder of the document the editor opened, in the workspace (`/`-separated, empty at the root). */
export function documentFolder(designKit: DesignKit) {
  const path = designKit.editor.host.init.document.path
  return path.split('/').filter(Boolean).slice(0, -1).join('/')
}

/** The workspace path a document in `folder` names with `path` (relative to it, `/` from the root); null above the root. */
export function workspacePath(folder: string, path: string): string | null {
  const parts: string[] = []
  for (const part of [...(path.startsWith('/') ? [] : folder.split('/')), ...path.split('/')]) {
    if (!part || part === '.') continue
    if (part === '..') {
      if (!parts.length) return null
      parts.pop()
    } else parts.push(part)
  }
  return parts.length ? parts.join('/') : null
}

/** Workspace path `to` as a document in `folder` names it (`../libraries/Icons.kdesign`), as Film names its clips. */
export function relativePath(folder: string, to: string) {
  const parts = (path: string) => path.replace(/\\/g, '/').split('/').filter(Boolean)
  const from = parts(folder)
  const target = parts(to)
  let shared = 0
  while (shared < from.length && shared < target.length - 1 && from[shared] === target[shared]) shared += 1
  return [...from.slice(shared).map(() => '..'), ...target.slice(shared)].join('/')
}

/**
 * Reads the library file the document names `path` with the host and lends it to the engine, unless
 * `known` says this version was lent already. Answers whether what the engine reads changed.
 */
export async function lendLibraryFile(designKit: DesignKit, path: string, known?: Map<string, string>) {
  const { host } = designKit.editor
  const at = workspacePath(documentFolder(designKit), path)
  let version: string | null = null
  if (at) {
    try {
      version = await host.files.stamp(at)
    } catch {
      version = null
    }
  }
  const seen = version ?? 'missing'
  if (known?.get(path) === seen) return false
  let changed: boolean
  if (at && version !== null) {
    try {
      const { bytes } = await host.files.read(at)
      changed = await designKit.invoke<boolean>('lendLibrary', path, new Uint8Array(bytes))
    } catch {
      changed = await designKit.invoke<boolean>('lendLibrary', path, null)
    }
  } else {
    changed = await designKit.invoke<boolean>('lendLibrary', path, null)
  }
  known?.set(path, seen)
  return changed
}

/** Keeps the engine's library files current: lends each one the design names, again whenever it changes. */
export function LibraryFiles(): null {
  const designKit = useDesignKit()
  const latest = useRef(designKit)
  latest.current = designKit
  const listing = useQuery<Files>('library.files')
  const paths = (listing?.files ?? []).map((file) => file.path).join('\n')
  // The versions lent to this document's engine (a reopened document starts again).
  const known = useRef(new Map<string, string>())
  const doc = designKit.doc
  useEffect(() => {
    known.current = new Map()
  }, [doc])
  useEffect(() => {
    if (!paths || !doc) return undefined
    let live = true
    // When the running check started (0: none); a check that never ends does not stop the next.
    let running = 0
    // The engine reads something new that the panels have not asked about yet.
    let stale = false
    const check = async () => {
      if ((running && Date.now() - running < STUCK_MS) || document.visibilityState === 'hidden') return
      const started = Date.now()
      running = started
      try {
        for (const path of paths.split('\n')) {
          if (!live) return
          stale = (await lendLibraryFile(latest.current, path, known.current).catch(() => false)) || stale
        }
        if (stale && live) {
          await latest.current.refresh()
          stale = false
        }
      } catch {
        // Asked again on the next check.
      } finally {
        if (running === started) running = 0
      }
    }
    void check()
    const timer = window.setInterval(() => void check(), WATCH_MS)
    const focus = () => void check()
    window.addEventListener('focus', focus)
    document.addEventListener('visibilitychange', focus)
    return () => {
      live = false
      window.clearInterval(timer)
      window.removeEventListener('focus', focus)
      document.removeEventListener('visibilitychange', focus)
    }
  }, [doc, paths])
  return null
}

/**
 * The editor's shared state, for every panel: the engine (`run`, `query`, `invoke`), the
 * inspected document, the selection, interactions (one undo step per drag), the canvas and its
 * view, and the shell's UI state (tabs, panels). Panels read it with the hooks in `hooks.ts`.
 */
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from 'react'
import type { KitCanvasHandle, KitCanvasView, KitEditor, KitEngine, KitTools } from '@kition/sdk/editor'
import type { DesignStrings, Values } from '../strings'
import { showToast } from '../ui/Toast'
import type { DesignInspect, LeftTab, RightTab } from './types'

/** The engine extras that make a run of commands one undo step (when the binding has them). */
export const INTERACTION_EXTRAS = { begin: 'beginInteraction', commit: 'commitInteraction', cancel: 'cancelInteraction' } as const

/** A drag or scrub: `update` runs commands live, `commit` makes them one undo step. */
export type Interaction = {
  /** Runs `command` now, or, while an earlier update is still running, after it (only the newest waits). */
  update: (command: string, params: Record<string, unknown>) => void
  /** Ends the interaction as one undo step (when the engine supports it). */
  commit: () => Promise<void>
  /** Ends it, undoing what it did (when the engine supports it; else its steps stay). */
  cancel: () => Promise<void>
}

export type ShellUi = {
  rightTab: RightTab
  setRightTab: (tab: RightTab) => void
  leftTab: LeftTab
  setLeftTab: (tab: LeftTab) => void
  leftOpen: boolean
  setLeftOpen: (open: boolean) => void
  /** View > Hide UI (Cmd+\ upstream; kit-editor's Focus mode here). */
  uiHidden: boolean
  /** Shows or hides the UI: enters or leaves kit-editor's Focus display (`useFrameDisplay`). */
  toggleFocus: () => void
  /** The main menu (logo button) is open. */
  mainMenu: boolean
  setMainMenu: (open: boolean) => void
  /** The Actions palette (Cmd+K) is open. */
  actions: boolean
  setActions: (open: boolean) => void
}

export type DesignKit = {
  editor: KitEditor
  doc: KitEngine | null
  inspect: DesignInspect
  /** Moves with every change of the document, the selection, or an interaction's end: queries re-run on it. */
  revision: string
  strings: DesignStrings
  t: (key: string, values?: Values) => string
  readOnly: boolean
  /** Runs an engine command; answers its JSON result, or null when it failed (a toast says why). */
  run: <R = unknown>(command: string, params?: Record<string, unknown>) => Promise<R | null>
  /** A panel query (`query(name, params)` in bindings/wasm/src/query.rs), cached per revision. */
  query: <R = unknown>(name: string, params?: Record<string, unknown>) => Promise<R>
  /** One of the binding's own extras (`chrome`, `exportNode`, `thumbnail`, …). */
  invoke: <R = unknown>(name: string, ...args: unknown[]) => Promise<R>
  /** Starts a drag or scrub labelled `label` (the undo step's name). */
  interaction: (label: string) => Interaction
  /** Whether the binding has the interaction extras (null until first asked). */
  interactions: boolean | null
  /**
   * Moves the canvas where View command `command` takes it (`view.zoomIn`, `view.zoomFit`,
   * `view.zoomSelection`, `view.nextFrame`, … — the engine's `viewTarget` extra).
   */
  viewTo: (command: string, params?: Record<string, unknown>) => Promise<void>
  /** Re-reads the engine's state after an `invoke` that edited, and re-runs the queries. */
  refresh: () => Promise<void>
  canvas: RefObject<KitCanvasHandle | null>
  tools: KitTools
  ui: ShellUi
  toast: (text: string) => void
}

const DesignKitContext = createContext<DesignKit | null>(null)
const ViewContext = createContext<KitCanvasView | null>(null)

export function useDesignKit(): DesignKit {
  const designKit = useContext(DesignKitContext)
  if (!designKit) throw new Error('useDesignKit outside DesignKitProvider')
  return designKit
}

/** The canvas view (zoom, the document point at its top-left, its size); changes on every pan and zoom. */
export function useView(): KitCanvasView | null {
  return useContext(ViewContext)
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error))

type Props = {
  editor: KitEditor
  inspect: DesignInspect
  strings: DesignStrings
  canvas: RefObject<KitCanvasHandle | null>
  view: KitCanvasView | null
  tools: KitTools
  ui: ShellUi
  children: ReactNode
}

export function DesignKitProvider({ editor, inspect, strings, canvas, view, tools, ui, children }: Props) {
  const { session } = editor
  const doc = session.doc
  const [local, setLocal] = useState(0)
  const revision = `${session.change.revision}.${local}`
  const latestDoc = useRef(doc)
  latestDoc.current = doc

  // Answers per revision: the same query asked twice in one revision runs once.
  const cache = useRef<{ revision: string; answers: Map<string, Promise<unknown>> }>({ revision, answers: new Map() })
  if (cache.current.revision !== revision) cache.current = { revision, answers: new Map() }

  const invoke = useCallback(async <R,>(name: string, ...args: unknown[]): Promise<R> => {
    const open = latestDoc.current
    if (!open) throw new Error('no document is open')
    return open.invoke<R>(name, ...args)
  }, [])

  const query = useCallback(
    <R,>(name: string, params: Record<string, unknown> = {}): Promise<R> => {
      const key = `${name}\u0000${JSON.stringify(params)}`
      const answers = cache.current.answers
      let answer = answers.get(key)
      if (!answer) {
        answer = invoke<string>('query', name, JSON.stringify(params)).then((text) => JSON.parse(text) as unknown)
        answers.set(key, answer)
        // A failed answer is asked again next time.
        answer.catch(() => answers.delete(key))
      }
      return answer as Promise<R>
    },
    [invoke],
  )

  const run = useCallback(
    async <R,>(command: string, params: Record<string, unknown> = {}): Promise<R | null> => {
      const answer = await editor.run(command, params)
      if (answer == null) return null
      try {
        return JSON.parse(answer) as R
      } catch {
        return answer as unknown as R
      }
    },
    [editor],
  )

  const refresh = useCallback(async () => {
    const open = latestDoc.current
    if (!open) return
    // A settled tool call brings kit-editor's `inspect()` up to date (an `invoke` and an
    // interaction's commit run no command through the session, so it would keep the last one:
    // the Undo label of the step before a drag).
    await session.tool('tool', [], true).catch(() => open.refreshInfo().catch(() => undefined))
    await canvas.current?.retryRender()
    setLocal((value) => value + 1)
  }, [canvas, session])

  // Interactions: begin/commit extras when the binding has them, else plain commands.
  const [interactions, setInteractions] = useState<boolean | null>(null)
  const supported = useRef<boolean | null>(null)
  // Every interaction's begin, updates and end run in order on one chain, so one that begins while
  // another's commit is on its way waits for it (the engine has one open interaction at a time).
  const chain = useRef<Promise<void>>(Promise.resolve())
  // The interaction this editor has open in the engine (its token), if any.
  const open = useRef<object | null>(null)
  const interaction = useCallback(
    (label: string): Interaction => {
      const token = {}
      // This interaction opened the engine's: its edits are one step; else they run as plain commands.
      let began = false
      const enqueue = (step: () => Promise<void>) => {
        const next = chain.current.then(step)
        chain.current = next.catch(() => undefined)
        return next
      }
      enqueue(async () => {
        if (supported.current === false) return
        try {
          // `false`: an interaction is open already (one that never ended): end it first, or this
          // one's edits would join it under its label.
          if ((await invoke<boolean>(INTERACTION_EXTRAS.begin, label)) === false) {
            await invoke(INTERACTION_EXTRAS.commit)
            await invoke(INTERACTION_EXTRAS.begin, label)
          }
          began = true
          open.current = token
          supported.current = true
        } catch (error) {
          // Only a binding without the extras turns grouping off; any other failure is this
          // interaction's alone.
          if (/has no method/.test(message(error))) supported.current = false
          else showToast(message(error))
        }
        setInteractions(supported.current)
      })
      let latest: { command: string; params: Record<string, unknown> } | null = null
      let queued = false
      let ended: Promise<void> | null = null
      const finish = (how: 'commit' | 'cancel') => {
        if (ended) return ended
        ended = enqueue(async () => {
          // A later interaction ended this one when it began: nothing is left open to end.
          if (!began || open.current !== token) return
          open.current = null
          try {
            await invoke(how === 'commit' ? INTERACTION_EXTRAS.commit : INTERACTION_EXTRAS.cancel)
          } catch (error) {
            showToast(message(error))
          }
          await refresh()
        })
        return ended
      }
      return {
        update: (command, params) => {
          if (ended) return
          latest = { command, params }
          if (queued) return
          queued = true
          void enqueue(async () => {
            queued = false
            const next = latest
            latest = null
            // Ended by a later interaction's begin: its edits must not join that one.
            if (!next || (began && open.current !== token)) return
            await session.run(next.command, next.params).then(() => undefined, (error: unknown) => showToast(message(error)))
          })
        },
        commit: () => finish('commit'),
        cancel: () => finish('cancel'),
      }
    },
    [invoke, refresh, session],
  )

  const viewTo = useCallback(
    async (command: string, params: Record<string, unknown> = {}) => {
      const handle = canvas.current
      const now = handle?.view()
      if (!handle || !now) return
      try {
        const answer = JSON.parse(await invoke<string>('viewTarget', command, JSON.stringify({ zoom: now.zoom, x: now.x, y: now.y, width: now.width, height: now.height, ...params }))) as { zoom: number; x: number; y: number; selection: number[] | null }
        handle.zoomTo(answer.zoom)
        handle.panTo(answer.x, answer.y)
        if (answer.selection) await run('select.set', { ids: answer.selection })
      } catch (error) {
        showToast(message(error))
      }
    },
    [canvas, invoke, run],
  )

  const value = useMemo<DesignKit>(
    () => ({
      editor,
      doc,
      inspect,
      revision,
      strings,
      t: strings.t,
      readOnly: editor.host.readOnly === true,
      run,
      query,
      invoke,
      interaction,
      interactions,
      viewTo,
      refresh,
      canvas,
      tools,
      ui,
      toast: showToast,
    }),
    [canvas, doc, editor, inspect, interaction, interactions, invoke, query, refresh, revision, run, strings, tools, ui, viewTo],
  )
  // Development and the flow tests reach the editor's state from the page (`window.designKit`).
  if (import.meta.env.DEV) (window as unknown as { designKit?: DesignKit }).designKit = value
  return (
    <DesignKitContext.Provider value={value}>
      <ViewContext.Provider value={view}>{children}</ViewContext.Provider>
    </DesignKitContext.Provider>
  )
}

/** The answer to `query(name, params)`, asked again whenever the document or the selection changes; null until the first answer. */
export function useQuery<R>(name: string, params: Record<string, unknown> = {}, options: { skip?: boolean } = {}): R | null {
  const { query, revision, doc } = useDesignKit()
  const key = JSON.stringify(params)
  const [answer, setAnswer] = useState<{ id: string; value: R } | null>(null)
  const id = `${name}\u0000${key}`
  const skip = options.skip === true || !doc
  useEffect(() => {
    if (skip) return undefined
    let live = true
    query<R>(name, JSON.parse(key) as Record<string, unknown>).then(
      (value) => {
        if (live) setAnswer({ id, value })
      },
      () => {
        if (live) setAnswer(null)
      },
    )
    return () => {
      live = false
    }
  }, [id, key, name, query, revision, skip])
  // The last answer stays while the next is on its way (no flicker between revisions).
  return answer && answer.id === id && !skip ? answer.value : null
}

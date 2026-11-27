/**
 * The DesignKit editor: kit-editor's session, commands and frame (Menus display: the engine's menus in
 * the frame's top bar, with Undo, Redo and Save), and inside it the upstream editor's layout
 * (`Shell`). See web/README.md.
 */
import { useCallback, useLayoutEffect, useMemo, useRef, useState, type RefObject } from 'react'
import {
  EditorFrame,
  HAND_TOOL,
  KitEditorProvider,
  useEngineTools,
  useFrameDisplay,
  useKitEditorState,
  useKitSession,
  type DisplayModeState,
  type HostConnection,
  type KitCanvasHandle,
  type KitCanvasView,
} from '@kition/sdk/editor'
import { engineManifest } from './engine'
import { DESIGN_OVERLAY_STYLE, drawDesignOverlay } from './canvas/engineOverlays'
import { DesignKitProvider, useDesignKit, type ShellUi } from './kit/context'
import { dispatchFromAnswer, dispatchUiRequests } from './kit/requests'
import { EMPTY_INSPECT, describe, type LeftTab, type RightTab } from './kit/types'
import { Shell } from './shell/Shell'
import { shellKeyDown } from './shell/keys'
import { LibraryFiles } from './left/libraryFiles'
import { interceptCommand } from './menus/invoke'
import { DEFAULT_MENU } from './menus/catalog'
import { NOT_OFFERED, keymapKeyDown, useKeymap } from './menus/keymap'
import { useMenuTabs } from './menus/model'
import type { DesignStrings } from './strings'
import { showToast } from './ui/Toast'
import { runInText } from './kit/textEditing'
import { onDevicePixels, placedHandleRef, placedView } from './canvas/view'

/** The hand is the host's (kit-editor's pan); every other tool runs in the engine. */
const HOST_TOOLS = { hand: HAND_TOOL }

const message = (error: unknown) => (error instanceof Error ? error.message : String(error))

export function App({ host, strings }: { host: HostConnection; strings: DesignStrings }) {
  const manifest = useMemo(engineManifest, [])
  const session = useKitSession(host, manifest, describe)
  // Failures show as the upstream editor's toast: the engine's own message.
  const report = useCallback((_title: string, error: unknown) => showToast(message(error)), [])
  const base = useKitEditorState(session, host, strings.t, 'design', strings.phrase, undefined, { report })
  // Commands only ask for UI (dialogs, panels): their answers' requests go to whoever shows it.
  // Whether a text is being edited on the canvas (the Edit commands then act on its characters).
  const editingText = useRef(false)
  const editor = useMemo(
    () => ({
      ...base,
      run: async (id: string, params?: Record<string, unknown>) => {
        // Commands the editor shows a dialog for, or runs itself (the menus package).
        if (interceptCommand(id, params)) return null
        if (editingText.current) {
          const inText = await runInText(id, params, base.run)
          if (inText !== undefined) return inText
        }
        const answer = await base.run(id, params)
        dispatchFromAnswer(answer)
        return answer
      },
    }),
    [base],
  )
  const tools = useEngineTools(session.tool, session.doc != null, session.change, {
    initial: 'move',
    hostTools: HOST_TOOLS,
    onDialog: (name, params) => dispatchUiRequests([{ kind: name, data: params }]),
    // The engine's overlays in the upstream editor's style (canvas/engineOverlays.ts).
    overlayStyle: DESIGN_OVERLAY_STYLE,
    drawOverlay: drawDesignOverlay,
  })
  editingText.current = session.state?.editing === 'text' || tools.wantsText
  // The canvas handle, its view the one on the screen (see placedHandleRef).
  const canvas = useMemo(placedHandleRef, [])
  const [view, setView] = useState<KitCanvasView | null>(null)
  const [rightTab, setRightTab] = useState<RightTab>('design')
  const [leftTab, setLeftTab] = useState<LeftTab>('file')
  const [leftOpen, setLeftOpen] = useState(true)
  const [mainMenu, setMainMenu] = useState(false)
  const [actions, setActions] = useState(false)
  // The frame's display (inside EditorFrame, below), for View > Show/Hide UI.
  const display = useRef<DisplayModeState | null>(null)
  const toggleFocus = useCallback(() => display.current?.toggleFocus(), [])
  const ui = useMemo<ShellUi>(
    () => ({ rightTab, setRightTab, leftTab, setLeftTab, leftOpen, setLeftOpen, uiHidden: false, toggleFocus, mainMenu, setMainMenu, actions, setActions }),
    [actions, leftOpen, leftTab, mainMenu, rightTab, toggleFocus],
  )
  const inspect = session.state ?? EMPTY_INSPECT
  const engine = session.doc
  const shownView = useRef<{ engine: unknown; view: KitCanvasView } | null>(null)
  const snapping = useRef(false)
  const onView = useCallback(
    (asked: KitCanvasView) => {
      // On whole device pixels (M5): the canvas reports a new view before the screen shows it, so
      // the view moved here is the one painted. Once: a pan bound may put it back off them.
      const snapped = snapping.current ? null : onDevicePixels(asked, window.devicePixelRatio || 1)
      if (snapped && canvas.current) {
        snapping.current = true
        try {
          canvas.current.panTo(snapped.x, snapped.y)
        } finally {
          snapping.current = false
        }
        return
      }
      // The view as the screen shows it, which the pointer goes through (see placedView).
      const next = placedView(asked, window.devicePixelRatio || 1)
      // The canvas reports its view after every frame it draws: only a moved one (or a new
      // document's) is news.
      const was = shownView.current
      if (was && was.engine === engine && was.view.x === next.x && was.view.y === next.y && was.view.zoom === next.zoom && was.view.width === next.width && was.view.height === next.height) return
      shownView.current = { engine, view: next }
      setView(next)
      // The engine's view (for View commands, chrome and hit tests); never an edit.
      void engine?.invoke('setViewport', next.x, next.y, next.width, next.height, next.zoom).catch(() => undefined)
    },
    [canvas, engine],
  )
  // The same elements after every engine answer (this component renders again for each, with
  // kit-editor's new session): the editor's own components render again only for what they read
  // (DesignKitProvider's context, `useTools`, `useView`), not for every hover and drag sample (R2).
  const content = useMemo(
    () => (
      <>
        <Frame canvas={canvas} display={display} onView={onView} />
        <LibraryFiles />
      </>
    ),
    [onView],
  )

  return (
    <KitEditorProvider editor={editor} tools={tools}>
      <DesignKitProvider editor={editor} inspect={inspect} strings={strings} canvas={canvas} view={view} tools={tools} ui={ui}>
        {content}
      </DesignKitProvider>
    </KitEditorProvider>
  )
}

/** Commands whose keys the frame does not bind (keymap.ts `NOT_OFFERED`: the keyboard marquee). */
const notOffered = (id: string) => NOT_OFFERED.has(id)

function Frame({ canvas, display, onView }: { canvas: React.RefObject<KitCanvasHandle | null>; display: RefObject<DisplayModeState | null>; onView: (view: KitCanvasView) => void }) {
  const designKit = useDesignKit()
  // The top bar's menus are the main menu's (the menus package), so both agree.
  const tabs = useMenuTabs()
  // The frame's own keys, before its command shortcuts: the shell's, then every binding's second keys.
  const keymap = useKeymap()
  return (
    <EditorFrame tabs={tabs} defaultTab={DEFAULT_MENU} canvas={canvas} statusBar={false} hiddenCommands={notOffered} onKeyDown={(event) => shellKeyDown(designKit, event) || keymapKeyDown(designKit, keymap, event)}>
      <FrameDisplay into={display} />
      <Shell onView={onView} />
    </EditorFrame>
  )
}

/** Hands the frame's display state (Focus) to the editor's own commands, which run outside the frame. */
function FrameDisplay({ into }: { into: RefObject<DisplayModeState | null> }) {
  const display = useFrameDisplay()
  useLayoutEffect(() => {
    into.current = display
  }, [display, into])
  return null
}

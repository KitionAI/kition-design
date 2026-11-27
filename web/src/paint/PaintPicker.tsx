/**
 * The colour picker on an engine paint (`PickerTarget` upstream): reads the paint of the target
 * (a layer's fill or stroke, an effect's colour, a selection colour, a text range, the page's
 * background, a colour variable's value, a colour style's paint), writes edits with the command
 * the target takes (a drag is one undo step, "Color"), remembers chosen colours in Recent, and
 * runs the eyedropper on the canvas (`samplePixel`). The canvas handles for gradients, crops and
 * progressive blurs are in `PaintHandles.tsx`.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useDesignKit, useQuery, useView, type Interaction } from '../kit/hooks'
import { useCanvasOverlay } from '../canvas/overlays'
import { type Color, fromHex, readColor, toCssHex, withAlpha } from './color'
import { ColorPicker, type ChangePhase, type ImagePreset, type PatternSource } from './ColorPicker'
import { useSelectionData } from './data'
import { type Paint, editColor, paintJson, readPaints, solid } from './paint'
import { PaintHandles } from './PaintHandles'
import { pickFile, toBase64 } from '../menus/invoke'
import { closePicker, type PickerTarget, rememberRecent, setEyedropper, setStop, usePaintState } from './store'

/** The images Choose image takes (the host's picker offers these). */
const IMAGE_FILES = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'tif', 'tiff']

type Variables = { variables: Array<{ id: string; name: string; resolvedType: string; values: Record<string, { type: string; value: unknown }> }> }
type Styles = Array<{ id: string; name: string; type?: string; value?: unknown; paints?: unknown }> | { styles?: unknown }

/** The paint a target shows, from what the panels already read. */
function useTargetPaint(target: PickerTarget | null): { paint: Paint | null; effect: Record<string, unknown> | null; stale: boolean } {
  const designKit = useDesignKit()
  const data = useSelectionData()
  const firstId = target && 'ids' in target ? target.ids[0] : target?.kind === 'selectionColor' ? target.targets[0]?.id : target?.kind === 'textRange' ? target.id : undefined
  const fromSelection = data?.nodes.find((n) => n.id === firstId)
  const node = useQuery<Record<string, unknown>>('node', { id: firstId ?? 0 }, { skip: firstId === undefined || fromSelection !== undefined })
  const vars = useQuery<Variables>('variables', {}, { skip: target?.kind !== 'variable' })
  const styles = useQuery<Styles>('styles', {}, { skip: target?.kind !== 'style' })
  const n = (fromSelection ?? node) as Record<string, unknown> | null | undefined
  if (!target) return { paint: null, effect: null, stale: false }
  const selection = designKit.inspect.selection
  const sameIds = (ids: number[]) => ids.length === selection.length && ids.every((id, i) => id === selection[i])
  switch (target.kind) {
    case 'paint': {
      if (!sameIds(target.ids)) return { paint: null, effect: null, stale: true }
      const list = readPaints(n?.[target.fills ? 'fills' : 'strokes'])
      return { paint: list[target.index] ?? null, effect: null, stale: n !== undefined && n !== null && !list[target.index] }
    }
    case 'effect': {
      if (!sameIds(target.ids)) return { paint: null, effect: null, stale: true }
      const effects = (n?.effects as Array<Record<string, unknown>> | undefined) ?? []
      const e = effects[target.index]
      const c = e ? readColor(target.secondary ? e.secondary : e.color) : null
      return { paint: c ? solid(withAlpha(c, 1), c.a) : null, effect: e ?? null, stale: n !== undefined && n !== null && !c }
    }
    case 'selectionColor': {
      if (!sameIds(target.selection)) return { paint: null, effect: null, stale: true }
      const slot = target.targets[0]
      const list = readPaints(n?.[slot?.fills ? 'fills' : 'strokes'])
      return { paint: slot ? (list[slot.index] ?? null) : null, effect: null, stale: false }
    }
    case 'textRange': {
      if (!selection.includes(target.id)) return { paint: null, effect: null, stale: true }
      const run = data?.text?.id === target.id ? data.text.runs.find((r) => r.start <= target.start && r.end > target.start) : undefined
      const fills = run?.style.fills ? readPaints(run.style.fills) : readPaints(n?.fills)
      return { paint: fills[0] ?? null, effect: null, stale: false }
    }
    case 'page': {
      const c = fromHex(designKit.inspect.background)
      return { paint: c ? solid(c) : null, effect: null, stale: false }
    }
    case 'variable': {
      const v = vars?.variables.find((x) => x.id === target.id)?.values[target.mode]
      const c = v ? readColor(v.value) : null
      return { paint: c ? solid(withAlpha(c, 1), c.a) : null, effect: null, stale: false }
    }
    case 'style': {
      const list = Array.isArray(styles) ? styles : Array.isArray((styles as { styles?: unknown } | null)?.styles) ? ((styles as { styles: Styles }).styles as unknown[]) : []
      const style = (list as Array<Record<string, unknown>>).find((s) => s.id === target.id)
      const paints = readPaints(style?.paints ?? style?.value)
      return { paint: paints[target.index] ?? null, effect: null, stale: false }
    }
  }
}

/** The command and params that write `paint` to `target`. */
function writeCommand(target: PickerTarget, paint: Paint, effect: Record<string, unknown> | null): [string, Record<string, unknown>] | null {
  const json = paintJson(paint)
  const c = editColor(paint)
  switch (target.kind) {
    case 'paint':
      return [target.fills ? 'fill.set' : 'stroke.set', { ids: target.ids, index: target.index, paint: json }]
    case 'effect': {
      if (!effect) return null
      const key = target.secondary ? 'secondary' : 'color'
      return ['effect.set', { ids: target.ids, index: target.index, effect: { ...effect, [key]: { r: c.r, g: c.g, b: c.b, a: c.a } } }]
    }
    case 'selectionColor':
      return ['select.setPaint', { targets: target.targets, paint: json }]
    case 'textRange':
      return ['text.style', { id: target.id, start: target.start, end: target.end, textFills: [json] }]
    case 'page':
      return ['page.setBackground', { color: toCssHex(withAlpha(c, 1)) }]
    case 'variable':
      return ['variable.set', { variable: target.id, mode: target.mode, value: toCssHex(c) }]
    case 'style':
      return null
  }
}

/**
 * Writes edits to a target: `live` ones inside one interaction, `done` ones commit it (or run
 * alone), `cancel` undoes it.
 */
export function usePaintWriter(label: string) {
  const { run, interaction, t } = useDesignKit()
  const open = useRef<Interaction | null>(null)
  const write = useCallback(
    (command: string, params: Record<string, unknown>, phase: ChangePhase, color: Color | null) => {
      if (phase === 'live') {
        if (!open.current) open.current = interaction(t(label))
        open.current.update(command, params)
        return
      }
      const active = open.current
      open.current = null
      if (phase === 'cancel') {
        void active?.cancel()
        return
      }
      if (active) {
        active.update(command, params)
        void active.commit()
      } else void run(command, params)
      if (color) rememberRecent(color)
    },
    [interaction, label, run, t],
  )
  // A drag cut short by unmounting still ends as one step.
  useEffect(
    () => () => {
      void open.current?.commit()
      open.current = null
    },
    [],
  )
  return write
}

/** The colour picker on the open target (the paint package's store), with its canvas helpers. */
export function TargetColorPicker() {
  const designKit = useDesignKit()
  const { t, run, inspect } = designKit
  const state = usePaintState()
  const target = state.picker
  const { paint, effect, stale } = useTargetPaint(target)
  const [shown, setShown] = useState<Paint | null>(null)
  const write = usePaintWriter('Color')
  const writeHandles = usePaintWriter('Edit gradient')
  const docColors = useQuery<{ colors: Array<{ color: string; css: string; alpha: number }> }>('documentColors', {}, { skip: !target })
  const imageHash = paint?.type === 'image' && typeof paint.image?.image === 'string' ? paint.image.image : null
  const imageInfo = useQuery<{ profile: string | null }>('image.info', { hash: imageHash ?? '' }, { skip: imageHash === null })
  const recentEngine = useQuery<{ colors: string[] }>('recentColors', {}, { skip: !target })
  const presets = useQuery<ImagePreset[] | { presets: ImagePreset[] }>('imagePresets', {}, { skip: paint?.type !== 'image' })
  const layers = useQuery<{ rows: Array<{ id: number; name: string }> }>('layers', {}, { skip: paint?.type !== 'pattern' && target?.kind !== 'paint' })

  // The picker closes with its target (the selection moved on, the paint went away); a paint
  // just added may take a moment to reach the panels' data.
  useEffect(() => {
    if (!target || !stale) return undefined
    const timer = window.setTimeout(closePicker, 600)
    return () => window.clearTimeout(timer)
  }, [stale, target])

  // The picker shows what it wrote (during a drag, and after a change until the document's paint
  // comes back with it): a field typed in straight after an edit starts from the new colour, not
  // the one the panels read before it.
  const settled = useRef(false)
  const paintKey = paint ? JSON.stringify(paint) : ''
  useEffect(() => {
    if (settled.current) {
      settled.current = false
      setShown(null)
    }
  }, [paintKey])
  // Another paint in the picker shows its own colour.
  const targetKey = target ? JSON.stringify(target) : ''
  useEffect(() => {
    settled.current = false
    setShown(null)
  }, [targetKey])
  const current = shown ?? paint

  const onChange = useCallback(
    (next: Paint, phase: ChangePhase) => {
      if (!target) return
      settled.current = phase === 'done'
      // A cancelled scrub shows the document's paint again once the interaction is undone.
      setShown(phase === 'cancel' ? null : next)
      if (target.kind === 'style') {
        // A colour style's paint: the whole list is rewritten.
        void designKit.query<Styles>('styles').then((styles) => {
          const list = (Array.isArray(styles) ? styles : []) as Array<Record<string, unknown>>
          const style = list.find((s) => s.id === target.id)
          const paints = readPaints(style?.paints ?? style?.value).map(paintJson)
          paints[target.index] = paintJson(next)
          write('style.set', { style: target.id, value: paints }, phase, next.type === 'solid' ? editColor(next) : null)
        })
        return
      }
      const command = writeCommand(target, next, effect)
      if (command) write(command[0], command[1], phase, next.type === 'solid' ? editColor(next) : null)
    },
    [effect, designKit, target, write],
  )

  const recent = useMemo(() => {
    const out: Color[] = [...state.recent]
    for (const hex of recentEngine?.colors ?? []) {
      const c = fromHex(hex)
      if (c && !out.some((x) => toCssHex(x) === toCssHex(c))) out.push(c)
    }
    return out.slice(0, 16)
  }, [recentEngine, state.recent])
  // "On this page" with each colour's own alpha (`css` is `#RRGGBBAA` when translucent).
  const documentColors = useMemo(() => (docColors?.colors ?? []).map((c) => fromHex(c.css ?? c.color)).filter((c): c is Color => c !== null), [docColors])
  // The image's own colour profile (upstream: unknown is taken as sRGB; an unnamed one is "Embedded profile").
  const imageProfile = imageInfo ? (imageInfo.profile === null ? t('Unknown — treated as sRGB') : imageInfo.profile === '' ? t('Embedded profile') : imageInfo.profile) : undefined

  const kinds = target !== null && !['effect', 'page', 'variable'].includes(target.kind)
  const painted = target?.kind === 'paint' ? target.ids : target?.kind === 'selectionColor' ? target.targets.map((s) => s.id) : target?.kind === 'textRange' ? [target.id] : []
  const sources: PatternSource[] = (layers?.rows ?? []).filter((r) => !painted.includes(r.id)).slice(0, 64).map((r) => ({ id: r.id, name: r.name }))
  const presetList = Array.isArray(presets) ? presets : (presets?.presets ?? [])

  // Choosing Image: the file the host's picker gives becomes this fill, in place (`image.place
  // { index }`, one undo step).
  const chooseImage = () => {
    if (target?.kind !== 'paint' || !target.fills || target.ids.length !== 1) {
      designKit.toast(t('Choose image'))
      return
    }
    const id = target.ids[0]
    const index = target.index
    void pickFile(designKit, 'image', IMAGE_FILES).then((picked) => {
      if (picked) void run('image.place', { target: id, data: toBase64(picked.bytes), index })
    })
  }

  const imagePreset = (action: { apply: string } | { save: string; filters: Record<string, number> } | { remove: string }) => {
    if ('save' in action) void run('image.savePreset', { name: action.save, filters: action.filters })
    else if ('remove' in action) void run('image.deletePreset', { name: action.remove })
    else if (target?.kind === 'paint') void run('image.adjust', { ids: target.ids, target: target.fills ? 'fill' : 'stroke', index: target.index, preset: action.apply })
  }

  if (!target || !current) return null
  return (
    <>
      <ColorPicker
        paint={current}
        onChange={onChange}
        onClose={closePicker}
        kinds={kinds}
        title={t('Custom')}
        documentColors={documentColors}
        recent={recent}
        stop={state.stop}
        onStop={setStop}
        onEyedropper={() => setEyedropper(!state.eyedropper)}
        displayP3={inspect.colorProfile === 'displayP3'}
        onChooseImage={chooseImage}
        patternSources={sources}
        imagePresets={presetList}
        onImagePreset={imagePreset}
        imageProfile={imageProfile}
      />
      {target.kind === 'paint' && target.ids.length === 1 && (current.type === 'gradient' || (current.type === 'image' && current.image?.scale_mode === 'crop')) ? (
        <PaintHandles
          id={target.ids[0]}
          index={target.index}
          fills={target.fills}
          paint={current}
          stop={state.stop}
          onChange={(next, phase) => {
            setShown(phase === 'live' ? next : null)
            const command = writeCommand(target, next, effect)
            if (command) writeHandles(command[0], command[1], phase, null)
          }}
        />
      ) : null}
      {state.eyedropper ? <Eyedropper paint={current} stop={state.stop} onPick={(next) => onChange(next, 'done')} /> : null}
    </>
  )
}

/** The eyedropper: a click on the canvas samples the colour shown there into the picker's paint. */
function Eyedropper({ paint, stop, onPick }: { paint: Paint; stop: number; onPick: (paint: Paint) => void }) {
  const { invoke } = useDesignKit()
  const view = useView()
  const latest = useRef({ paint, stop, onPick, view })
  latest.current = { paint, stop, onPick, view }
  const element = useMemo(
    () => (
      <div
        className="design-eyedropper"
        data-testid="design-eyedropper"
        onPointerDown={async (event) => {
          if (event.button !== 0) return
          event.preventDefault()
          event.stopPropagation()
          const { view: v } = latest.current
          if (!v) return
          const r = event.currentTarget.getBoundingClientRect()
          const x = v.x + (event.clientX - r.left) / v.zoom
          const y = v.y + (event.clientY - r.top) / v.zoom
          let hex = ''
          try {
            hex = await invoke<string>('samplePixel', x, y, v.zoom)
          } catch {
            setEyedropper(false)
            return
          }
          const c = fromHex(hex)
          setEyedropper(false)
          if (!c) return
          const { paint: p, stop: s, onPick: pick } = latest.current
          if (p.type === 'solid') pick({ ...p, color: withAlpha(c, 1), opacity: 1 })
          else if (p.type === 'gradient' && p.gradient) pick({ ...p, gradient: { ...p.gradient, stops: p.gradient.stops.map((x, i) => (i === s ? { ...x, color: withAlpha(c, x.color.a) } : x)) } })
        }}
      />
    ),
    [invoke],
  )
  useCanvasOverlay('paint.eyedropper', element)
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation()
        setEyedropper(false)
      }
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [])
  return null
}

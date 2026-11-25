/**
 * The style picker (2.2, `widgets/style_picker.rs`): the dark popover the Design panel's "Styles"
 * buttons open (`stylePicker.open` with `{ ids, slot: fill|stroke|effect|grid|text, y? }`). It lists
 * the local styles of the slot by folder, as a list or a grid, with a search; a click applies one,
 * a hovered text style previews on the canvas, "+" creates one from the selection, and the editor
 * changes a style's name, description, properties and variable bindings. Each enabled library's
 * published styles follow under its name (`styles.libraries`; "This file" heads the local ones
 * then), read-only: a click copies one in and applies it (`library.applyStyle`), and the linked
 * copies this file holds show as their library's row, never among the local styles.
 */
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { dispatchUiRequests } from '../kit/requests'
import { Icon, type IconName } from '../icons/Icon'
import { Menu, type MenuEntry, NumberField } from '../ui'
import { pressedOutside } from '../ui/Popover'
import { ColorPicker } from '../paint/ColorPicker'
import { usePaintWriter } from '../paint/PaintPicker'
import { paintJson, readPaints, type Paint } from '../paint/paint'
import { devRefuses } from './dev'
import { RenameInput } from './fields'
import { DragGhost } from './Pages'
import { paintCss } from './swatch'
import { openVariablePicker, VariableRow } from './VariablePicker'
import { useVariables, type Variables } from './variables'

export type StyleSlot = 'fill' | 'stroke' | 'effect' | 'grid' | 'text'
export type StylePickerRequest = { ids: number[]; slot: StyleSlot; y?: number; edit?: string }

/** Opens the style picker for `slot` of layers `ids` (`edit`: straight into a style's editor). */
export function openStylePicker(request: StylePickerRequest) {
  dispatchUiRequests([{ kind: 'stylePicker.open', data: request }])
}

type Measure = { unit?: string; value?: number } | string | null | undefined
type TextDef = { fontFamily?: string; fontStyle?: string; fontSize?: number; lineHeight?: Measure; letterSpacing?: Measure } & Record<string, unknown>
type StyleJson = { id: string; name: string; description?: string; type: 'paint' | 'text' | 'effect' | 'grid'; paints?: unknown[]; effects?: Array<{ type?: string }>; grids?: Array<{ kind?: string; pattern?: string; size?: number; count?: number }>; bound?: Record<string, string> }
type TextStyleJson = { id: string; style: TextDef }
/** A library's published style as `styles.libraries` answers it (text styles carry their properties). */
type LibraryStyleJson = StyleJson & { text?: TextDef }
type VariableLibraries = { copies: { collections: string[]; variables: string[] }; libraries: Array<{ id: string; name: string } & Variables> }
type StyleLibraries = { copies: Record<string, { library: string; style: string }>; libraries: Array<{ id: string; name: string; error: string | null; styles: LibraryStyleJson[] }> }

const W = 240
const TITLES: Record<StyleSlot, string> = { fill: 'Color styles', stroke: 'Color styles', effect: 'Effect styles', grid: 'Grid styles', text: 'Text styles' }
const CREATE: Record<StyleSlot, string> = { fill: 'paint', stroke: 'stroke', effect: 'effect', grid: 'grid', text: 'text' }
const BASE: Record<StyleSlot, string> = { fill: 'Color', stroke: 'Color', effect: 'Effect', grid: 'Grid', text: 'Text' }
const HOLDS: Record<StyleSlot, StyleJson['type']> = { fill: 'paint', stroke: 'paint', effect: 'effect', grid: 'grid', text: 'text' }
const ICON: Record<StyleSlot, IconName> = { fill: 'Effects', stroke: 'Effects', effect: 'Effects', grid: 'Grid', text: 'Text' }

/** The list or grid view, shared by every picker (upstream's `style_grid`). */
let gridView = false
/** Folders folded away (upstream's `style_folds`). */
const folds = new Set<string>()

const folderOf = (name: string) => {
  const at = name.lastIndexOf('/')
  return at < 0 ? '' : name.slice(0, at).trim()
}
const leafOf = (name: string) => name.slice(name.lastIndexOf('/') + 1).trim()

/** A line height or letter spacing as upstream labels it ("Auto", "140%", "24"). */
export function measureLabel(m: Measure): string {
  if (m == null) return 'Auto'
  if (typeof m === 'string') return m
  if (m.unit === 'auto' || m.value == null) return 'Auto'
  const v = Math.round(m.value * 100) / 100
  return m.unit === 'percent' ? `${v}%` : String(v)
}

/** A window point under element `el` (where a picker opened from it goes). */
const below = (el: Element) => {
  const r = el.getBoundingClientRect()
  return { x: r.left, y: r.bottom }
}

const textMeta = (def: TextDef) => `${Math.round((def.fontSize ?? 0) * 100) / 100}/${measureLabel(def.lineHeight)}`

/** The selection's link in `slot`: a style id, null for none, 'mixed' when they differ. */
function linkOf(nodes: Array<{ styles?: Record<string, string | null> }>, slot: StyleSlot): string | null | 'mixed' {
  const links = nodes.map((n) => n.styles?.[slot] ?? null)
  if (!links.length) return null
  return links.every((l) => l === links[0]) ? links[0] : 'mixed'
}

/** A text style's sample, drawn by the engine in its own font (`textPreview`), as an image. */
function TextSample({ id, library, text, size, maxWidth, maxHeight }: { id: string; library?: string; text: string; size: number; maxWidth: number; maxHeight: number }) {
  const { invoke, revision } = useDesignKit()
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    let made: string | null = null
    const scale = Math.min(Math.max(window.devicePixelRatio || 1, 1), 3)
    void invoke<Uint8Array>('textPreview', JSON.stringify({ styleId: id, ...(library ? { library } : {}), size, text, color: '#ffffff', scale, maxWidth, maxHeight }))
      .then((bytes) => {
        if (!live) return
        made = URL.createObjectURL(new Blob([bytes as BlobPart], { type: 'image/png' }))
        setUrl(made)
      })
      .catch(() => live && setUrl(null))
    return () => {
      live = false
      if (made) URL.revokeObjectURL(made)
    }
  }, [id, invoke, library, maxHeight, maxWidth, revision, size, text])
  if (!url) return <span className="design-style-sample-text">{text}</span>
  // Drawn at the display's scale: shown at its size in CSS pixels.
  const scale = Math.min(Math.max(window.devicePixelRatio || 1, 1), 3)
  return (
    <img
      className="design-style-sample"
      src={url}
      alt={text}
      style={{ maxWidth, maxHeight }}
      draggable={false}
      onLoad={(event) => {
        const img = event.currentTarget
        img.style.width = `${img.naturalWidth / scale}px`
        img.style.height = `${img.naturalHeight / scale}px`
      }}
    />
  )
}

export function StylePicker({ request, onClose }: { request: StylePickerRequest; onClose: () => void }) {
  const designKit = useDesignKit()
  const { t, run, inspect } = designKit
  const { ids, slot } = request
  const all = useQuery<StyleJson[]>('styles')
  const textStyles = useQuery<{ styles: TextStyleJson[] }>('textStyles', {}, { skip: slot !== 'text' })
  const libraryAnswer = useQuery<StyleLibraries>('styles.libraries', { type: HOLDS[slot] })
  const variableLibraries = useQuery<VariableLibraries>('variables.libraries', {}, { skip: !(slot === 'fill' || slot === 'stroke') })
  const selection = useQuery<{ nodes?: Array<{ styles?: Record<string, string | null> }> }>('selection')
  const vars = useVariables()
  const [query, setQuery] = useState('')
  const [edit, setEdit] = useState<{ id: string; name: string; description: string; focus: boolean } | null>(null)
  const [, setTick] = useState(0)
  const [paintEdit, setPaintEdit] = useState<{ style: string; index: number } | null>(null)
  const panel = useRef<HTMLDivElement>(null)
  const previewing = useRef(false)
  const styles = useMemo(() => (Array.isArray(all) ? all : []), [all])
  const copies = libraryAnswer?.copies ?? {}
  const defs = useMemo(() => new Map((textStyles?.styles ?? []).map((s) => [s.id, s.style])), [textStyles])
  const link = linkOf(selection?.nodes ?? [], slot)
  const paints = slot === 'fill' || slot === 'stroke'
  const hasGrid = paints || slot === 'text'
  const grid = hasGrid && gridView

  // Where it opens: left of the right panel, at the pointer's height.
  const [at, setAt] = useState<{ left: number; top: number }>({ left: 0, top: 48 })
  useLayoutEffect(() => {
    const right = document.querySelector('.design-right')?.getBoundingClientRect()
    const rightWidth = right ? window.innerWidth - right.left : 240
    const y = (request.y ?? 152) - 12
    setAt({ left: window.innerWidth - rightWidth - W - 32 - (paintEdit ? 260 : 0), top: Math.max(48, Math.min(y, Math.max(48, window.innerHeight - 460))) })
  }, [request.y, paintEdit])

  const commitEdit = () => {
    if (!edit) return
    const style = styles.find((s) => s.id === edit.id)
    if (!style) return
    const params: Record<string, unknown> = { style: edit.id }
    const name = edit.name.trim()
    if (name && name !== style.name) params.name = name
    if (edit.description !== (style.description ?? '')) params.description = edit.description
    if (Object.keys(params).length > 1) void run('style.set', params)
  }
  const endPreview = () => {
    if (!previewing.current) return
    previewing.current = false
    void run('text.endPreview', {})
  }
  const close = () => {
    endPreview()
    commitEdit()
    onClose()
  }
  const latestClose = useRef(close)
  latestClose.current = close

  // Closes on a press outside, Escape, and a change of the selection.
  useEffect(() => {
    const down = (event: PointerEvent) => {
      if (panel.current && pressedOutside(panel.current, event.target, null)) latestClose.current()
    }
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || document.querySelector('.design-layer:not(.design-style-picker) ~ .design-layer, [data-testid="design-variable-picker"]')) return
      event.preventDefault()
      event.stopPropagation()
      latestClose.current()
    }
    document.addEventListener('pointerdown', down, true)
    window.addEventListener('keydown', key, true)
    return () => {
      document.removeEventListener('pointerdown', down, true)
      window.removeEventListener('keydown', key, true)
    }
  }, [])
  const selectionKey = inspect.selection.join(',')
  const opened = useRef(selectionKey)
  useEffect(() => {
    if (selectionKey !== opened.current) latestClose.current()
  }, [selectionKey])

  useEffect(() => {
    if (request.edit) {
      const style = styles.find((s) => s.id === request.edit)
      if (style) setEdit({ id: style.id, name: style.name, description: style.description ?? '', focus: false })
    }
    // Once, when the styles arrive.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request.edit, all === null])

  const apply = (style: StyleJson) => {
    endPreview()
    if (devRefuses(designKit, 'style.apply')) return
    void run('style.apply', { ids, style: style.id, ...(paints ? { target: slot } : {}) })
    close()
  }
  /** A library's style: copied in and applied (upstream `Key::apply`). */
  const applyLibrary = (library: string, style: StyleJson) => {
    endPreview()
    if (devRefuses(designKit, 'library.applyStyle')) return
    void run('library.applyStyle', { library, style: style.id, ids, ...(paints ? { target: slot } : {}) })
    close()
  }
  const preview = (style: StyleJson | null, library?: string) => {
    if (slot !== 'text') return
    if (!style) {
      endPreview()
      return
    }
    previewing.current = true
    if (library) void run('text.preview', { command: 'library.applyStyle', params: { library, style: style.id, ids } })
    else void run('text.preview', { command: 'style.apply', params: { ids, style: style.id } })
  }
  const create = async () => {
    if (devRefuses(designKit, 'style.add')) return
    let n = 1
    while (styles.some((s) => s.name === `${BASE[slot]} ${n}`)) n += 1
    const name = `${BASE[slot]} ${n}`
    const made = await run<{ id?: string }>('style.add', { type: CREATE[slot], name, ...(ids[0] != null ? { from: ids[0] } : {}) })
    if (made?.id) {
      setQuery('')
      setEdit({ id: made.id, name, description: '', focus: true })
    }
  }

  const q = query.trim().toLowerCase()
  const fits = (s: StyleJson) => s.type === HOLDS[slot] && (q === '' || s.name.toLowerCase().includes(q))
  // This file's own styles: its linked library copies show under their library.
  const offered = styles.filter((s) => !copies[s.id] && fits(s))
  const libraries = (libraryAnswer?.libraries ?? []).map((l) => ({ ...l, styles: l.styles.filter(fits) })).filter((l) => l.styles.length > 0)
  const byFolder = (list: StyleJson[]) => {
    const folders: Array<[string, StyleJson[]]> = []
    for (const s of list) {
      const f = folderOf(s.name)
      const found = folders.find(([g]) => g === f)
      if (found) found[1].push(s)
      else folders.push([f, [s]])
    }
    return folders.sort((a, b) => Number(a[0] !== '') - Number(b[0] !== ''))
  }
  /** Whether the selection's link in this slot is `style` (a library row: this file's copy of it). */
  const isApplied = (style: StyleJson, library?: string) => {
    if (link === null || link === 'mixed') return false
    if (!library) return link === style.id
    const copy = copies[link]
    return copy?.library === library && copy.style === style.id
  }
  // This file's colour variables (not its linked library copies), then each library's
  // (upstream `variable_ui::groups`: "{library} · {collection}", a click copies one in and binds it).
  const linkedVars = new Set(variableLibraries?.copies.variables ?? [])
  const colorVar = (v: Variables['variables'][number]) => v.resolvedType === 'color' && (q === '' || v.name.toLowerCase().includes(q))
  const colorVars = paints ? vars.variables.filter((v) => !linkedVars.has(v.id) && colorVar(v)) : []
  const libraryColorVars = paints
    ? (variableLibraries?.libraries ?? []).flatMap((l) => l.collections.map((c) => ({ library: l, collection: c, list: l.variables.filter((v) => v.collection === c.id && colorVar(v)) }))).filter((g) => g.list.length > 0)
    : []

  const iconButton = (icon: IconName, label: string, onClick: () => void, testId?: string) => (
    <button type="button" className="design-dark-icon" aria-label={label} data-tip={label} onClick={onClick} data-testid={testId}>
      <Icon name={icon} size={14} />
    </button>
  )

  const row = (s: LibraryStyleJson, library?: string) => {
    const def = library ? s.text : defs.get(s.id)
    const applied = isApplied(s, library)
    // Library styles are read-only here (upstream: no editor, no menu).
    const pick = () => (library ? applyLibrary(library, s) : apply(s))
    const tip = s.description ? s.description : def ? `${s.name}\n${def.fontFamily ?? ''} ${def.fontStyle ?? ''} · ${textMeta(def)}` : s.bound && Object.keys(s.bound).length ? `${s.name}\n${t('Uses variables: each layer shows them in its own mode')}` : undefined
    return (
      <div
        key={s.id}
        className="design-style-row"
        role="button"
        tabIndex={0}
        data-text={def ? true : undefined}
        data-tip={tip}
        data-testid={library ? `design-library-style-${library}-${s.id}` : `design-style-${s.id}`}
        data-library={library}
        data-applied={applied || undefined}
        onClick={pick}
        onKeyDown={(event) => event.key === 'Enter' && pick()}
        onContextMenu={(event) => {
          event.preventDefault()
          if (library) return
          endPreview()
          setEdit({ id: s.id, name: s.name, description: s.description ?? '', focus: false })
        }}
        onPointerEnter={() => preview(s, library)}
        onPointerLeave={() => preview(null)}
      >
        {def ? (
          <>
            <span className="design-style-row-sample">
              <TextSample id={s.id} library={library} text={leafOf(s.name)} size={Math.min(Math.max(def.fontSize ?? 12, 6), 16)} maxWidth={150} maxHeight={28} />
            </span>
            <span className="design-style-row-meta">{textMeta(def)}</span>
          </>
        ) : (
          <>
            {s.type === 'paint' ? <span className="design-left-swatch design-style-row-lead" style={{ background: paintCss(s.paints?.[s.paints.length - 1]) }} /> : <span className="design-style-row-lead"><Icon name={ICON[slot]} size={14} /></span>}
            <span className="design-style-row-name">{leafOf(s.name)}</span>
          </>
        )}
        {s.bound && Object.keys(s.bound).length ? (
          <span className="design-style-row-var">
            <Icon name="Variable" size={11} />
          </span>
        ) : null}
        {library ? null : (
          <button
            type="button"
            className="design-dark-icon design-style-row-edit"
            aria-label={t('Edit style')}
            data-tip={t('Edit style')}
            onClick={(event) => {
              event.stopPropagation()
              endPreview()
              setEdit({ id: s.id, name: s.name, description: s.description ?? '', focus: false })
            }}
            data-testid={`design-style-edit-${s.id}`}
          >
            <Icon name="Adjust" size={14} />
          </button>
        )}
        {applied ? (
          <span className="design-style-row-check">
            <Icon name="Check" size={12} />
          </span>
        ) : null}
      </div>
    )
  }

  const tiles = (list: LibraryStyleJson[], library?: string) => (
    <div className="design-style-grid">
      {list.map((s) => {
        const def = library ? s.text : defs.get(s.id)
        return (
          <button
            key={s.id}
            type="button"
            className="design-style-tile"
            data-text={def ? true : undefined}
            data-applied={isApplied(s, library) || undefined}
            data-library={library}
            data-tip={def ? `${s.name}\n${def.fontFamily ?? ''} ${def.fontStyle ?? ''} · ${textMeta(def)}` : s.name}
            onClick={() => (library ? applyLibrary(library, s) : apply(s))}
            onContextMenu={(event) => {
              event.preventDefault()
              if (!library) setEdit({ id: s.id, name: s.name, description: s.description ?? '', focus: false })
            }}
            onPointerEnter={() => preview(s, library)}
            onPointerLeave={() => preview(null)}
            data-testid={library ? `design-library-style-${library}-${s.id}` : `design-style-${s.id}`}
            style={def ? undefined : { background: paintCss(s.paints?.[s.paints.length - 1]) }}
          >
            {def ? <TextSample id={s.id} library={library} text="Ag" size={Math.min(def.fontSize ?? 12, 24)} maxWidth={48} maxHeight={40} /> : null}
          </button>
        )
      })}
    </div>
  )

  /** One source's styles by folder (`library`: an enabled library's id; its folds are its own). */
  const section = (list: LibraryStyleJson[], library?: string) =>
    byFolder(list).map(([f, items]) => {
      const key = library ? `${library}/${f}` : f
      const closed = q === '' && folds.has(key)
      return (
        <div key={key || '*'}>
          {f ? (
            <button
              type="button"
              className="design-style-folder"
              onClick={() => {
                if (q) return
                if (!folds.delete(key)) folds.add(key)
                setTick((n) => n + 1)
              }}
            >
              <Icon name={closed ? 'ChevronRight' : 'Chevron'} size={10} />
              <span>{f.split('/').map((p) => p.trim()).join(' / ')}</span>
            </button>
          ) : null}
          {closed ? null : grid ? tiles(items, library) : items.map((s) => row(s, library))}
        </div>
      )
    })

  let body: ReactNode
  const editing = edit ? styles.find((s) => s.id === edit.id) : null
  if (edit && editing) {
    body = (
      <StyleEditor
        style={editing}
        def={defs.get(editing.id) ?? null}
        name={edit.name}
        description={edit.description}
        focus={edit.focus}
        onName={(name) => setEdit({ ...edit, name, focus: false })}
        onDescription={(description) => setEdit({ ...edit, description, focus: false })}
        onBack={() => {
          commitEdit()
          setEdit(null)
        }}
        onPaint={(index) => {
          commitEdit()
          setPaintEdit({ style: editing.id, index })
        }}
        hasSelection={ids.length > 0}
        firstId={ids[0]}
        onDeleted={() => setEdit(null)}
      />
    )
  } else {
    body = (
      <>
        <div className="design-dark-header">
          <span className="design-dark-popover-title">{t(TITLES[slot])}</span>
          <span style={{ flex: 1 }} />
          {hasGrid
            ? iconButton(grid ? 'ViewList' : 'ViewGrid', t(grid ? 'Show as list' : 'Show as grid'), () => {
                gridView = !gridView
                setTick((n) => n + 1)
                // Upstream's search keeps the keyboard.
                document.querySelector<HTMLInputElement>('[data-testid="design-style-search"]')?.focus()
              }, 'design-style-view')
            : null}
          {link !== null ? iconButton('Detach', t('Detach style'), () => void run('style.detach', { ids, which: slot }), 'design-style-detach') : null}
          {iconButton('Plus', t(slot === 'text' ? 'Create style from the selected text' : 'Create style'), () => void create(), 'design-style-create')}
        </div>
        <input className="design-dark-search" value={query} placeholder={t('Search')} aria-label={t('Search')} autoFocus spellCheck={false} onChange={(event) => setQuery(event.target.value)} data-testid="design-style-search" />
        <div className="design-dark-popover-scroll" style={{ maxHeight: 320 }}>
          {offered.length === 0 && libraries.length === 0 ? (
            <div className="design-dark-caption">
              {styles.some((s) => s.type === HOLDS[slot]) || (libraryAnswer?.libraries ?? []).some((l) => l.styles.length > 0)
                ? t('No matching styles')
                : slot === 'text'
                  ? t('No text styles yet. Click + to create one from the selected text.')
                  : t('No styles yet. Click + to create one from the selection.')}
            </div>
          ) : null}
          {offered.length && libraries.length ? <div className="design-dark-caption" data-testid="design-style-section-local">{t('This file')}</div> : null}
          {section(offered)}
          {libraries.map((l) => (
            <div key={l.id} data-testid={`design-style-library-${l.id}`}>
              <div className="design-dark-caption" style={{ marginTop: 6 }}>{l.name}</div>
              {section(l.styles, l.id)}
            </div>
          ))}
          {colorVars.length || libraryColorVars.length ? <div className="design-dark-caption" style={{ marginTop: 6 }}>{t('Color variables')}</div> : null}
          {vars.collections.map((c) => {
            const list = colorVars.filter((v) => v.collection === c.id)
            if (!list.length) return null
            return (
              <div key={c.id}>
                <div className="design-dark-caption">{c.name}</div>
                {list.map((v) => (
                  <VariableRow
                    key={v.id}
                    vars={vars}
                    variable={v}
                    onPick={() => {
                      void run('variable.bind', { ids, field: slot, variable: v.id })
                      close()
                    }}
                  />
                ))}
              </div>
            )
          })}
          {libraryColorVars.map(({ library, collection, list }) => (
            <div key={`${library.id}/${collection.id}`} data-testid={`design-style-library-variables-${library.id}`}>
              <div className="design-dark-caption">{`${library.name} · ${collection.name}`}</div>
              {list.map((v) => (
                <VariableRow
                  key={v.id}
                  vars={library}
                  variable={v}
                  onPick={() => {
                    if (devRefuses(designKit, 'library.bindVariable')) return
                    void run('library.bindVariable', { library: library.id, variable: v.id, ids, field: slot })
                    close()
                  }}
                />
              ))}
            </div>
          ))}
        </div>
      </>
    )
  }

  return (
    <>
      <div ref={panel} className="design-dark-popover design-style-picker design-layer" style={at} role="dialog" aria-label={t(TITLES[slot])} data-testid="design-style-picker" onPointerLeave={() => preview(null)}>
        {body}
      </div>
      {paintEdit ? <StylePaint styleId={paintEdit.style} index={paintEdit.index} styles={styles} onClose={() => setPaintEdit(null)} /> : null}
    </>
  )
}

/** The colour picker for paint `index` of a paint style: drags are one step. */
function StylePaint({ styleId, index, styles, onClose }: { styleId: string; index: number; styles: StyleJson[]; onClose: () => void }) {
  const style = styles.find((s) => s.id === styleId)
  const list = readPaints(style?.paints ?? [])
  // Drags are one step, ended even when the picker unmounts mid-drag.
  const write = usePaintWriter('Edit style')
  const paint = list[index]
  if (!style || !paint) return null
  const value = (next: Paint) => list.map((p, i) => paintJson(i === index ? next : p))
  return (
    <ColorPicker
      paint={paint}
      onClose={onClose}
      onChange={(next, phase) => write('style.set', { style: styleId, value: value(next) }, phase, null)}
      testId="design-style-color"
    />
  )
}

type EditorProps = {
  style: StyleJson
  def: TextDef | null
  name: string
  description: string
  focus: boolean
  onName: (name: string) => void
  onDescription: (description: string) => void
  onBack: () => void
  onPaint: (index: number) => void
  hasSelection: boolean
  firstId: number | undefined
  onDeleted: () => void
}

const EFFECT_LABELS: Record<string, string> = { dropShadow: 'Drop shadow', innerShadow: 'Inner shadow', layerBlur: 'Layer blur', backgroundBlur: 'Background blur', noise: 'Noise', texture: 'Texture', glass: 'Glass' }

function StyleEditor({ style, def, name, description, focus, onName, onDescription, onBack, onPaint, hasSelection, firstId, onDeleted }: EditorProps) {
  const designKit = useDesignKit()
  const { t, run } = designKit
  const vars = useVariables()
  const nameField = useRef<HTMLInputElement>(null)
  useLayoutEffect(() => {
    if (!focus) return
    nameField.current?.focus()
    nameField.current?.select()
  }, [focus])
  const bindCell = (field: string, kind: 'color' | 'number' | 'string') => {
    const bound = style.bound?.[field]
    if (bound) {
      const full = vars.variables.find((v) => v.id === bound)?.name ?? bound
      return (
        <button type="button" className="design-style-bound" data-tip={t('Bound to {name}. Click to detach the variable.', { name: full })} onClick={(event) => (event.stopPropagation(), void run('style.bind', { style: style.id, field }))}>
          <Icon name="Variable" size={11} />
          <span>{full.split('/').pop()}</span>
        </button>
      )
    }
    return (
      <button
        type="button"
        className="design-dark-icon design-style-bind"
        aria-label={t('Apply variable')}
        data-tip={t('Apply variable')}
        onClick={(event) => {
          event.stopPropagation()
          const r = (event.currentTarget.parentElement ?? event.currentTarget).getBoundingClientRect()
          openVariablePicker({ target: { style: { style: style.id, field } }, kind, x: r.left, y: r.bottom })
        }}
      >
        <Icon name="Variable" size={12} />
      </button>
    )
  }
  let properties: ReactNode = null
  if (style.type === 'paint') {
    const list = readPaints(style.paints ?? [])
    properties = list.length ? (
      list
        .map((p, i) => (
          <div key={i} className="design-style-prop" data-click role="button" onClick={() => onPaint(i)}>
            <span className="design-left-swatch" style={{ background: paintCss(style.paints?.[i]) }} />
            <span>{`${t(p.type === 'solid' ? 'Solid' : p.type === 'image' ? 'Image' : 'Gradient')}  ${Math.round((p.opacity ?? 1) * 100)}%`}</span>
            {p.type === 'solid' ? bindCell(`fills/${i}/color`, 'color') : null}
          </div>
        ))
        .reverse()
    ) : (
      <div className="design-dark-caption">{t('No paints')}</div>
    )
  } else if (style.type === 'effect') {
    properties = (style.effects ?? [])
      .map((fx, i) => {
        const kind = fx.type ?? ''
        const field = /shadow/i.test(kind) ? `effects/${i}/color` : /blur/i.test(kind) ? `effects/${i}/radius` : null
        return (
          <div key={i} className="design-style-prop">
            <Icon name="Effects" size={14} />
            <span>{t(EFFECT_LABELS[kind] ?? kind)}</span>
            {field ? bindCell(field, /color$/.test(field) ? 'color' : 'number') : null}
          </div>
        )
      })
      .reverse()
  } else if (style.type === 'grid') {
    properties = (style.grids ?? []).map((g, i) => {
      const kind = (g.pattern ?? g.kind ?? 'grid').toLowerCase()
      const label = kind.startsWith('col') ? t('Columns {count}', { count: g.count ?? 0 }) : kind.startsWith('row') ? t('Rows {count}', { count: g.count ?? 0 }) : t('Grid {size}', { size: g.size ?? 0 })
      return (
        <div key={i} className="design-style-prop">
          <Icon name="Grid" size={14} />
          <span>{label}</span>
          {bindCell(`layoutGrids/${i}/size`, 'number')}
        </div>
      )
    })
  } else if (def) {
    properties = <TextProperties style={style} def={def} bindCell={bindCell} />
  }
  return (
    <div className="design-style-editor" data-testid="design-style-editor">
      <div className="design-dark-header">
        <button type="button" className="design-dark-icon" aria-label={t('Back to styles')} data-tip={t('Back to styles')} onClick={onBack} data-testid="design-style-back">
          <Icon name="ChevronLeft" size={14} />
        </button>
        <span className="design-dark-popover-title" style={{ marginLeft: 6 }}>{t('Edit style')}</span>
      </div>
      <div className="design-dark-caption">{t('Name')}</div>
      <input ref={nameField} className="design-dark-search" value={name} onChange={(event) => onName(event.target.value)} data-testid="design-style-name" onKeyDown={(event) => event.key === 'Enter' && onBack()} />
      <div className="design-dark-caption">{t('Description')}</div>
      <textarea className="design-dark-search" rows={2} value={description} onChange={(event) => onDescription(event.target.value)} data-testid="design-style-description" />
      <div className="design-dark-caption">{t('Properties')}</div>
      {properties}
      <div className="design-style-buttons">
        {hasSelection && firstId != null ? (
          <button type="button" className="design-dark-button" data-tip={t('Take the values from the first selected layer')} onClick={() => void run('style.set', { style: style.id, from: firstId })}>
            {t('Update from selection')}
          </button>
        ) : null}
        <button
          type="button"
          className="design-dark-button"
          data-tip={t('Delete the style (layers keep their values)')}
          onClick={() => {
            if (devRefuses(designKit, 'style.delete')) return
            void run('style.delete', { style: style.id })
            onDeleted()
          }}
          data-testid="design-style-delete"
        >
          {t('Delete')}
        </button>
      </div>
    </div>
  )
}

/** A text style's properties: sample, font family and style, size, line height, letter spacing. */
function TextProperties({ style, def, bindCell }: { style: StyleJson; def: TextDef; bindCell: (field: string, kind: 'color' | 'number' | 'string') => ReactNode }) {
  const { t, run } = useDesignKit()
  const families = useQuery<{ families: Array<{ family: string; styles: string[] }> }>('fontFamilies')
  const [menu, setMenu] = useState<{ anchor: Element; items: MenuEntry[] } | null>(null)
  const set = (change: Partial<TextDef>) => void run('style.set', { style: style.id, value: { ...def, ...change } })
  const family = families?.families.find((f) => f.family === def.fontFamily)
  const bound = (field: string) => style.bound?.[field]
  return (
    <div className="design-style-text design-on-dark">
      <div className="design-style-text-sample">
        <TextSample id={style.id} text="Ag" size={Math.min(def.fontSize ?? 12, 28)} maxWidth={60} maxHeight={36} />
        <span>
          {def.fontFamily} {def.fontStyle}
          <br />
          {textMeta(def)}
        </span>
      </div>
      {bound('fontFamily') ? (
        <div className="design-style-field-row">{bindCell('fontFamily', 'string')}</div>
      ) : (
        <button
          type="button"
          className="design-field design-style-dropdown"
          data-tip={t('Font')}
          onClick={(event) => {
            const el = event.currentTarget
            setMenu({
              anchor: el,
              items: [
                { label: t('Apply variable'), onSelect: () => openVariablePicker({ target: { style: { style: style.id, field: 'fontFamily' } }, kind: 'string', ...below(el) }) },
                { type: 'separator' },
                ...(families?.families ?? []).map((f) => ({
                  label: f.family,
                  checked: f.family === def.fontFamily,
                  onSelect: () => set({ fontFamily: f.family, ...(f.styles.length && !f.styles.includes(def.fontStyle ?? '') ? { fontStyle: f.styles.find((s) => s.toLowerCase() === 'regular') ?? f.styles[0] } : {}) }),
                })),
              ],
            })
          }}
        >
          <span>{def.fontFamily}</span>
          <Icon name="Chevron" size={12} />
        </button>
      )}
      <div className="design-style-field-row">
        {bound('fontStyle') ? (
          bindCell('fontStyle', 'string')
        ) : (
          <button
            type="button"
            className="design-field design-style-dropdown"
            style={{ width: (W - 8) / 2 }}
            data-tip={t('Font style')}
            onClick={(event) => {
              const el = event.currentTarget
              setMenu({
                anchor: el,
                items: [
                  { label: t('Apply variable'), onSelect: () => openVariablePicker({ target: { style: { style: style.id, field: 'fontStyle' } }, kind: 'string', ...below(el) }) },
                  { type: 'separator' },
                  ...(family?.styles ?? []).map((s) => ({ label: s, checked: s === def.fontStyle, onSelect: () => set({ fontStyle: s }) })),
                ],
              })
            }}
          >
            <span>{def.fontStyle}</span>
            <Icon name="Chevron" size={12} />
          </button>
        )}
        {bound('fontSize') ? bindCell('fontSize', 'number') : <NumberField value={def.fontSize ?? null} label={t('Size')} width={(W - 8) / 2} min={1} max={5000} onCommit={(edit) => set({ fontSize: Math.min(Math.max(edit.kind === 'set' ? edit.value : edit.apply(def.fontSize ?? 0), 1), 5000) })} />}
      </div>
      <div className="design-style-field-row">
        {bound('lineHeight') ? bindCell('lineHeight', 'number') : <MeasureField value={measureLabel(def.lineHeight)} label={t('Line height')} onCommit={(text) => set({ lineHeight: /^auto$/i.test(text.trim()) ? 'auto' : text.trim() })} />}
        {bound('letterSpacing') ? bindCell('letterSpacing', 'number') : <MeasureField value={measureLabel(def.letterSpacing)} label={t('Letter spacing')} onCommit={(text) => set({ letterSpacing: text.trim() })} />}
      </div>
      {menu ? <Menu tone="light" anchor={menu.anchor} items={menu.items} minWidth={200} onClose={() => setMenu(null)} /> : null}
    </div>
  )
}

function MeasureField({ value, label, onCommit }: { value: string; label: string; onCommit: (text: string) => void }) {
  const [text, setText] = useState(value)
  const [focused, setFocused] = useState(false)
  useEffect(() => {
    if (!focused) setText(value)
  }, [focused, value])
  return (
    <label className="design-field" style={{ width: (W - 8) / 2 }} data-tip={label}>
      <input
        value={text}
        aria-label={label}
        onFocus={(event) => (setFocused(true), event.currentTarget.select())}
        onChange={(event) => setText(event.target.value)}
        onBlur={() => {
          setFocused(false)
          if (text !== value && text.trim()) onCommit(text)
        }}
        onKeyDown={(event) => {
          event.stopPropagation()
          if (event.key === 'Enter') event.currentTarget.blur()
        }}
      />
    </label>
  )
}

const LOCAL_GROUPS: Array<[StyleSlot, string]> = [
  ['text', 'Text styles'],
  ['fill', 'Color styles'],
  ['effect', 'Effect styles'],
  ['grid', 'Grid styles'],
]

/**
 * The Design panel's local styles with nothing selected (`style_picker.rs` `local_styles`): per type a
 * 24 px label, folder captions, 28 px rows (swatch or icon, the leaf name, a text style's size and
 * line height); a click opens the style's editor, a right-click offers Edit, Duplicate and Delete.
 * A row dragged by its grip (the 16 px strip left of it) moves within its type, into the folder it
 * lands in (`style.move`); a right-click on a folder renames it (`style.renameFolder`).
 */
export function LocalStyles() {
  const all = useQuery<StyleJson[]>('styles')
  const textStyles = useQuery<{ styles: TextStyleJson[] }>('textStyles')
  const defs = useMemo(() => new Map((textStyles?.styles ?? []).map((s) => [s.id, s.style])), [textStyles])
  // Linked library copies are the library's, not this file's (upstream `library::local_styles`).
  const copies = useQuery<StyleLibraries>('styles.libraries')?.copies ?? {}
  const styles = (Array.isArray(all) ? all : []).filter((s) => !copies[s.id])
  return (
    <div className="design-local-styles" data-testid="design-local-styles">
      {LOCAL_GROUPS.map(([slot, title]) => {
        const list = styles.filter((s) => s.type === HOLDS[slot])
        return list.length ? <LocalStyleGroup key={slot} slot={slot} title={title} list={list} defs={defs} /> : null
      })}
    </div>
  )
}

/** Where a row dragged from shown index `from` lands with the pointer at `p` (`list_drag::drop_slot`). */
function dropSlot(rows: DOMRect[], from: number, p: { x: number; y: number }): { to: number; y: number } | null {
  const first = rows[0]
  const last = rows[rows.length - 1]
  if (!first || !last) return null
  const slack = Math.min(first.height, last.height) / 2
  if (p.x < Math.min(first.left, last.left) || p.x > Math.max(first.right, last.right) || p.y < first.top - slack || p.y > last.bottom + slack) return null
  // The gap under the pointer: how many rows sit above it.
  const gap = rows.filter((r) => r.top + r.height / 2 < p.y).length
  if (gap === from || gap === from + 1) return null
  const above = rows[gap - 1]
  const below = rows[gap]
  const y = above && below ? (above.bottom + below.top) / 2 : above ? last.bottom : first.top
  return { to: gap > from ? gap - 1 : gap, y }
}

function LocalStyleGroup({ slot, title, list, defs }: { slot: StyleSlot; title: string; list: StyleJson[]; defs: Map<string, TextDef> }) {
  const designKit = useDesignKit()
  const { t, run } = designKit
  const [menu, setMenu] = useState<{ x: number; y: number; style: StyleJson } | null>(null)
  const [folderMenu, setFolderMenu] = useState<{ x: number; y: number; folder: string } | null>(null)
  const [renaming, setRenaming] = useState<string | null>(null)
  const [drag, setDrag] = useState<{ from: number; label: string; x: number; y: number; line: number | null } | null>(null)
  const box = useRef<HTMLDivElement>(null)
  const rows = useRef(new Map<number, HTMLElement>())
  // A press on a grip is not a click on its row.
  const gripped = useRef(false)
  const folders: Array<[string, StyleJson[]]> = []
  for (const s of list) {
    const f = folderOf(s.name)
    const at = folders.find(([name]) => name === f)
    if (at) at[1].push(s)
    else folders.push([f, [s]])
  }
  // Top-level styles first, as upstream's `by_folder`.
  folders.sort((a, b) => (a[0] === '' ? -1 : b[0] === '' ? 1 : 0))
  const shown = folders.flatMap(([, items]) => items)
  const edit = (s: StyleJson, el: Element) => openStylePicker({ ids: [], slot, edit: s.id, y: el.getBoundingClientRect().top })
  const rects = () => shown.map((_, i) => rows.current.get(i)?.getBoundingClientRect()).filter((r): r is DOMRect => r != null)

  const startDrag = (event: ReactPointerEvent, index: number, s: StyleJson) => {
    if (event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()
    gripped.current = true
    let dragging = false
    const start = { x: event.clientX, y: event.clientY }
    const move = (e: PointerEvent) => {
      if (!dragging && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 3) return
      if (!dragging) {
        if (devRefuses(designKit, 'style.move')) return stop()
        dragging = true
      }
      const slot = dropSlot(rects(), index, { x: e.clientX, y: e.clientY })
      const top = box.current?.getBoundingClientRect().top ?? 0
      setDrag({ from: index, label: leafOf(s.name), x: e.clientX, y: e.clientY, line: slot ? slot.y - top : null })
    }
    const up = (e: PointerEvent) => {
      stop()
      if (!dragging) return
      const slot = dropSlot(rects(), index, { x: e.clientX, y: e.clientY })
      if (!slot) return
      // `drop_move`: into the folder of the row above the gap (or below it at the top), before the row below.
      const rest = shown.filter((_, i) => i !== index)
      const below = rest[slot.to]
      const near = rest[slot.to - 1] ?? below
      if (!near) return
      const params: Record<string, unknown> = { styles: [s.id], folder: folderOf(near.name) }
      if (below) params.before = below.id
      void run('style.move', params)
    }
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      e.stopPropagation()
      stop()
    }
    const stop = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('keydown', key, true)
      setDrag(null)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    window.addEventListener('keydown', key, true)
  }

  return (
    <div ref={box} className="design-local-styles-group" data-testid={`design-local-styles-${slot}`}>
      <div className="design-local-styles-label">{t(title)}</div>
      {folders.map(([folder, items]) => (
        <div key={folder}>
          {folder ? (
            <div
              className="design-local-styles-folder design-s11"
              data-tip={renaming === folder ? undefined : folder}
              onContextMenu={(event) => {
                event.preventDefault()
                setFolderMenu({ x: event.clientX, y: event.clientY, folder })
              }}
              data-testid={`design-local-folder-${folder}`}
            >
              {renaming === folder ? (
                <RenameInput
                  value={folder}
                  style={{ left: 16, right: 16, top: 2, bottom: 2 }}
                  testId="design-local-folder-rename"
                  onDone={(name) => {
                    setRenaming(null)
                    if (name != null) void run('style.renameFolder', { folder, name })
                  }}
                />
              ) : (
                folder
              )}
            </div>
          ) : null}
          {items.map((s) => {
            const index = shown.indexOf(s)
            const def = defs.get(s.id)
            const paint = s.paints?.[s.paints.length - 1]
            return (
              <button
                key={s.id}
                ref={(el) => {
                  if (el) rows.current.set(index, el)
                  else rows.current.delete(index)
                }}
                type="button"
                className="design-local-style"
                style={{ paddingLeft: 16 + (folder ? 12 : 0) }}
                data-tip={drag ? undefined : s.description ? `${s.name}\n${s.description}` : s.name}
                data-dragging={drag?.from === index || undefined}
                onClick={(event) => {
                  if (gripped.current) {
                    gripped.current = false
                    return
                  }
                  edit(s, event.currentTarget)
                }}
                onContextMenu={(event) => {
                  event.preventDefault()
                  setMenu({ x: event.clientX, y: event.clientY, style: s })
                }}
                data-testid={`design-local-style-${s.id}`}
              >
                <span className="design-local-style-grip" onPointerDown={(event) => startDrag(event, index, s)} onClick={(event) => event.stopPropagation()} data-testid={`design-local-style-grip-${s.id}`}>
                  <Icon name="DragHandle" size={12} />
                </span>
                {slot === 'fill' ? <span className="design-left-swatch design-local-style-swatch" style={{ background: paintCss(paint) }} /> : <Icon name={ICON[slot]} size={16} />}
                <span className="design-local-style-name">{leafOf(s.name)}</span>
                {def ? <span className="design-local-style-meta">{textMeta(def)}</span> : null}
              </button>
            )
          })}
        </div>
      ))}
      {drag?.line != null ? <span className="design-insertion-line" style={{ top: drag.line - 1, left: 16, right: 16 }} /> : null}
      {drag ? <DragGhost x={drag.x} y={drag.y} label={drag.label} /> : null}
      {menu ? (
        <Menu
          tone="light"
          variant="buttons"
          anchor={{ left: menu.x, top: menu.y, right: menu.x, bottom: menu.y }}
          gap={0}
          onClose={() => setMenu(null)}
          items={[
            { label: t('Edit style'), onSelect: () => openStylePicker({ ids: [], slot, edit: menu.style.id, y: menu.y }) },
            { label: t('Duplicate'), onSelect: () => !devRefuses(designKit, 'style.duplicate') && void run('style.duplicate', { style: menu.style.id }) },
            { label: t('Delete'), onSelect: () => !devRefuses(designKit, 'style.delete') && void run('style.delete', { style: menu.style.id }) },
          ]}
        />
      ) : null}
      {folderMenu ? (
        <Menu
          tone="light"
          variant="buttons"
          anchor={{ left: folderMenu.x, top: folderMenu.y, right: folderMenu.x, bottom: folderMenu.y }}
          gap={0}
          onClose={() => setFolderMenu(null)}
          items={[{ label: t('Rename folder'), onSelect: () => !devRefuses(designKit, 'style.renameFolder') && setRenaming(folderMenu.folder), testId: 'design-local-folder-menu-rename' }]}
        />
      ) : null}
    </div>
  )
}


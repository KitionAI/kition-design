/**
 * The Assets tab (2.2, `assets_panel.rs`): the search row with the Libraries button, the "Library
 * updates available" banner (`library.updates`; its label opens the list of what changed, the
 * button pulls the updates in), "Local components" as two-column thumbnail tiles under folder
 * headings (`assets.list`: a set is one tile drawn from its default variant; click to place in the
 * middle of the view, drag onto the canvas to place where it drops, the drop target and insertion
 * line from `drop.preview`, Alt-drop onto an instance to swap it, right-click for "Hide when
 * publishing"), "Local styles" (click to apply), then each enabled library under its name (its
 * published components drawn by `libraryThumbnail`, placed with `library.place` and swapped with
 * `library.swapAt`; its styles applied with `library.applyStyle`; "Can't read this library" when
 * its file can't be read), and the libraries view.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { useCanvasOverlay } from '../canvas/overlays'
import { useDesignKit, useQuery, useView } from '../kit/hooks'
import type { Rect } from '../kit/types'
import { Icon } from '../icons/Icon'
import { Button } from '../ui'
import { Menu } from '../ui/Menu'
import { ScrollBar } from '../ui/ScrollBar'
import { prettyShortcut } from '../ui/shortcuts'
import { devRefuses } from './dev'
import { Libraries } from './Libraries'
import { setLeft, useLeft } from './store'
import { paintCss } from './swatch'

/** One tile of `query('assets.list')` (bindings/wasm/src/lists.rs `assets_list`). */
type Tile = { id: number; name: string; label: string; set: boolean; hidden: boolean; defaultVariant: number; page: number; variants: number }
type Folder = { name: string; tiles: Tile[] }
type StyleRow = { id: string; name: string; type: string; folder: string; label: string; description: string; paints?: unknown[] }
/** An enabled library's section of `assets.list` (tile ids are the library's). */
type LibrarySection = { id: string; name: string; path: string; error: string | null; folders: Folder[]; styles: StyleRow[] }
type Listing = {
  folders: Folder[]
  styles: StyleRow[]
  libraries: LibrarySection[]
  count: number
}
type Style = { id: string; paints?: unknown[] }
type Pending = { id?: string | null; key: string | number; name: string | null }
/** `query('library.updates')`. */
type Updates = { count: number; libraries: Array<{ id: string; name: string; version: number; components: Pending[]; styles: Pending[]; collections: Pending[]; variables: Pending[] }> }
/** `query('drop.preview')`: where an Assets drag released here lands. */
type DropPreview = {
  parent: number
  dropTarget: number | null
  index: number | null
  line: [number, number, number, number] | null
  gridCell: { column: number; row: number; rect: [number, number, number, number] } | null
  swap: number | null
}
/** A tile being dragged: `library` is its library's id (null: this file's). */
type AssetDrag = { tile: Tile; library: string | null; x: number; y: number; preview: DropPreview | null }

const PAD = 16
const GAP = 8

/**
 * Thumbnails by tile id, rendered by the engine: this file's with `nodeThumbnail` (a set showing
 * its default variant), a library's with `libraryThumbnail` from its published snapshot.
 */
function useThumbnails(tiles: Tile[], px: number, library: string | null = null) {
  const { invoke, revision } = useDesignKit()
  const [urls, setUrls] = useState<Map<number, string | null>>(new Map())
  const key = tiles.map((tile) => `${tile.id}:${tile.defaultVariant}`).join(',')
  useEffect(() => {
    let live = true
    // Re-rendered a moment after the document changes, not on every step.
    const timer = window.setTimeout(async () => {
      const next = new Map<number, string | null>()
      for (const pair of key ? key.split(',') : []) {
        if (!live) return
        const [id, shown] = pair.split(':')
        try {
          const bytes = library ? await invoke<Uint8Array>('libraryThumbnail', library, id, px) : await invoke<Uint8Array>('nodeThumbnail', shown, px)
          next.set(Number(id), URL.createObjectURL(new Blob([bytes as BlobPart], { type: 'image/png' })))
        } catch {
          next.set(Number(id), null)
        }
      }
      if (live) setUrls(next)
    }, 300)
    return () => {
      live = false
      window.clearTimeout(timer)
    }
  }, [invoke, key, library, px, revision])
  useEffect(() => () => urls.forEach((url) => url && URL.revokeObjectURL(url)), [urls])
  return urls
}

type TilesProps = {
  /** Where the tiles come from: `local`, or a library's id. */
  source: string
  folders: Folder[]
  tile: number
  query: string
  library: string | null
  onDown: (event: React.PointerEvent, tile: Tile, library: string | null, thumb: string | null) => void
  onPlace: (tile: Tile, library: string | null) => void
  onMenu?: (event: React.MouseEvent, tile: Tile) => void
}

/** Folder headings and two-column tiles (`folders`, `tiles` in `assets_panel.rs`). */
function TileFolders({ source, folders, tile, query, library, onDown, onPlace, onMenu }: TilesProps) {
  const { t } = useDesignKit()
  const left = useLeft()
  const tiles = useMemo(() => folders.flatMap((f) => f.tiles), [folders])
  const thumbs = useThumbnails(tiles, Math.round(tile * Math.min(window.devicePixelRatio || 1, 2)), library)
  const toggleFolder = (key: string) =>
    setLeft((now) => {
      const folded = new Set(now.folded)
      if (!folded.delete(key)) folded.add(key)
      return { folded }
    })
  return (
    <>
      {folders.map((folder) => {
        const folderKey = `${source}/${folder.name}`
        const open = query !== '' || !left.folded.has(folderKey)
        return (
          <div key={folderKey}>
            {folder.name ? (
              <button type="button" className="design-assets-folder" aria-expanded={open} data-tip={t(open ? 'Folder: click to collapse' : 'Folder: click to expand')} onClick={() => toggleFolder(folderKey)} data-testid={library ? `design-assets-folder-${library}-${folder.name}` : `design-assets-folder-${folder.name}`}>
                <Icon name={open ? 'Chevron' : 'ChevronRight'} size={12} />
                <span>{folder.name}</span>
              </button>
            ) : null}
            {open ? (
              <div className="design-assets-tiles" style={{ gridTemplateColumns: `${tile}px ${tile}px` }}>
                {folder.tiles.map((item) => {
                  const url = thumbs.get(item.id)
                  return (
                    <div
                      key={item.id}
                      className="design-asset-tile"
                      role="button"
                      tabIndex={0}
                      aria-label={item.name}
                      data-hidden={item.hidden || undefined}
                      data-set={item.set || undefined}
                      data-variants={item.set ? item.variants : undefined}
                      data-library={library ?? undefined}
                      data-thumb={url ? true : undefined}
                      data-tip={t('{name}\nDrag onto the canvas, or click to place in the middle of the view.\nHold Alt (Option) while dropping onto an instance to swap it.', { name: item.name })}
                      data-testid={library ? `design-asset-${library}-${item.id}` : `design-asset-${item.id}`}
                      onPointerDown={(event) => onDown(event, item, library, url ?? null)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault()
                          onPlace(item, library)
                        }
                      }}
                      onContextMenu={(event) => {
                        event.preventDefault()
                        onMenu?.(event, item)
                      }}
                    >
                      <span className="design-asset-thumb" style={{ width: tile, height: tile }}>
                        {url ? <img src={url} alt="" draggable={false} style={{ maxWidth: tile - 28, maxHeight: tile - 28 }} /> : <Icon name={item.set ? 'ComponentSet' : 'Component'} size={20} />}
                        {item.hidden && !library ? (
                          <span className="design-asset-hidden">
                            <Icon name="EyeOff" size={16} />
                          </span>
                        ) : null}
                      </span>
                      <span className="design-asset-label design-r11">{item.label}</span>
                    </div>
                  )
                })}
              </div>
            ) : null}
          </div>
        )
      })}
    </>
  )
}

/** The drag's drop target, insertion line, grid cell, or the instance an Alt-drop swaps, over the canvas. */
function DropFeedback({ preview }: { preview: DropPreview }) {
  const { query, inspect } = useDesignKit()
  const view = useView()
  const outlined = preview.swap ?? preview.dropTarget
  const [bounds, setBounds] = useState<{ id: number; rect: Rect } | null>(null)
  useEffect(() => {
    if (outlined == null) return undefined
    let live = true
    void query<{ absoluteBounds?: Rect }>('node', { id: outlined }).then(
      (node) => {
        if (live && node.absoluteBounds) setBounds({ id: outlined, rect: node.absoluteBounds })
      },
      () => undefined,
    )
    return () => {
      live = false
    }
  }, [outlined, query])
  if (!view) return null
  const sx = (x: number) => (x - view.x) * view.zoom
  const sy = (y: number) => (y - view.y) * view.zoom
  // `absoluteBounds` is page space; the view is in document pixels.
  const box = bounds && bounds.id === outlined ? bounds.rect : null
  const color = preview.swap != null ? 'var(--design-component-const)' : 'var(--design-accent-line)'
  return (
    <svg className="design-asset-drop" data-testid="design-asset-drop" data-target={outlined ?? undefined} data-swap={preview.swap != null || undefined}>
      {box ? <rect x={sx(box.x - inspect.area.x)} y={sy(box.y - inspect.area.y)} width={box.width * view.zoom} height={box.height * view.zoom} fill="none" stroke={color} strokeWidth={2} /> : null}
      {preview.gridCell ? (
        <rect x={sx(preview.gridCell.rect[0])} y={sy(preview.gridCell.rect[1])} width={(preview.gridCell.rect[2] - preview.gridCell.rect[0]) * view.zoom} height={(preview.gridCell.rect[3] - preview.gridCell.rect[1]) * view.zoom} fill="var(--design-accent-line)" fillOpacity={0.12} stroke="var(--design-accent-line)" strokeDasharray="4 3" data-testid="design-asset-drop-cell" />
      ) : null}
      {preview.line ? <line x1={sx(preview.line[0])} y1={sy(preview.line[1])} x2={sx(preview.line[2])} y2={sy(preview.line[3])} stroke="var(--design-accent-line)" strokeWidth={2} data-testid="design-asset-drop-line" /> : null}
    </svg>
  )
}

export function Assets() {
  const left = useLeft()
  if (left.libraries) return <Libraries />
  return <AssetList />
}

function AssetList() {
  const designKit = useDesignKit()
  const { t, run, editor, inspect } = designKit
  const view = useView()
  const left = useLeft()
  const query = left.assetFilter.trim().toLowerCase()
  const listing = useQuery<Listing>('assets.list', query ? { query } : {})
  const paints = useQuery<Style[]>('styles')
  const updates = useQuery<Updates>('library.updates')
  const [review, setReview] = useState(false)
  const folders = useMemo(() => (listing?.folders ?? []).filter((folder) => folder.tiles.length), [listing])
  const localStyles = listing?.styles ?? []
  const libraries = listing?.libraries ?? []
  const paintOf = useMemo(() => new Map((paints ?? []).map((style) => [style.id, style.paints?.[0]])), [paints])
  const scroller = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(238)
  useEffect(() => {
    const el = scroller.current
    if (!el) return undefined
    const observer = new ResizeObserver(() => setWidth(el.clientWidth))
    observer.observe(el)
    setWidth(el.clientWidth)
    return () => observer.disconnect()
  }, [])
  const tile = Math.max((width - PAD * 2 - GAP) / 2, 40)
  const [menu, setMenu] = useState<{ x: number; y: number; tile: Tile } | null>(null)
  const [drag, setDrag] = useState<AssetDrag | null>(null)
  const [ghost, setGhost] = useState<string | null>(null)
  const feedback = useMemo(() => (drag?.preview ? <DropFeedback preview={drag.preview} /> : null), [drag?.preview])
  useCanvasOverlay('assets.drop', feedback)

  /** The document pixel under window point (`x`, `y`) on the canvas, or the middle of the view; null off the canvas. */
  const docPoint = (at?: { x: number; y: number }) => {
    if (!view) return null
    const surface = document.querySelector('[data-testid="kit-canvas-surface"]')?.getBoundingClientRect()
    if (at && (!surface || at.x < surface.left || at.x > surface.right || at.y < surface.top || at.y > surface.bottom)) return null
    const sx = at && surface ? at.x - surface.left : view.width / 2
    const sy = at && surface ? at.y - surface.top : view.height / 2
    return { x: view.x + sx / view.zoom, y: view.y + sy / view.zoom }
  }

  /** Places (or with `swap`, swaps to) tile `item` of this file or of `library` at a window point, or in the middle of the view. */
  const place = (item: Tile, library: string | null, at?: { x: number; y: number }, swap = false) => {
    const p = docPoint(at)
    const command = library ? (swap ? 'library.swapAt' : 'library.place') : swap ? 'instance.swapAt' : 'instance.place'
    if (!p || devRefuses(designKit, command)) return
    // Page space: the document pixel plus where the document sits on the page.
    const point = { x: p.x + inspect.area.x, y: p.y + inspect.area.y }
    void run(command, library ? { library, component: item.id, ...point } : { component: item.id, ...point })
  }

  const tileDown = (event: React.PointerEvent, item: Tile, library: string | null, thumb: string | null) => {
    if (event.button !== 0) return
    const start = { x: event.clientX, y: event.clientY }
    let dragging = false
    // The latest drop preview asked, so a slow answer never replaces a newer one.
    let asked = 0
    let preview: DropPreview | null = null
    setGhost(thumb)
    const ask = (x: number, y: number, alt: boolean) => {
      const p = docPoint({ x, y })
      const serial = (asked += 1)
      if (!p) {
        preview = null
        setDrag({ tile: item, library, x, y, preview: null })
        return
      }
      setDrag((now) => ({ tile: item, library, x, y, preview: now?.preview ?? null }))
      // A library tile's id is the library's: the engine finds the instance an Alt-drop swaps by position.
      void designKit.query<DropPreview>('drop.preview', { x: p.x, y: p.y, ...(library ? {} : { id: item.id }), alt }).then(
        (answer) => {
          if (serial !== asked || !dragging) return
          preview = answer
          setDrag((now) => (now ? { ...now, preview: answer } : now))
        },
        () => undefined,
      )
    }
    const move = (e: PointerEvent) => {
      if (!dragging && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 4) return
      dragging = true
      ask(e.clientX, e.clientY, e.altKey)
    }
    const stop = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('pointercancel', cancel)
      window.removeEventListener('keydown', key, true)
      setDrag(null)
    }
    // A press the browser cancels places nothing, as Escape.
    const cancel = () => {
      dragging = false
      stop()
    }
    const up = (e: PointerEvent) => {
      stop()
      if (!dragging) {
        place(item, library)
        return
      }
      dragging = false
      // An Alt-drop swaps the instance the preview found; anywhere else it places.
      if (docPoint({ x: e.clientX, y: e.clientY })) place(item, library, { x: e.clientX, y: e.clientY }, e.altKey && preview?.swap != null)
    }
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      e.stopPropagation()
      dragging = false
      stop()
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', key, true)
  }

  const acceptUpdates = async () => {
    if (devRefuses(designKit, 'library.acceptUpdates')) return
    const answer = await run<{ components?: number; styles?: number }>('library.acceptUpdates', {})
    if (!answer) return
    setReview(false)
    designKit.toast(t('Updated {components} and {styles}', { components: t('{n} components', { n: answer.components ?? 0 }), styles: t('{n} styles', { n: answer.styles ?? 0 }) }))
  }

  const applyStyle = (id: string, library: string | null) => {
    const command = library ? 'library.applyStyle' : 'style.apply'
    if (devRefuses(designKit, command)) return
    if (!inspect.selection.length) {
      designKit.toast(t('Select layers to apply the style to'))
      return
    }
    void run(command, library ? { library, style: id } : { style: id })
  }

  const toggleLibrary = (id: string) =>
    setLeft((now) => {
      const folded = new Set(now.folded)
      if (!folded.delete(`library:${id}`)) folded.add(`library:${id}`)
      return { folded }
    })

  const styleRows = (rows: StyleRow[], library: string | null) =>
    rows.map((style) => (
      <button key={style.id} type="button" className="design-assets-style" data-tip={t('{name}\nClick to apply to the selection', { name: style.name })} onClick={() => applyStyle(style.id, library)} data-testid={library ? `design-assets-style-${library}-${style.id}` : `design-assets-style-${style.id}`}>
        {style.type === 'paint' ? <span className="design-left-swatch" style={{ background: paintCss(library ? style.paints?.[0] : paintOf.get(style.id)) }} /> : <Icon name={style.type === 'text' ? 'Text' : style.type === 'effect' ? 'Effects' : 'Grid'} size={14} />}
        <span>{style.name}</span>
      </button>
    ))

  const pending = (updates?.libraries ?? []).flatMap((lib) =>
    [
      ...lib.components.map((item) => ({ kind: 'Component' as const, item })),
      ...lib.styles.map((item) => ({ kind: 'Styles' as const, item })),
      ...[...lib.collections, ...lib.variables].map((item) => ({ kind: 'Variable' as const, item })),
    ].map(({ kind, item }) => ({ library: lib.name, kind, name: item.name ?? String(item.key), key: `${lib.id}/${kind}/${item.key}` })),
  )

  return (
    <>
      <div className="design-assets-search">
        <label className="design-assets-search-field">
          <Icon name="Search" size={12} />
          <input
            value={left.assetFilter}
            placeholder={t('Search assets')}
            aria-label={t('Search assets')}
            spellCheck={false}
            autoComplete="off"
            data-testid="design-assets-search"
            onChange={(event) => setLeft({ assetFilter: event.target.value })}
            onFocus={(event) => event.currentTarget.select()}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                event.stopPropagation()
                setLeft({ assetFilter: '' })
              }
            }}
          />
        </label>
        <button type="button" className="design-left-header-button design-assets-libraries" aria-label={t('Libraries')} data-tip={t('Libraries')} data-testid="design-assets-libraries" onClick={() => setLeft({ libraries: true })}>
          <Icon name="Library" size={16} />
        </button>
      </div>
      <div ref={scroller} className="design-assets-scroll design-scroll-hidden" data-testid="design-assets">
        {updates && updates.count > 0 ? (
          <>
            <div className="design-assets-updates" data-tip={t('{n} components and styles have newer versions', { n: updates.count })} data-testid="design-assets-updates">
              <button type="button" className="design-assets-updates-review design-s11" aria-expanded={review} onClick={() => setReview((now) => !now)} data-testid="design-assets-updates-review">
                {t('Library updates available')}
              </button>
              <Button onClick={() => void acceptUpdates()} testId="design-assets-update">
                {t('Update')}
              </Button>
            </div>
            {review ? (
              <div className="design-assets-updates-list" data-testid="design-assets-updates-list">
                {pending.map((row) => (
                  <div key={row.key} className="design-assets-updates-row design-r11" data-tip={row.library}>
                    <Icon name={row.kind} size={12} />
                    <span>{row.name}</span>
                    <span className="design-assets-updates-library">{row.library}</span>
                  </div>
                ))}
              </div>
            ) : null}
          </>
        ) : null}
        <div className="design-assets-title">{t('Local components')}</div>
        {listing && folders.length === 0 ? (
          <div className="design-assets-hint">{query ? t('No matching components') : t('No components yet. Select layers and press {keys}.', { keys: prettyShortcut(editor.byId.get('component.add')?.shortcut) })}</div>
        ) : null}
        <TileFolders source="local" folders={folders} tile={tile} query={query} library={null} onDown={tileDown} onPlace={(item) => place(item, null)} onMenu={(event, item) => setMenu({ x: event.clientX, y: event.clientY, tile: item })} />
        <hr className="design-separator" style={{ marginBottom: 10 }} />
        <div className="design-assets-title">{t('Local styles')}</div>
        {listing && localStyles.length === 0 ? <div className="design-assets-hint">{query ? t('No matching styles') : t('No styles yet.')}</div> : null}
        {styleRows(localStyles, null)}
        {libraries.map((library) => {
          const open = query !== '' || !left.folded.has(`library:${library.id}`)
          return (
            <div key={library.id} data-testid={`design-assets-library-${library.id}`}>
              <hr className="design-separator" style={{ margin: '6px 0 4px' }} />
              <button type="button" className="design-assets-library" aria-expanded={open} data-tip={t(open ? 'Library: click to collapse' : 'Library: click to expand')} onClick={() => toggleLibrary(library.id)} data-testid={`design-assets-library-title-${library.id}`}>
                <Icon name={open ? 'Chevron' : 'ChevronRight'} size={12} />
                <span>{library.name}</span>
              </button>
              {open && library.error ? <div className="design-assets-hint" data-testid={`design-assets-library-error-${library.id}`}>{t("Can't read this library: {e}", { e: library.error })}</div> : null}
              {open && !library.error ? (
                <>
                  <TileFolders source={library.id} folders={library.folders.filter((folder) => folder.tiles.length)} tile={tile} query={query} library={library.id} onDown={tileDown} onPlace={(item) => place(item, library.id)} />
                  {styleRows(library.styles, library.id)}
                </>
              ) : null}
            </div>
          )
        })}
      </div>
      <ScrollBar target={scroller} />
      {drag ? (
        <span className="design-asset-ghost" style={{ left: drag.x - 32, top: drag.y - 32 }}>
          {ghost ? <img src={ghost} alt="" /> : <Icon name={drag.tile.set ? 'ComponentSet' : 'Component'} size={20} />}
        </span>
      ) : null}
      {menu ? (
        <Menu
          tone="light"
          anchor={{ left: menu.x, top: menu.y, right: menu.x, bottom: menu.y }}
          gap={0}
          minWidth={180}
          onClose={() => setMenu(null)}
          testId="design-asset-menu"
          items={[{ label: t('Hide when publishing'), checked: menu.tile.hidden, onSelect: () => !devRefuses(designKit, 'component.hideWhenPublishing') && void run('component.hideWhenPublishing', { ids: [menu.tile.id], on: !menu.tile.hidden }), testId: 'design-asset-menu-hide' }]}
        />
      ) : null}
    </>
  )
}

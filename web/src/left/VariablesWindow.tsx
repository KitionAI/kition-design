/**
 * The Local variables window (2.2, `dialogs/variables_panel.rs`): collections and their groups on
 * the left; the selected collection's variables on the right as a table with a value column per
 * mode. Double-click renames a collection, group, mode or variable; right-click for more. Values
 * edit in place (colours with the colour picker, numbers scrub as one step), and a value's
 * variable button makes it an alias. Import and export read and write the variables JSON.
 * Each enabled library's published collections follow under its name (`variables.libraries`),
 * read-only, with "Extend collection" (`library.extendCollection`); this file's linked copies of
 * them are left out of its own collections, as upstream's `View::local`.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Button, Checkbox, NumberField, TextField } from '../ui'
import { Menu, type MenuEntry } from '../ui/Menu'
import { ColorPicker } from '../paint/ColorPicker'
import { usePaintWriter } from '../paint/PaintPicker'
import { fromHexOrName } from '../paint/color'
import { editColor, solid } from '../paint/paint'
import { devRefuses } from './dev'
import { RenameInput } from './fields'
import { colorCss } from './swatch'
import { openVariablePicker } from './VariablePicker'
import { cssHexOf, display, groupOf, groups, hexOf, resolve, SCOPES, unique, useVariables, valueJson, type Collection, type Variable, type VariableKind, type Variables } from './variables'

const SIDEBAR_W = 200
const NAME_W = 240
const MODE_W = 200
const KINDS: Array<[VariableKind, string]> = [
  ['color', 'Color'],
  ['number', 'Number'],
  ['string', 'String'],
  ['boolean', 'Boolean'],
]

type Rename = { kind: 'collection'; id: string } | { kind: 'mode'; collection: string; mode: string } | { kind: 'variable'; id: string } | { kind: 'group'; collection: string; group: string }
type MenuAt = { x: number; y: number; items: MenuEntry[]; from?: string }

const sameRename = (a: Rename | null, b: Rename) => JSON.stringify(a) === JSON.stringify(b)

/** Each enabled library's published collections and variables, and this file's linked copies (`variables.libraries`). */
type VariableLibraries = { copies: { collections: string[]; variables: string[] }; libraries: Array<{ id: string; name: string; error: string | null } & Variables> }
/** A library collection chosen in the sidebar. */
type LibraryPick = { library: string; collection: string }

export function VariablesWindow({ initial, onClose }: { initial?: string | null; onClose: () => void }) {
  const designKit = useDesignKit()
  const { t, run } = designKit
  const vars = useVariables()
  const libraryAnswer = useQuery<VariableLibraries>('variables.libraries')
  const copies = new Set(libraryAnswer?.copies.collections ?? [])
  const libraries = (libraryAnswer?.libraries ?? []).filter((l) => l.collections.length > 0)
  const [picked, setPicked] = useState<LibraryPick | null>(null)
  const [collectionId, setCollectionId] = useState<string | null>(initial ?? null)
  const setCollection = (id: string | null) => {
    setPicked(null)
    setCollectionId(id)
  }
  const [group, setGroup] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [rename, setRename] = useState<Rename | null>(null)
  const [details, setDetails] = useState<string | null>(null)
  const [menu, setMenu] = useState<MenuAt | null>(null)
  const [color, setColor] = useState<{ variable: Variable; mode: string } | null>(null)
  const file = useRef<HTMLInputElement>(null)
  const [size, setSize] = useState({ width: 880, height: 480 })
  useEffect(() => {
    const fit = () => setSize({ width: Math.max(480, Math.min(880, window.innerWidth - 80)), height: Math.max(240, Math.min(480, window.innerHeight - 120)) })
    fit()
    window.addEventListener('resize', fit)
    return () => window.removeEventListener('resize', fit)
  }, [])

  // This file's own collections: its linked library copies are the library's.
  const own = vars.collections.filter((c) => !copies.has(c.id))
  const coll = own.find((c) => c.id === collectionId || c.name === collectionId) ?? own[0] ?? null
  const pickedLibrary = picked ? libraries.find((l) => l.id === picked.library) : undefined
  const pickedCollection = picked ? pickedLibrary?.collections.find((c) => c.id === picked.collection) : undefined
  const edit = (command: string, params: Record<string, unknown>) => {
    if (devRefuses(designKit, command)) return Promise.resolve(null)
    return run<{ id?: string }>(command, params)
  }

  const createCollection = async () => {
    const name = unique('Collection', (n) => vars.collections.some((c) => c.name === n))
    const made = await edit('variable.createCollection', { name, modes: ['Mode 1'] })
    if (made?.id) {
      setCollection(made.id)
      setGroup(null)
      setRename({ kind: 'collection', id: made.id })
    }
  }

  const exportAs = async (format: string) => {
    const answer = await run<{ text?: string }>('variable.export', { format })
    if (!answer?.text) return
    const name = `${designKit.inspect.name}.variables.${format}`
    const url = URL.createObjectURL(new Blob([answer.text], { type: 'text/plain' }))
    const a = document.createElement('a')
    a.href = url
    a.download = name
    a.click()
    URL.revokeObjectURL(url)
    designKit.toast(t('Exported {file}', { file: name }))
  }

  const importFile = async (picked: File | undefined) => {
    if (!picked) return
    const text = await picked.text()
    const answer = await edit('variable.import', { variables: text }) as { variables?: number; updated?: number; warnings?: unknown[] } | null
    if (!answer) return
    let message = t('Imported {n} new variables, updated {updated}', { n: answer.variables ?? 0, updated: answer.updated ?? 0 })
    const skipped = answer.warnings?.length ?? 0
    if (skipped) message = `${message} ${t('({n} warnings: some entries were skipped)', { n: skipped })}`
    designKit.toast(message)
  }

  const left = Math.round((window.innerWidth - size.width) / 2)
  const top = Math.round((window.innerHeight - size.height) / 2)
  return (
    <div className="design-vars design-layer" style={{ left, top, width: size.width, height: size.height }} role="dialog" aria-label={t('Local variables')} data-testid="design-variables">
      <div className="design-vars-header">
        <span className="design-s13">{t('Local variables')}</span>
        <span style={{ flex: 1 }} />
        <Button testId="design-variables-import" tip={t('Add or update collections from a variables JSON')} onClick={() => file.current?.click()}>
          {t('Import…')}
        </Button>
        <Button
          testId="design-variables-export"
          // Upstream's button keeps its pressed fill while its menu is open.
          style={menu?.from === 'export' ? { background: 'var(--design-hover)' } : undefined}
          onClick={() => {
            const rect = document.querySelector('[data-testid="design-variables-export"]')?.getBoundingClientRect()
            setMenu({
              from: 'export',
              x: rect?.left ?? 0,
              y: rect?.bottom ?? 0,
              items: [
                { label: t('Variables JSON…'), onSelect: () => void exportAs('json') },
                { label: t('CSS custom properties…'), onSelect: () => void exportAs('css') },
                { label: t('Dart constants for Flutter…'), onSelect: () => void exportAs('dart') },
              ],
            })
          }}
        >
          {t('Export…')}
        </Button>
        <button type="button" className="design-left-header-button" style={{ position: 'static', marginLeft: 4 }} aria-label={t('Close')} data-tip={t('Close')} onClick={onClose} data-testid="design-variables-close">
          <Icon name="Close" size={14} />
        </button>
        <input ref={file} type="file" accept=".json,application/json" hidden onChange={(event) => void importFile(event.target.files?.[0]).finally(() => (event.target.value = ''))} />
      </div>
      <div className="design-vars-body">
        <div className="design-vars-sidebar">
          <div className="design-vars-sidebar-header">
            <span>{t('Collections')}</span>
            <button type="button" className="design-left-header-button" style={{ right: 8 }} aria-label={t('Create collection')} data-tip={t('Create collection')} onClick={() => void createCollection()} data-testid="design-variables-add-collection">
              <Icon name="Plus" size={14} />
            </button>
          </div>
          <div className="design-vars-scroll">
            {own.map((c) => {
              const selected = !pickedCollection && c.id === coll?.id
              const members = vars.variables.filter((v) => v.collection === c.id)
              const target: Rename = { kind: 'collection', id: c.id }
              return (
                <div key={c.id}>
                  <div
                    className="design-vars-collection"
                    data-selected={selected || undefined}
                    data-testid={`design-variables-collection-${c.id}`}
                    onClick={() => {
                      if (!selected) {
                        setGroup(null)
                        setQuery('')
                      }
                      setCollection(c.id)
                    }}
                    onDoubleClick={() => setRename(target)}
                    onContextMenu={(event) => {
                      event.preventDefault()
                      setMenu({ x: event.clientX, y: event.clientY, items: [{ label: t('Rename'), onSelect: () => setRename(target) }, { label: t('Delete collection'), onSelect: () => void edit('variable.deleteCollection', { collection: c.id }) }] })
                    }}
                  >
                    {sameRename(rename, target) ? (
                      <RenameInput value={c.name} style={{ left: 14, right: 40, top: 4, bottom: 4 }} onDone={(name) => (setRename(null), name != null && void edit('variable.renameCollection', { collection: c.id, name }))} />
                    ) : (
                      <span className={`design-vars-collection-name ${selected ? 'design-s11' : 'design-r11'}`}>{c.name}</span>
                    )}
                    <span className="design-vars-count">{members.length}</span>
                  </div>
                  {selected ? (
                    <GroupRows
                      vars={vars}
                      collection={c}
                      group={group}
                      onGroup={setGroup}
                      rename={rename}
                      onRename={setRename}
                      onMenu={setMenu}
                      onEdit={edit}
                    />
                  ) : null}
                </div>
              )
            })}
            {libraries.map((l) => (
              <div key={l.id} data-testid={`design-variables-library-${l.id}`}>
                <div className="design-vars-library" data-tip={t('Library: its variables are read-only here; edit them in the library file and publish it')}>
                  <Icon name="Library" size={14} />
                  <span className="design-s11">{l.name}</span>
                </div>
                {l.collections.map((c) => (
                  <div
                    key={c.id}
                    className="design-vars-collection"
                    data-library={l.id}
                    data-selected={(picked?.library === l.id && picked.collection === c.id) || undefined}
                    data-testid={`design-variables-library-collection-${l.id}-${c.id}`}
                    onClick={() => {
                      setGroup(null)
                      setQuery('')
                      setPicked({ library: l.id, collection: c.id })
                    }}
                  >
                    <span className={`design-vars-collection-name ${picked?.library === l.id && picked.collection === c.id ? 'design-s11' : 'design-r11'}`}>{c.name}</span>
                    <span className="design-vars-count">{l.variables.filter((v) => v.collection === c.id).length}</span>
                  </div>
                ))}
              </div>
            ))}
          </div>
        </div>
        <div className="design-vars-table-pane">
          {pickedLibrary && pickedCollection ? (
            <LibraryTable
              library={pickedLibrary}
              collection={pickedCollection}
              query={query}
              onQuery={setQuery}
              onExtended={(id) => {
                setCollection(id)
                setGroup(null)
              }}
              onEdit={edit}
            />
          ) : coll ? (
            <Table
              vars={vars}
              collection={coll}
              group={group}
              query={query}
              onQuery={setQuery}
              rename={rename}
              onRename={setRename}
              onMenu={setMenu}
              onDetails={setDetails}
              onColor={setColor}
              onEdit={edit}
              details={details}
            />
          ) : (
            <div className="design-vars-empty">
              <div>{t('Variables store reusable values (colours, numbers, strings, booleans) with modes such as Light and Dark.')}</div>
              <Button variant="primary" onClick={() => void createCollection()} testId="design-variables-create-first">
                {t('Create collection')}
              </Button>
            </div>
          )}
        </div>
      </div>
      {menu ? <Menu tone="light" anchor={{ left: menu.x, top: menu.y, right: menu.x, bottom: menu.y }} gap={0} minWidth={160} items={menu.items} onClose={() => setMenu(null)} testId="design-variables-menu" /> : null}
      {details ? <Details vars={vars} id={details} onClose={() => setDetails(null)} onEdit={edit} /> : null}
      {color ? <VariableColor vars={vars} variable={color.variable} mode={color.mode} onClose={() => setColor(null)} /> : null}
    </div>
  )
}

type Edit = (command: string, params: Record<string, unknown>) => Promise<{ id?: string } | null>

/** "All variables" and the collection's groups (indented by depth), with counts. */
function GroupRows({ vars, collection, group, onGroup, rename, onRename, onMenu, onEdit }: { vars: Variables; collection: Collection; group: string | null; onGroup: (g: string | null) => void; rename: Rename | null; onRename: (r: Rename | null) => void; onMenu: (m: MenuAt) => void; onEdit: Edit }) {
  const { t, run } = useDesignKit()
  const members = vars.variables.filter((v) => v.collection === collection.id)
  const list = groups(members.map((v) => v.name))
  if (!list.length) return null
  const renameGroup = (g: string, next: string) => {
    const name = next.trim().replace(/\//g, '-')
    if (!name) return
    const at = g.lastIndexOf('/')
    const renamed = at < 0 ? name : `${g.slice(0, at)}/${name}`
    const prefix = `${g}/`
    const commands = members.filter((v) => v.name.startsWith(prefix)).map((v) => ({ command: 'variable.update', params: { variable: v.id, name: `${renamed}/${v.name.slice(prefix.length)}` } }))
    void run('command.batch', { label: 'Rename group', commands }).then(() => {
      if (group && (group === g || group.startsWith(prefix))) onGroup(`${renamed}${group.slice(g.length)}`)
    })
  }
  return (
    <>
      {[null, ...list].map((g) => {
        const depth = g ? (g.match(/\//g) ?? []).length : 0
        const count = members.filter((v) => g == null || v.name.startsWith(`${g}/`)).length
        const target: Rename | null = g ? { kind: 'group', collection: collection.id, group: g } : null
        const label = g ? g.split('/').pop() ?? g : t('All variables')
        return (
          <div
            key={g ?? '*'}
            className="design-vars-group"
            data-on={group === g || undefined}
            data-all={g == null || undefined}
            onClick={() => onGroup(g)}
            onDoubleClick={() => target && onRename(target)}
            onContextMenu={(event) => {
              event.preventDefault()
              if (!g || !target) return
              onMenu({
                x: event.clientX,
                y: event.clientY,
                items: [
                  { label: t('Rename group'), onSelect: () => onRename(target) },
                  {
                    label: t('Delete group'),
                    onSelect: () => {
                      const commands = members.filter((v) => v.name.startsWith(`${g}/`)).map((v) => ({ command: 'variable.delete', params: { variable: v.id } }))
                      void onEdit('command.batch', { label: 'Delete group', commands })
                      if (group && (group === g || group.startsWith(`${g}/`))) onGroup(null)
                    },
                  },
                ],
              })
            }}
            data-testid={`design-variables-group-${g ?? 'all'}`}
          >
            {target && sameRename(rename, target) ? (
              <RenameInput value={label} style={{ left: 28 + depth * 12, right: 36, top: 3, bottom: 3 }} onDone={(next) => (onRename(null), next != null && g && renameGroup(g, next))} />
            ) : (
              <span className="design-vars-group-name" style={{ left: 28 + depth * 12 }}>
                {label}
              </span>
            )}
            <span className="design-vars-count design-r10">{count}</span>
          </div>
        )
      })}
    </>
  )
}

/**
 * A library collection's variables (upstream `variables_panel.rs` with `view.editable()` false):
 * the values of each mode shown, nothing to rename, add or delete; "Extend collection" makes a
 * collection of this file with its modes and variables and opens it.
 */
function LibraryTable({ library, collection, query, onQuery, onExtended, onEdit }: { library: { id: string; name: string } & Variables; collection: Collection; query: string; onQuery: (q: string) => void; onExtended: (id: string) => void; onEdit: Edit }) {
  const { t } = useDesignKit()
  const q = query.trim().toLowerCase()
  const list = library.variables
    .filter((v) => v.collection === collection.id && (q === '' || v.name.toLowerCase().includes(q)))
    .sort((a, b) => (groupOf(a.name) < groupOf(b.name) ? -1 : groupOf(a.name) > groupOf(b.name) ? 1 : 0))
  const width = NAME_W + collection.modes.length * MODE_W + 40
  const extend = async () => {
    const made = await onEdit('library.extendCollection', { library: library.id, collection: collection.id })
    if (made?.id) onExtended(made.id)
  }
  const rows: ReactNode[] = []
  let shownGroup: string | null = null
  for (const v of list) {
    const g = groupOf(v.name)
    if (g !== shownGroup) {
      shownGroup = g
      if (g) rows.push(<div key={`g:${g}`} className="design-vars-group-header" style={{ width }}>{g}</div>)
    }
    rows.push(
      <div key={v.id} className="design-vars-row" style={{ width }} data-testid={`design-variables-library-row-${v.id}`}>
        <KindGlyph vars={library} variable={v} collection={collection} />
        <span className="design-vars-row-name">{v.name.split('/').pop() ?? v.name}</span>
        {collection.modes.map((m, i) => (
          <span key={m.id} className="design-vars-cell" style={{ left: NAME_W + i * MODE_W + 4 }} data-testid={`design-variables-library-cell-${v.id}-${m.id}`}>
            <span className="design-vars-cell-body" style={{ right: 26 }}>
              <ReadOnlyValue vars={library} variable={v} mode={m.id} />
            </span>
          </span>
        ))}
      </div>,
    )
  }
  return (
    <>
      <div className="design-vars-search-row">
        <label className="design-vars-search">
          <Icon name="Search" size={12} />
          <input value={query} placeholder={t('Search')} aria-label={t('Search')} spellCheck={false} onChange={(event) => onQuery(event.target.value)} data-testid="design-variables-search" />
        </label>
        <span className="design-vars-note" data-testid="design-variables-library-note">{t('From the library "{name}": read-only', { name: library.name })}</span>
        <Button tip={t('A collection of this file with these modes and variables: override their values per mode and add variables of its own')} onClick={() => void extend()} testId="design-variables-extend">
          {t('Extend collection')}
        </Button>
      </div>
      <div className="design-vars-table" data-testid="design-variables-table" data-library={library.id}>
        <div className="design-vars-head" style={{ width }}>
          <span className="design-vars-head-name">{t('Name')}</span>
          {collection.modes.map((m, i) => (
            <span key={m.id} className="design-vars-mode" style={{ left: NAME_W + i * MODE_W + 4, width: MODE_W - 12 }} data-testid={`design-variables-mode-${i}`}>
              {m.name}
            </span>
          ))}
        </div>
        {rows}
        {list.length === 0 && q ? <div className="design-vars-none">{t('No matching variables')}</div> : null}
      </div>
    </>
  )
}

/** A value shown, not edited (upstream `read_only_value`): a swatch and its hex, an alias's name, or the text. */
function ReadOnlyValue({ vars, variable, mode }: { vars: Variables; variable: Variable; mode: string }) {
  const { t } = useDesignKit()
  const value = variable.values[mode]
  if (!value) return null
  if (value.type === 'alias') {
    return (
      <span className="design-vars-readonly">
        <span className="design-vars-alias-chip">
          <Icon name="Variable" size={12} />
          <span>{vars.variables.find((x) => x.id === value.value)?.name ?? t('Missing variable')}</span>
        </span>
      </span>
    )
  }
  return (
    <span className="design-vars-readonly">
      {value.type === 'color' ? <span className="design-left-swatch" style={{ background: colorCss(value.value) }} /> : null}
      <span>{display(value, t)}</span>
    </span>
  )
}

type TableProps = {
  vars: Variables
  collection: Collection
  group: string | null
  query: string
  onQuery: (q: string) => void
  rename: Rename | null
  onRename: (r: Rename | null) => void
  onMenu: (m: MenuAt) => void
  onDetails: (id: string | null) => void
  onColor: (c: { variable: Variable; mode: string } | null) => void
  onEdit: Edit
  details: string | null
}

function Table({ vars, collection, group, query, onQuery, rename, onRename, onMenu, onDetails, onColor, onEdit, details }: TableProps) {
  const { t } = useDesignKit()
  const q = query.trim().toLowerCase()
  const parent = collection.extends ? vars.collections.find((c) => c.id === collection.extends) : null
  const list = vars.variables
    .filter((v) => v.collection === collection.id && (group == null || v.name.startsWith(`${group}/`)) && (q === '' || v.name.toLowerCase().includes(q)))
    .sort((a, b) => (groupOf(a.name) < groupOf(b.name) ? -1 : groupOf(a.name) > groupOf(b.name) ? 1 : 0))
  const width = NAME_W + collection.modes.length * MODE_W + 40
  const editableModes = !collection.extends
  const [createMenu, setCreateMenu] = useState<HTMLElement | null>(null)

  const create = async (kind: VariableKind, label: string) => {
    const name = unique(label, (n) => vars.variables.some((v) => v.collection === collection.id && v.name === n))
    const made = await onEdit('variable.create', { collection: collection.id, name, type: kind })
    if (made?.id) {
      onQuery('')
      onRename({ kind: 'variable', id: made.id })
    }
  }
  const addMode = async () => {
    const name = unique('Mode', (n) => collection.modes.some((m) => m.name === n))
    const made = await onEdit('variable.addMode', { collection: collection.id, name })
    if (made?.id) onRename({ kind: 'mode', collection: collection.id, mode: made.id })
  }

  const rows: ReactNode[] = []
  let shownGroup: string | null = null
  for (const v of list) {
    const g = groupOf(v.name)
    if (g !== shownGroup) {
      shownGroup = g
      if (g) rows.push(<div key={`g:${g}`} className="design-vars-group-header" style={{ width }}>{g}</div>)
    }
    rows.push(<VariableRow key={v.id} vars={vars} collection={collection} variable={v} width={width} rename={rename} onRename={onRename} onMenu={onMenu} onDetails={onDetails} onColor={onColor} onEdit={onEdit} open={details === v.id} />)
  }

  return (
    <>
      <div className="design-vars-search-row">
        <label className="design-vars-search">
          <Icon name="Search" size={12} />
          <input value={query} placeholder={t('Search')} aria-label={t('Search')} spellCheck={false} onChange={(event) => onQuery(event.target.value)} data-testid="design-variables-search" />
        </label>
        {parent ? <span className="design-vars-note">{t('Extends "{name}": overridden values are marked', { name: parent.name })}</span> : null}
      </div>
      <div className="design-vars-table" data-testid="design-variables-table">
        <div className="design-vars-head" style={{ width }}>
          <span className="design-vars-head-name">{t('Name')}</span>
          {collection.modes.map((m, i) => {
            const target: Rename = { kind: 'mode', collection: collection.id, mode: m.id }
            if (editableModes && sameRename(rename, target))
              return <RenameInput key={m.id} value={m.name} style={{ left: NAME_W + i * MODE_W + 8, width: MODE_W - 20, top: 6, bottom: 6 }} onDone={(name) => (onRename(null), name != null && void onEdit('variable.renameMode', { collection: collection.id, mode: m.id, name }))} />
            const last = collection.modes.length - 1
            const items: MenuEntry[] = [
              { label: t('Rename mode'), onSelect: () => onRename(target) },
              {
                label: t('Duplicate mode'),
                onSelect: async () => {
                  const name = unique(`${m.name} copy`, (n) => collection.modes.some((x) => x.name === n))
                  const made = await onEdit('variable.addMode', { collection: collection.id, name, from: m.id })
                  if (made?.id) onRename({ kind: 'mode', collection: collection.id, mode: made.id })
                },
              },
              ...(i > 0 ? [{ label: t('Set as default'), onSelect: () => void onEdit('variable.moveMode', { collection: collection.id, mode: m.id, to: 0 }) }] : []),
              ...(i > 0 ? [{ label: t('Move column left'), onSelect: () => void onEdit('variable.moveMode', { collection: collection.id, mode: m.id, to: i - 1 }) }] : []),
              ...(i < last ? [{ label: t('Move column right'), onSelect: () => void onEdit('variable.moveMode', { collection: collection.id, mode: m.id, to: i + 1 }) }] : []),
              ...(collection.modes.length > 1 ? [{ label: t('Delete mode'), onSelect: () => void onEdit('variable.removeMode', { collection: collection.id, mode: m.id }) }] : []),
            ]
            return (
              <span
                key={m.id}
                className="design-vars-mode"
                data-editable={editableModes || undefined}
                style={{ left: NAME_W + i * MODE_W + 4, width: MODE_W - 12 }}
                onDoubleClick={() => editableModes && onRename(target)}
                onContextMenu={(event) => {
                  event.preventDefault()
                  if (editableModes) onMenu({ x: event.clientX, y: event.clientY, items })
                }}
                data-testid={`design-variables-mode-${i}`}
              >
                {m.name}
              </span>
            )
          })}
          {editableModes ? (
            <button type="button" className="design-left-header-button" style={{ left: NAME_W + collection.modes.length * MODE_W + 2, top: 4 }} aria-label={t('Add mode')} data-tip={t('Add mode')} onClick={() => void addMode()} data-testid="design-variables-add-mode">
              <Icon name="Plus" size={14} />
            </button>
          ) : null}
        </div>
        {rows}
        {list.length === 0 && q ? <div className="design-vars-none">{t('No matching variables')}</div> : null}
        <div className="design-vars-create">
          <Button onClick={() => setCreateMenu(document.querySelector<HTMLElement>('[data-testid="design-variables-create"]'))} testId="design-variables-create">
            {t('+ Create variable')}
          </Button>
        </div>
      </div>
      {createMenu ? (
        <Menu
          tone="light"
          anchor={createMenu}
          minWidth={140}
          onClose={() => setCreateMenu(null)}
          testId="design-variables-create-menu"
          items={KINDS.map(([kind, label]) => ({ label: t(label), onSelect: () => void create(kind, label), testId: `design-variables-create-${kind}` }))}
        />
      ) : null}
    </>
  )
}

type RowProps = {
  vars: Variables
  collection: Collection
  variable: Variable
  width: number
  rename: Rename | null
  onRename: (r: Rename | null) => void
  onMenu: (m: MenuAt) => void
  onDetails: (id: string | null) => void
  onColor: (c: { variable: Variable; mode: string } | null) => void
  onEdit: Edit
  open: boolean
}

function VariableRow({ vars, collection, variable: v, width, rename, onRename, onMenu, onDetails, onColor, onEdit, open }: RowProps) {
  const { t } = useDesignKit()
  const target: Rename = { kind: 'variable', id: v.id }
  const inherited = v.extends != null
  const short = v.name.split('/').pop() ?? v.name
  return (
    <div
      className="design-vars-row"
      data-open={open || undefined}
      style={{ width }}
      data-testid={`design-variables-row-${v.id}`}
      onContextMenu={(event) => {
        event.preventDefault()
        onMenu({
          x: event.clientX,
          y: event.clientY,
          items: [
            { label: t('Edit variable'), onSelect: () => onDetails(v.id) },
            ...(!inherited ? [{ label: t('Rename'), onSelect: () => onRename(target) }] : []),
            ...(v.overrides?.length ? [{ label: t('Reset overrides'), onSelect: () => void onEdit('variable.resetOverride', { variable: v.id }) }] : []),
            { label: t('Duplicate'), onSelect: () => void onEdit('variable.duplicate', { variable: v.id }) },
            ...(!inherited ? [{ label: t('Delete variable'), onSelect: () => void onEdit('variable.delete', { variable: v.id }), testId: 'design-variables-delete' }] : []),
          ],
        })
      }}
    >
      <KindGlyph vars={vars} variable={v} collection={collection} />
      {sameRename(rename, target) ? (
        <RenameInput value={v.name} style={{ left: 38, width: NAME_W - 46, top: 6, bottom: 6 }} testId="design-variables-rename" onDone={(name) => (onRename(null), name != null && void onEdit('variable.update', { variable: v.id, name }))} />
      ) : (
        <span className="design-vars-row-name" onDoubleClick={() => !inherited && onRename(target)}>
          {short}
        </span>
      )}
      {collection.modes.map((m, i) => (
        <ValueCell key={m.id} vars={vars} variable={v} collection={collection} mode={m.id} left={NAME_W + i * MODE_W + 4} onColor={onColor} onEdit={onEdit} />
      ))}
    </div>
  )
}

/** The type marker before a name: the colour (default mode), "#", the text icon, or a switch. */
function KindGlyph({ vars, variable, collection }: { vars: Variables; variable: Variable; collection: Collection }) {
  if (variable.resolvedType === 'color') {
    const value = resolve(vars, variable.id, { [collection.id]: collection.modes[0]?.id ?? '' })
    return <span className="design-vars-glyph design-left-swatch" style={{ background: value?.type === 'color' ? colorCss(value.value) : 'transparent' }} />
  }
  if (variable.resolvedType === 'number') return <span className="design-vars-glyph design-s12 design-vars-hash">#</span>
  if (variable.resolvedType === 'string')
    return (
      <span className="design-vars-glyph">
        <Icon name="Text" size={14} />
      </span>
    )
  return <span className="design-vars-glyph design-vars-bool" />
}

function ValueCell({ vars, variable: v, collection, mode, left, onColor, onEdit }: { vars: Variables; variable: Variable; collection: Collection; mode: string; left: number; onColor: (c: { variable: Variable; mode: string }) => void; onEdit: Edit }) {
  const designKit = useDesignKit()
  const { t } = designKit
  const value = v.values[mode]
  const scrub = useRef<ReturnType<typeof designKit.interaction> | null>(null)
  if (!value) return null
  const set = (next: unknown) => void onEdit('variable.set', { variable: v.id, mode, value: next })
  const extended = collection.extends != null
  const overridden = v.overrides?.includes(mode)
  const bodyRight = extended ? 50 : 26
  const aliasPicker = (el: HTMLElement) => {
    const r = el.getBoundingClientRect()
    openVariablePicker({ target: { alias: { variable: v.id, mode } }, kind: v.resolvedType, x: r.left, y: r.bottom })
  }
  let body: ReactNode
  if (value.type === 'alias') {
    const name = vars.variables.find((x) => x.id === value.value)?.name ?? t('Missing variable')
    body = (
      <button type="button" className="design-vars-alias" data-tip={t('Change alias')} onClick={(event) => aliasPicker(event.currentTarget)} data-testid={`design-variables-alias-${v.id}`}>
        <span className="design-vars-alias-chip">
          <Icon name="Variable" size={12} />
          <span>{name}</span>
        </span>
      </button>
    )
  } else if (value.type === 'color') {
    const c = value.value
    body = (
      <span className="design-vars-color">
        <button type="button" className="design-left-swatch design-vars-color-swatch" style={{ background: colorCss(c) }} aria-label={t('Edit colour')} data-tip={t('Edit colour')} onClick={() => onColor({ variable: v, mode })} />
        <HexField
          value={hexOf(c)}
          onCommit={(text) => {
            const parsed = fromHexOrName(text)
            if (!parsed) return
            const digits = text.trim().replace(/^#/, '')
            const next = digits.length === 8 && /^[0-9a-f]+$/i.test(digits) ? parsed : { ...parsed, a: c.a ?? 1 }
            set(cssHexOf(next))
          }}
        />
        <span className="design-vars-alpha">{Math.round((c.a ?? 1) * 100)}%</span>
      </span>
    )
  } else if (value.type === 'number') {
    body = (
      <NumberField
        value={value.value}
        width="100%"
        onCommit={(edit) => set(edit.kind === 'set' ? edit.value : edit.apply(value.value))}
        scrub={{
          begin: () => {
            scrub.current = designKit.interaction('Edit variable')
          },
          move: (next) => scrub.current?.update('variable.set', { variable: v.id, mode, value: next }),
          end: (cancelled) => {
            const done = scrub.current
            scrub.current = null
            void (cancelled ? done?.cancel() : done?.commit())
          },
        }}
      />
    )
  } else if (value.type === 'string') {
    body = <TextField value={value.value} width="100%" onCommit={(text) => set(text)} />
  } else {
    body = (
      <span style={{ paddingLeft: 6 }}>
        <Checkbox checked={value.value} label={t(value.value ? 'True' : 'False')} onChange={() => set(!value.value)} />
      </span>
    )
  }
  return (
    <span className="design-vars-cell" style={{ left }} data-testid={`design-variables-cell-${v.id}-${mode}`}>
      {overridden ? <span className="design-vars-override" data-tip={t('Overridden here (the collection it extends has another value)')} /> : null}
      <span className="design-vars-cell-body" style={{ right: bodyRight }}>
        {body}
      </span>
      {overridden ? (
        <button type="button" className="design-vars-cell-button" style={{ right: 24 }} aria-label={t('Reset to the inherited value')} data-tip={t('Reset to the inherited value')} onClick={() => void onEdit('variable.resetOverride', { variable: v.id, mode })}>
          <Icon name="Reset" size={14} />
        </button>
      ) : null}
      {value.type === 'alias' ? (
        <button
          type="button"
          className="design-vars-cell-button"
          aria-label={t('Detach alias')}
          data-tip={t('Detach alias')}
          onClick={() => {
            const resolved = resolve(vars, v.id, { [collection.id]: mode })
            if (resolved) set(valueJson(resolved))
          }}
        >
          <Icon name="Detach" size={14} />
        </button>
      ) : (
        <button type="button" className="design-vars-cell-button" aria-label={t('Create alias')} data-tip={t('Create alias')} onClick={(event) => aliasPicker(event.currentTarget.parentElement ?? event.currentTarget)} data-testid={`design-variables-make-alias-${v.id}`}>
          <Icon name="Variable" size={14} />
        </button>
      )}
    </span>
  )
}

/** The hex text of a colour value, edited in place (Enter or a blur commits). */
function HexField({ value, onCommit }: { value: string; onCommit: (text: string) => void }) {
  const [text, setText] = useState(value)
  const [focused, setFocused] = useState(false)
  useEffect(() => {
    if (!focused) setText(value)
  }, [focused, value])
  return (
    <input
      className="design-vars-hex"
      value={text}
      spellCheck={false}
      onFocus={(event) => {
        setFocused(true)
        event.currentTarget.select()
      }}
      onChange={(event) => setText(event.target.value)}
      onBlur={() => {
        setFocused(false)
        if (text !== value) onCommit(text)
      }}
      onKeyDown={(event) => {
        event.stopPropagation()
        if (event.key === 'Enter') event.currentTarget.blur()
        if (event.key === 'Escape') {
          setText(value)
          event.currentTarget.blur()
        }
      }}
    />
  )
}

/** The shared colour picker for a colour value: drags are one step ("Edit variable"). */
function VariableColor({ vars, variable, mode, onClose }: { vars: Variables; variable: Variable; mode: string; onClose: () => void }) {
  const value = vars.variables.find((v) => v.id === variable.id)?.values[mode]
  // Drags are one step, ended even when the picker unmounts mid-drag.
  const write = usePaintWriter('Edit variable')
  if (!value || value.type !== 'color') return null
  const c = { r: value.value.r, g: value.value.g, b: value.value.b, a: value.value.a ?? 1 }
  return (
    <ColorPicker
      paint={solid(c)}
      kinds={false}
      onClose={onClose}
      onChange={(paint, phase) => write('variable.set', { variable: variable.id, mode, value: cssHexOf(editColor(paint)) }, phase, null)}
      testId="design-variables-color"
    />
  )
}

/** "Edit variable": name, description, Hide from publishing, code syntax, and scoping. */
function Details({ vars, id, onClose, onEdit }: { vars: Variables; id: string; onClose: () => void; onEdit: Edit }) {
  const { t } = useDesignKit()
  const v = vars.variables.find((x) => x.id === id)
  if (!v) return null
  const update = (props: Record<string, unknown>) => void onEdit('variable.update', { variable: v.id, ...props })
  const scopes = SCOPES[v.resolvedType]
  const on = scopes.map(([sid]) => sid).filter((sid) => !v.scopes || v.scopes.includes(sid) || v.scopes.includes('ALL_SCOPES'))
  return (
    <div className="design-vars-details design-layer" role="dialog" aria-label={t('Edit variable')} data-testid="design-variables-details">
      <div className="design-vars-details-title">
        <span className="design-s13">{t('Edit variable')}</span>
        <button type="button" className="design-left-header-button" style={{ position: 'static' }} aria-label={t('Close')} onClick={onClose}>
          <Icon name="Close" size={14} />
        </button>
      </div>
      <div className="design-vars-caption">{t('Name')}</div>
      <TextField value={v.name} onCommit={(name) => update({ name })} />
      <div className="design-vars-caption">{t('Description')}</div>
      <TextField value={v.description ?? ''} placeholder={t("What it's for")} onCommit={(description) => update({ description })} />
      <div style={{ height: 6 }} />
      <Checkbox checked={v.hiddenFromPublishing === true} label={t('Hide from publishing')} onChange={(hidden) => update({ hiddenFromPublishing: hidden })} />
      <div className="design-vars-caption" style={{ marginTop: 10 }}>{t('Code syntax')}</div>
      {[
        ['WEB', 'Web'],
        ['iOS', 'iOS'],
        ['ANDROID', 'Android'],
      ].map(([key, label]) => (
        <div key={key} className="design-vars-code">
          <span>{t(label)}</span>
          <TextField value={v.codeSyntax?.[key] ?? ''} width={192} ariaLabel={t(label)} onCommit={(text) => update({ codeSyntax: { [key]: text } })} />
        </div>
      ))}
      {scopes.length ? (
        <>
          <div className="design-vars-caption" style={{ marginTop: 10 }}>{t('Scoping: where pickers offer it')}</div>
          <Checkbox checked={!v.scopes} label={t('All supported properties')} onChange={() => update({ scopes: v.scopes ? null : [] })} />
          {scopes.map(([sid, label]) => (
            <div key={sid} style={{ paddingLeft: 16 }}>
              <Checkbox checked={on.includes(sid)} label={t(label)} onChange={(checked) => update({ scopes: checked ? [...on, sid] : on.filter((x) => x !== sid) })} />
            </div>
          ))}
        </>
      ) : null}
    </div>
  )
}

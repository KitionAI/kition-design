/**
 * The Edit and Object dialogs (2.7 §4.3-§4.5): Find and replace, Rename layers (batch rename) and
 * the menu input dialogs (Create link, Offset path, Move to page).
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Checkbox, NumberField } from '../ui'
import { Button, Caption, Dialog } from './Dialog'
import { batchRename } from './rename'

/** A plain text input (egui `TextEdit`): Enter runs `onEnter`. */
function Input({ value, onChange, placeholder, width, onEnter, autoFocus, testId }: { value: string; onChange: (value: string) => void; placeholder: string; width: number | string; onEnter?: () => void; autoFocus?: boolean; testId?: string }) {
  return (
    <div className="design-field design-field-text" style={{ width }}>
      <input
        value={value}
        placeholder={placeholder}
        aria-label={placeholder}
        spellCheck={false}
        autoComplete="off"
        autoFocus={autoFocus}
        onChange={(event) => onChange(event.target.value)}
        onFocus={(event) => event.currentTarget.select()}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && onEnter) {
            event.preventDefault()
            onEnter()
          }
        }}
        data-testid={testId}
      />
    </div>
  )
}

/** Find and replace (§4.3): upstream's plain egui window over the canvas's top-left, 338 wide; Find selects matches (layer names too), Replace all changes text only. */
export function FindReplaceDialog({ onClose }: { onClose: () => void }) {
  const { t, run, toast } = useDesignKit()
  const [find, setFind] = useState('')
  const [replace, setReplace] = useState('')
  const go = async (replacing: boolean) => {
    if (!find) return
    const answer = await run<{ matches?: unknown[] }>('edit.findReplace', replacing ? { find, replace, layerNames: false } : { find, layerNames: true })
    if (answer) {
      const n = Array.isArray(answer.matches) ? answer.matches.length : 0
      toast(t('{n} matches', { n }))
    }
  }
  // Upstream anchors it at (260, 120) in the window: 80 below the tab strip, here under the top bar.
  const top = (document.querySelector('.design-shell')?.getBoundingClientRect().top ?? 37) + 80
  return (
    <Dialog
      title={t('Find and replace')}
      width={312}
      plain
      placement={{ kind: 'at', left: 260, top }}
      onClose={onClose}
      testId="design-dialog-find"
      footer={
        <>
          <button type="button" className="design-dialog-plain-button" onClick={() => void go(false)} data-testid="design-find-find">
            {t('Find')}
          </button>
          <button type="button" className="design-dialog-plain-button" onClick={() => void go(true)} data-testid="design-find-replace-all">
            {t('Replace all')}
          </button>
        </>
      }
    >
      <Input value={find} onChange={setFind} placeholder={t('Find')} width={260} onEnter={() => void go(false)} testId="design-find-input" />
      <Input value={replace} onChange={setReplace} placeholder={t('Replace with')} width={260} testId="design-replace-input" />
    </Dialog>
  )
}

type LayerRow = { id: number; name: string }

/** The layers' names in stacking order, bottom first (the order `object.batchRename` numbers them in). */
function useStackedNames(ids: readonly number[]): LayerRow[] | null {
  const { query, revision } = useDesignKit()
  const [rows, setRows] = useState<LayerRow[] | null>(null)
  const key = ids.join(',')
  useEffect(() => {
    let live = true
    type Node = { id: number; name: string; children?: Node[] }
    void query<{ rows?: Node[] }>('layers', { all: true })
      .then((answer) => {
        // The Layers list, top first, in tree order.
        const order = new Map<number, number>()
        const named = new Map<number, string>()
        const walk = (nodes: Node[] | undefined) => {
          for (const node of nodes ?? []) {
            order.set(node.id, order.size)
            named.set(node.id, node.name)
            walk(node.children)
          }
        }
        walk(answer.rows)
        // Bottom of the stack first.
        const sorted = [...ids].sort((a, b) => (order.get(b) ?? 0) - (order.get(a) ?? 0))
        if (live) setRows(sorted.map((id) => ({ id, name: named.get(id) ?? '' })))
      })
      .catch(() => live && setRows(ids.map((id) => ({ id, name: '' }))))
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, revision])
  return rows
}

const PREVIEW_ROWS = 5

/** Rename layers (§4.4): a match (or none: the whole name), a pattern with codes, a preview, Rename. */
export function BatchRenameDialog({ ids, onClose }: { ids: number[]; onClose: () => void }) {
  const { t, run } = useDesignKit()
  const layers = useStackedNames(ids)
  const [find, setFind] = useState('')
  const [regex, setRegex] = useState(false)
  const [to, setTo] = useState('')
  const [start, setStart] = useState(1)
  const target = useRef<HTMLDivElement>(null)
  const plan = useMemo(() => batchRename(find, regex, to, start), [find, regex, start, to])
  const ready = !('error' in plan) && !(find === '' && to === '')
  const apply = async () => {
    if (!ready) return
    const params = find ? { ids, regex, start, find, replace: to } : { ids, regex, start, name: to }
    if (await run('object.batchRename', params)) onClose()
  }
  const append = (code: string) => {
    setTo((now) => now + code)
    target.current?.querySelector('input')?.focus()
  }
  const n = ids.length
  return (
    <Dialog
      title={t('Rename layers')}
      width={320}
      onClose={onClose}
      testId="design-dialog-rename"
      footer={
        <>
          <Button onClick={onClose} testId="design-rename-cancel">
            {t('Cancel')}
          </Button>
          <Button variant="primary" disabled={!ready} onClick={() => void apply()} testId="design-rename-apply">
            {t('Rename')}
          </Button>
        </>
      }
    >
      <Caption tone="secondary" style={{ marginBottom: 6 }}>
        {t('{n} layers', { n })}
      </Caption>
      <Input value={find} onChange={setFind} placeholder={t('Match (empty: the whole name)')} width={300} testId="design-rename-find" />
      <Checkbox checked={regex} label={t('Regular expression')} onChange={setRegex} testId="design-rename-regex" />
      <div ref={target}>
        <Input value={to} onChange={setTo} placeholder={t('Rename to')} width={300} onEnter={() => void apply()} testId="design-rename-to" />
      </div>
      <div className="design-dialog-row">
        {/* egui's plain buttons: frameless until hovered. */}
        <button type="button" className="design-plain-button" onClick={() => append('$&')}>
          {t('Current name')}
        </button>
        <button type="button" className="design-plain-button" onClick={() => append('$n')}>
          {t('Number ↑')}
        </button>
        <button type="button" className="design-plain-button" onClick={() => append('$N')}>
          {t('Number ↓')}
        </button>
      </div>
      <div className="design-dialog-row">
        <label>{t('Start ascending from / stop descending at')}</label>
        <span className="design-drag-value">
          <NumberField value={start} min={0} max={1_000_000} width={56} ariaLabel={t('Start ascending from / stop descending at')} onCommit={(edit) => setStart(Math.round(edit.kind === 'set' ? edit.value : edit.apply(start)))} testId="design-rename-start" />
        </span>
      </div>
      <div style={{ marginTop: 2, display: 'flex', flexDirection: 'column', gap: 4 }} data-testid="design-rename-preview">
        {'error' in plan ? (
          <Caption tone="danger">{plan.error}</Caption>
        ) : !ready ? null : (
          (layers ?? []).slice(0, PREVIEW_ROWS).map((layer, k) => (
            <Caption key={layer.id}>
              <span style={{ color: 'var(--design-text-secondary)' }}>{layer.name}</span> → {plan.apply(layer.name, k, n)}
            </Caption>
          ))
        )}
        {ready && n > PREVIEW_ROWS ? <Caption tone="secondary">{t('and {n} more', { n: n - PREVIEW_ROWS })}</Caption> : null}
      </div>
    </Dialog>
  )
}

/** Create link (§4.5): a URL and Done (`text.link {url}`). */
export function LinkDialog({ onClose }: { onClose: () => void }) {
  const { t, run, strings } = useDesignKit()
  const [url, setUrl] = useState('')
  const ok = url.trim() !== ''
  const done = async () => {
    if (ok && (await run('text.link', { url: url.trim() }))) onClose()
  }
  return (
    <Dialog title={strings.menuLabel('text.link', 'Create link')} width={320} onClose={onClose} testId="design-dialog-link">
      <div className="design-dialog-row">
        <Input value={url} onChange={setUrl} placeholder="https://" width={250} onEnter={() => void done()} testId="design-link-url" />
        <Button variant="primary" disabled={!ok} onClick={() => void done()} testId="design-link-done">
          {t('Done')}
        </Button>
      </div>
    </Dialog>
  )
}

/** Offset path (§4.5): an amount (±100000) and Offset (`vector.offset {offset}`). */
export function OffsetDialog({ onClose }: { onClose: () => void }) {
  const { t, run, strings } = useDesignKit()
  const [offset, setOffset] = useState(0)
  const clamp = (value: number) => Math.max(-100_000, Math.min(100_000, value))
  return (
    <Dialog title={strings.menuLabel('vector.offset', 'Offset path')} width={320} onClose={onClose} testId="design-dialog-offset">
      <div className="design-dialog-row">
        <NumberField value={offset} width={120} ariaLabel={t('Offset')} onCommit={(edit) => setOffset(clamp(edit.kind === 'set' ? edit.value : edit.apply(offset)))} testId="design-offset-value" />
        <Button
          variant="primary"
          onClick={() => {
            void run('vector.offset', { offset }).then((answer) => answer && onClose())
          }}
          testId="design-offset-apply"
        >
          {t('Offset')}
        </Button>
      </div>
    </Dialog>
  )
}

/** Move to page (§4.5): one button per other page (dividers skipped). */
export function MoveToPageDialog({ onClose }: { onClose: () => void }) {
  const { t, run, strings } = useDesignKit()
  const pages = useQuery<{ current: number; pages: Array<{ id: number; name: string; divider: boolean }> }>('pages')
  const others = (pages?.pages ?? []).filter((page) => page.id !== pages?.current && !page.divider)
  return (
    <Dialog title={strings.menuLabel('object.moveToPage', 'Move to page')} width={320} onClose={onClose} testId="design-dialog-move">
      {pages && !others.length ? <Caption tone="secondary">{t('No other pages')}</Caption> : null}
      {/* One button per other page, stacked (egui's vertical layout, 6 apart). */}
      {others.length ? (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-start', gap: 6 }}>
          {others.map((page) => (
            <Button key={page.id} onClick={() => void run('object.moveToPage', { page: page.id }).then((answer) => answer && onClose())} testId={`design-move-page-${page.id}`}>
              {page.name}
            </Button>
          ))}
        </div>
      ) : null}
    </Dialog>
  )
}


/**
 * The font family picker (upstream `typography/picker.rs`): a search field (focused, cleared on
 * open), then "Recent" (up to eight families used lately) and "All fonts", or the families matching
 * the search; each family drawn in its own font (`textPreview`), a check on the current one. Up and
 * Down move a highlight (from the current family), Enter picks it or the first match; hovering or
 * highlighting a family previews it on the canvas (`text.preview`), closing without a pick ends the
 * preview (`text.endPreview`).
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Popover } from '../ui'

const RECENT_KEY = 'kition-design.recentFonts'
const ROW = 26
/** Rows sit 6 apart (the menu's item spacing). */
const PITCH = 32
const LIST_HEIGHT = 380
const previewScale = () => Math.min(3, Math.max(1, Math.round(window.devicePixelRatio || 1)))

export function recentFamilies(): string[] {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]') as unknown
    return Array.isArray(raw) ? raw.filter((f): f is string => typeof f === 'string') : []
  } catch {
    return []
  }
}

export function rememberFamily(family: string) {
  const next = [family, ...recentFamilies().filter((f) => f.toLowerCase() !== family.toLowerCase())].slice(0, 8)
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next))
  } catch {
    // Storage off: the list lives for the page only.
  }
}

/** Family previews (`textPreview` PNGs), by family and colour, as object URLs. */
const previews = new Map<string, Promise<string | null>>()

export function useFamilyPreview(family: string, color: string, size = 13): string | null {
  const { invoke } = useDesignKit()
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    const key = `${family}\u0000${color}\u0000${size}`
    let answer = previews.get(key)
    if (!answer) {
      const scale = previewScale()
      answer = invoke<Uint8Array>('textPreview', JSON.stringify({ family, size, color, scale, maxWidth: 400, maxHeight: 24 }))
        .then((bytes) => URL.createObjectURL(new Blob([bytes as BlobPart], { type: 'image/png' })))
        .catch(() => null)
      previews.set(key, answer)
    }
    void answer.then((u) => {
      if (live) setUrl(u)
    })
    return () => {
      live = false
    }
  }, [color, family, invoke, size])
  return url
}

type Row = { kind: 'header'; label: string } | { kind: 'family'; family: string }

function FamilyRow({ family, current, hot, onPick, onHover }: { family: string; current: boolean; hot: boolean; onPick: () => void; onHover: () => void }) {
  const ink = hot ? '#07352A' : getComputedStyle(document.documentElement).getPropertyValue('--design-text').trim() || '#1E1E1E'
  const url = useFamilyPreview(family, ink.length === 7 ? ink : '#1E1E1E')
  return (
    <div className="design-font-row" data-hot={hot || undefined} role="option" aria-selected={current} data-testid={`design-font-${family}`} onPointerEnter={onHover} onClick={onPick}>
      {current ? (
        <span className="design-font-check">
          <Icon name="Check" size={12} />
        </span>
      ) : null}
      {url ? (
        <img
          className="design-font-preview"
          src={url}
          alt={family}
          draggable={false}
          onLoad={(event) => {
            // The preview is drawn at the display's scale: shown at its size in CSS pixels.
            const img = event.currentTarget
            img.style.width = `${img.naturalWidth / previewScale()}px`
          }}
        />
      ) : (
        <span className="design-font-name">{family}</span>
      )}
    </div>
  )
}

export function FontPicker({ anchor, current, onPick, onPreview, onClose, onApplyVariable }: { anchor: Element; current: string | null; onPick: (family: string) => void; onPreview: (family: string) => void; onClose: () => void; onApplyVariable?: () => void }) {
  const { t } = useDesignKit()
  const [query, setQuery] = useState('')
  const [hi, setHi] = useState<number | null>(null)
  const [scroll, setScroll] = useState(0)
  const list = useRef<HTMLDivElement>(null)
  const answer = useQuery<{ families: Array<{ family: string }> }>('fontFamilies')
  const all = useMemo(() => (answer?.families ?? []).map((f) => f.family), [answer])
  const rows: Row[] = useMemo(() => {
    const q = query.trim().toLowerCase()
    const matches = all.filter((f) => !q || f.toLowerCase().includes(q))
    const out: Row[] = []
    if (!q) {
      const recent = recentFamilies().filter((f) => all.some((a) => a.toLowerCase() === f.toLowerCase()))
      if (recent.length) {
        out.push({ kind: 'header', label: t('Recent') }, ...recent.map((family) => ({ kind: 'family' as const, family })), { kind: 'header', label: t('All fonts') })
      }
    }
    out.push(...matches.map((family) => ({ kind: 'family' as const, family })))
    return out
  }, [all, query, t])
  const familyAt = (i: number | null) => (i !== null && rows[i]?.kind === 'family' ? (rows[i] as { family: string }).family : null)
  const hasMatch = rows.some((r) => r.kind === 'family')

  const step = (by: number) => {
    const currentAt = current ? rows.map((r, i) => (r.kind === 'family' && r.family.toLowerCase() === current.toLowerCase() ? i : -1)).filter((i) => i >= 0).pop() ?? -1 : -1
    let from = hi ?? currentAt
    if (from < 0) from = by > 0 ? -1 : rows.length
    let at = from + by
    while (at >= 0 && at < rows.length && rows[at].kind !== 'family') at += by
    if (at < 0 || at >= rows.length) return
    setHi(at)
    const fam = familyAt(at)
    if (fam) onPreview(fam)
    // Scrolled just enough to show it.
    const el = list.current
    if (el) {
      const top = at * PITCH
      if (top < el.scrollTop) el.scrollTop = top
      else if (top + ROW > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW - el.clientHeight
    }
  }

  // Virtualised: only the rows in view (and a few more) are drawn.
  const first = Math.max(0, Math.floor(scroll / PITCH) - 4)
  const last = Math.min(rows.length, first + Math.ceil(LIST_HEIGHT / PITCH) + 8)
  return (
    <Popover open anchor={anchor} placement="below-end" gap={0} width={Math.max(208, 240)} onClose={onClose} className="design-font-picker" testId="design-font-picker" label={t('Font')}>
      {onApplyVariable ? (
        <>
          <button type="button" className="design-font-apply" onClick={onApplyVariable} data-testid="design-fontFamily-variable">
            {t('Apply variable')}
          </button>
          <div className="design-menu-separator" />
        </>
      ) : null}
      <input
        className="design-font-search"
        autoFocus
        placeholder={t('Search fonts')}
        value={query}
        spellCheck={false}
        data-testid="design-font-search"
        onChange={(event) => {
          setQuery(event.target.value)
          setHi(null)
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            step(event.key === 'ArrowDown' ? 1 : -1)
          } else if (event.key === 'Enter') {
            const pick = familyAt(hi) ?? (rows.find((r) => r.kind === 'family') as { family: string } | undefined)?.family
            if (pick) onPick(pick)
          } else return
          event.stopPropagation()
        }}
      />
      {!hasMatch ? (
        <div className="design-font-empty">{t('No fonts match')}</div>
      ) : (
        <div ref={list} className="design-font-list" style={{ maxHeight: LIST_HEIGHT }} role="listbox" onScroll={(event) => setScroll(event.currentTarget.scrollTop)}>
          <div style={{ height: Math.max(0, rows.length * PITCH - (PITCH - ROW)), position: 'relative' }}>
            {rows.slice(first, last).map((row, k) => {
              const i = first + k
              return (
                <div key={`${i}-${row.kind === 'family' ? row.family : row.label}`} style={{ position: 'absolute', top: i * PITCH, left: 0, right: 0 }}>
                  {row.kind === 'header' ? (
                    <div className="design-font-header">{row.label}</div>
                  ) : (
                    <FamilyRow
                      family={row.family}
                      current={current !== null && current.toLowerCase() === row.family.toLowerCase()}
                      hot={hi === i}
                      onHover={() => {
                        setHi(i)
                        onPreview(row.family)
                      }}
                      onPick={() => onPick(row.family)}
                    />
                  )}
                </div>
              )
            })}
          </div>
        </div>
      )}
    </Popover>
  )
}

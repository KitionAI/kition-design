/**
 * Dev Mode's own small widgets: a dropdown whose label is a prompt ("Add measurement"), the
 * inspected property row (copy on click), a multiline note field, and thumbnails from the engine.
 */
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { Icon, type IconName } from '../icons/Icon'
import { useDesignKit } from '../kit/hooks'
import { Menu, type MenuEntry } from '../ui'
import { copyText } from './store'
import { releaseField } from '../kit/focus'

/** An inspected property (`inspect.properties` rows, pinned annotation values). */
export type Prop = { label: string; value: string; color?: string; token?: string }

/** A dropdown button with any label (`widgets::dropdown` with a lead icon or colour dot) that opens `items`. */
export function MenuDropdown({ label, items, icon, dot, width, menuWidth, tip, testId, muted }: { label: string; items: () => MenuEntry[]; icon?: IconName; dot?: string; width?: CSSProperties['width']; menuWidth?: number; tip?: string; testId?: string; muted?: boolean }) {
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button ref={button} type="button" className="design-dropdown" style={{ width }} aria-haspopup="menu" aria-expanded={open} data-tip={tip} data-testid={testId} onClick={() => setOpen((now) => !now)}>
        {dot ? <span className="design-dropdown-dot" style={{ background: dot }} /> : null}
        {icon ? (
          <span className="design-dropdown-lead">
            <Icon name={icon} size={14} />
          </span>
        ) : null}
        <span className="design-dropdown-label" data-mixed={muted || undefined}>
          {label}
        </span>
        <Icon name="Chevron" size={12} className="design-chevron" />
      </button>
      {open ? <Menu tone="light" items={items()} anchor={button.current} placement="below-start" gap={4} minWidth={menuWidth ?? button.current?.offsetWidth} onClose={() => setOpen(false)} testId={testId ? `${testId}-menu` : undefined} /> : null}
    </>
  )
}

/** A colour swatch (`paint_swatch`): the left half opaque, the right half with the colour's alpha over a checkerboard. */
export function Swatch({ color }: { color: string }) {
  const opaque = /^#[0-9a-f]{8}$/i.test(color) ? color.slice(0, 7) : color
  return <span className="design-dev-swatch" style={{ ['--swatch' as string]: color, ['--swatch-opaque' as string]: opaque }} />
}

/** A property row (`prop_row`): label, value with a swatch, the token as a chip; a click copies the value. */
export function PropRow({ prop, testId }: { prop: Prop; testId?: string }) {
  const { t, toast } = useDesignKit()
  return (
    <button
      type="button"
      className="design-dev-prop"
      data-tip={t('Copy {value}', { value: prop.value })}
      data-testid={testId}
      onClick={() => {
        void copyText(prop.value)
        toast(t('Copied {value}', { value: t(prop.label).toLowerCase() }))
      }}
    >
      <span className="design-dev-prop-label">{t(prop.label)}</span>
      <span className="design-dev-prop-value">
        {prop.color ? <Swatch color={prop.color} /> : null}
        <span className="design-dev-prop-text">{prop.value}</span>
      </span>
      {prop.token ? <span className="design-dev-chip"><span>{prop.token}</span></span> : null}
    </button>
  )
}

/**
 * A multiline field (`multiline_field`): `rows` tall, commits when it loses focus or on
 * Cmd/Ctrl+Enter; Escape puts the value back.
 */
export function NoteField({ value, placeholder, rows, onCommit, testId, clearOnCommit = false }: { value: string; placeholder: string; rows: number; onCommit: (value: string) => void; testId?: string; clearOnCommit?: boolean }) {
  const [text, setText] = useState(value)
  const focused = useRef(false)
  const cancelled = useRef(false)
  useEffect(() => {
    if (!focused.current) setText(value)
  }, [value])
  return (
    <div className="design-dev-note-field">
      <textarea
        rows={rows}
        style={rows > 2 ? { minHeight: rows * 13 + 12 } : undefined}
        value={text}
        placeholder={placeholder}
        aria-label={placeholder}
        spellCheck={false}
        data-testid={testId}
        onFocus={() => {
          focused.current = true
        }}
        onChange={(event) => setText(event.target.value)}
        onBlur={() => {
          focused.current = false
          if (!cancelled.current && text !== value) onCommit(text)
          cancelled.current = false
          if (clearOnCommit) setText('')
        }}
        onKeyDown={(event) => {
          event.stopPropagation()
          if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) releaseField(event.currentTarget)
          else if (event.key === 'Escape') {
            cancelled.current = true
            setText(value)
            releaseField(event.currentTarget)
          }
        }}
      />
    </div>
  )
}

const thumbs = new Map<string, Promise<string | null>>()

/** Layer `id` drawn by the engine (`nodeThumbnail`) as an object URL, cached per revision. */
export function useThumbnail(id: number | null, size: number): string | null {
  const { invoke, revision, doc } = useDesignKit()
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    if (id == null || !doc) return undefined
    let live = true
    const key = `${revision}:${id}:${size}`
    let answer = thumbs.get(key)
    if (!answer) {
      answer = invoke<unknown>('nodeThumbnail', String(id), size * (window.devicePixelRatio || 1)).then(
        (bytes) => URL.createObjectURL(new Blob([toBytes(bytes) as BlobPart], { type: 'image/png' })),
        () => null,
      )
      thumbs.set(key, answer)
      if (thumbs.size > 400) thumbs.delete(thumbs.keys().next().value as string)
    }
    void answer.then((value) => {
      if (live) setUrl(value)
    })
    return () => {
      live = false
    }
  }, [doc, id, invoke, revision, size])
  return url
}

export function toBytes(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) return value
  if (value instanceof ArrayBuffer) return new Uint8Array(value)
  if (Array.isArray(value)) return new Uint8Array(value as number[])
  return new Uint8Array()
}

/** A row with two lines of text, a lead icon and trailing buttons (dev resources, measurements). */
export function TwoLineRow({ icon, title, sub, onClick, tip, accent, children, testId, reserve = 16 }: { icon: IconName; title: string; sub: string; onClick?: () => void; tip?: string; accent?: boolean; children?: ReactNode; testId?: string; /** Pixels kept free after the text, from the row's right edge (upstream's elide width). */ reserve?: number }) {
  return (
    <div className="design-dev-row2" data-testid={testId} data-tip={tip} onClick={onClick} role={onClick ? 'button' : undefined}>
      <span className="design-dev-row2-icon">
        <Icon name={icon} size={14} />
      </span>
      {/* The row's content box is its width − 25 (16 left, 9 right); the text starts 38 from the row's left. */}
      <span className="design-dev-row2-text" style={{ maxWidth: `calc(100% - ${13 + reserve}px)` }}>
        <span className="design-dev-row2-title" data-accent={accent || undefined}>
          {title}
        </span>
        <span className="design-dev-row2-sub">{sub}</span>
      </span>
      {children ? (
        <span className="design-dev-row2-buttons" onClick={(event) => event.stopPropagation()}>
          {children}
        </span>
      ) : null}
    </div>
  )
}

/**
 * The small standard controls (2.1 §7): icon buttons and icon toggles, buttons, the segmented
 * toggle group, checkbox and radio, the section header, rows, sublabels, separators, notes, and the
 * disclosure.
 */
import type { CSSProperties, MouseEvent, ReactNode } from 'react'
import { Icon, type IconName } from '../icons/Icon'

export type IconButtonProps = {
  icon: IconName
  /** The tooltip and accessible name ("Label  ⇧L" for tools). */
  label: string
  onClick?: (event: MouseEvent<HTMLButtonElement>) => void
  /** An icon toggle's state (`selected_row` fill, `accent_text` glyph). */
  pressed?: boolean
  disabled?: boolean
  /** The box (24 by default); the glyph is 14 in a 24 box, 16 in larger toggles, 12 below 22. */
  size?: number
  iconSize?: number
  testId?: string
  className?: string
}

/** A 24 × 24 icon button (r5, `hover` fill), or an icon toggle when `pressed` is given. */
export function IconButton({ icon, label, onClick, pressed, disabled, size = 24, iconSize, testId, className = '' }: IconButtonProps) {
  const glyph = iconSize ?? (pressed === undefined ? 14 : size < 22 ? 12 : size > 24 ? 16 : 14)
  return (
    <button
      type="button"
      className={`design-icon-button ${className}`}
      style={size === 24 ? undefined : { width: size, height: size }}
      aria-label={label.split('  ')[0]}
      aria-pressed={pressed}
      data-tip={label}
      disabled={disabled}
      onClick={onClick}
      data-testid={testId}
    >
      <Icon name={icon} size={glyph} />
    </button>
  )
}

export type ButtonProps = {
  children: ReactNode
  variant?: 'primary' | 'secondary'
  /** Full width minus the panel's insets (the 40 px row's wide button). */
  wide?: boolean
  disabled?: boolean
  onClick?: () => void
  tip?: string
  testId?: string
  style?: CSSProperties
}

export function Button({ children, variant = 'secondary', wide, disabled, onClick, tip, testId, style }: ButtonProps) {
  return (
    <button type="button" className="design-button" data-variant={variant} data-wide={wide || undefined} disabled={disabled} onClick={onClick} data-tip={tip} data-testid={testId} style={style}>
      {children}
    </button>
  )
}

export type Segment<V extends string> = { value: V; label: string; icon?: IconName }

/**
 * The segmented control (`segmented`, `segmented_text`): icon cells of `cell` × 24 on a `field`
 * background, the active one raised (`panel` + 1 px `border`, r4); text cells share the width.
 */
export function Segmented<V extends string>({ value, segments, onChange, cell = 24, text = false, disabled, testId, ariaLabel }: { value: V | null; segments: Segment<V>[]; onChange: (value: V) => void; cell?: number; text?: boolean; disabled?: boolean; testId?: string; ariaLabel?: string }) {
  return (
    <div className="design-segmented" role="group" aria-label={ariaLabel} data-text={text || undefined} data-testid={testId}>
      {segments.map((segment) => (
        <button
          key={segment.value}
          type="button"
          className="design-segment"
          style={text ? undefined : { width: cell }}
          aria-pressed={segment.value === value}
          aria-label={segment.label}
          data-tip={text ? undefined : segment.label}
          disabled={disabled}
          onClick={() => onChange(segment.value)}
          data-testid={testId ? `${testId}-${segment.value}` : undefined}
        >
          {text || !segment.icon ? <span>{segment.label}</span> : <Icon name={segment.icon} size={14} />}
        </button>
      ))}
    </div>
  )
}

/** A checkbox row (box 14 r3, label at +22; on: `accent` with an `accent_on` check). `checked: 'mixed'` shows a dash. */
export function Checkbox({ checked, label, onChange, disabled, testId }: { checked: boolean | 'mixed'; label: string; onChange: (checked: boolean) => void; disabled?: boolean; testId?: string }) {
  return (
    <button type="button" role="checkbox" className="design-check" aria-checked={checked} disabled={disabled} onClick={() => onChange(checked !== true)} data-testid={testId}>
      <span className="design-check-box">{checked === true ? <Icon name="Check" size={11} /> : checked === 'mixed' ? <Icon name="Minus" size={11} /> : null}</span>
      <span>{label}</span>
    </button>
  )
}

/** A radio row (circle r7, label at +22). */
export function Radio({ checked, label, onChange, disabled, testId }: { checked: boolean; label: string; onChange: () => void; disabled?: boolean; testId?: string }) {
  return (
    <button type="button" role="radio" className="design-check" aria-checked={checked} disabled={disabled} onClick={onChange} data-testid={testId}>
      <span className="design-radio-dot" />
      <span>{label}</span>
    </button>
  )
}

export type HeaderAction = { icon: IconName; label: string; onClick: () => void; pressed?: boolean; testId?: string }

/**
 * The section header (`section_header`): 40 tall, the title `semibold(11)` at +16, 24 × 24 action
 * buttons from the right. A `dim` (empty) section's title is `text_secondary` and a click anywhere
 * on it runs its first action (its "+").
 */
export function SectionHeader({ title, actions = [], dim = false, dimAction = 0, onContextMenu, testId, children }: { title: ReactNode; actions?: HeaderAction[]; dim?: boolean; /** The action a dim header's click runs (its "+"; the first by default). */ dimAction?: number; onContextMenu?: (event: MouseEvent<HTMLDivElement>) => void; testId?: string; children?: ReactNode }) {
  const add = dim ? actions[dimAction] : undefined
  return (
    <div className="design-section-header" data-dim={dim || undefined} onClick={add ? () => add.onClick() : undefined} onContextMenu={onContextMenu} data-testid={testId}>
      <span className="design-section-header-title">{title}</span>
      {children}
      <span className="design-section-header-actions" onClick={(event) => event.stopPropagation()}>
        {actions.map((action) => (
          <IconButton key={action.label} icon={action.icon} label={action.label} pressed={action.pressed} onClick={action.onClick} testId={action.testId} />
        ))}
      </span>
    </div>
  )
}

/** A property row: full width × `height` (32), content inset 16, items 8 apart. */
export function Row({ children, height = 32, style, testId }: { children: ReactNode; height?: number; style?: CSSProperties; testId?: string }) {
  return (
    <div className="design-row" style={{ minHeight: height, ...style }} data-testid={testId}>
      {children}
    </div>
  )
}

/**
 * Sublabels above a row of two fields (`sublabels`): 18 tall, `regular(10)` `text_secondary`; the
 * second starts at the second column (`col` + 8).
 */
export function SubLabels({ labels, column }: { labels: string[]; column?: number }) {
  return (
    <div className="design-sublabels">
      {labels.map((label, index) => (
        <span key={index} style={index === 0 && column ? { width: column, flex: 'none' } : undefined} data-tip={label.length > 14 ? label : undefined}>
          {label}
        </span>
      ))}
    </div>
  )
}

export function Separator() {
  return <hr className="design-separator" />
}

/** A wrapped note (`note`): padding 16 × 6, `regular(11)`. */
export function Note({ children }: { children: ReactNode }) {
  return <div className="design-note">{children}</div>
}

/** A disclosure (`disclosure`): a chevron and a `text_secondary` label that toggles. */
export function Disclosure({ open, label, onToggle, testId }: { open: boolean; label: string; onToggle: () => void; testId?: string }) {
  return (
    <button type="button" className="design-disclosure" aria-expanded={open} onClick={onToggle} data-testid={testId}>
      <Icon name={open ? 'Chevron' : 'ChevronRight'} size={10} />
      {label}
    </button>
  )
}

/** The two-column field width at panel width `w` (`col_width`): 84 at 240. */
export function columnWidth(w: number) {
  return Math.max((w - 32 - 8 - 32) / 2, 40)
}

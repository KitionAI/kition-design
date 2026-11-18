/**
 * The dropdown (2.1 §7 `dropdown`): a field-coloured button with the value (an optional lead icon
 * or colour dot) and a chevron at the right; it opens a light menu of the options.
 */
import { useRef, useState, type CSSProperties } from 'react'
import { Icon, type IconName } from '../icons/Icon'
import { Menu, type MenuEntry } from './Menu'

export type DropdownOption<V extends string> = { value: V; label: string; icon?: IconName; shortcut?: string; disabled?: boolean } | { separator: true }

export type DropdownProps<V extends string> = {
  value: V | null
  options: DropdownOption<V>[]
  onChange: (value: V) => void
  /** Shown when `value` is null (several values): "Mixed". */
  mixedLabel?: string
  /** A lead icon (14 px) or colour dot. */
  icon?: IconName
  dot?: string
  width?: CSSProperties['width']
  /** The menu's width at least (200 typical); the button's width by default. */
  menuWidth?: number
  disabled?: boolean
  ariaLabel?: string
  testId?: string
  /** Check the current option in the menu (`menu_check`); upstream's plain `menu_item` menus pass false. */
  checks?: boolean
}

export function Dropdown<V extends string>({ value, options, onChange, mixedLabel = 'Mixed', icon, dot, width, menuWidth, disabled, ariaLabel, testId, checks = true }: DropdownProps<V>) {
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const chosen = options.find((option): option is Exclude<DropdownOption<V>, { separator: true }> => !('separator' in option) && option.value === value)
  const items: MenuEntry[] = options.map((option) =>
    'separator' in option
      ? { type: 'separator' }
      : { label: option.label, checked: checks ? option.value === value : undefined, disabled: option.disabled, shortcut: option.shortcut, onSelect: () => onChange(option.value), testId: testId ? `${testId}-${option.value}` : undefined },
  )
  return (
    <>
      <button
        ref={button}
        type="button"
        className="design-dropdown"
        style={{ width }}
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel}
        data-tip={chosen && chosen.label.length > 18 ? chosen.label : undefined}
        data-testid={testId}
        onClick={() => setOpen((now) => !now)}
      >
        {dot ? <span className="design-dropdown-dot" style={{ background: dot }} /> : null}
        {icon ? (
          <span className="design-dropdown-lead">
            <Icon name={icon} size={14} />
          </span>
        ) : null}
        <span className="design-dropdown-label" data-mixed={!chosen || undefined}>
          {chosen ? chosen.label : mixedLabel}
        </span>
        <Icon name="Chevron" size={12} className="design-chevron" />
      </button>
      {open ? <Menu tone="light" items={items} anchor={button.current} placement="below-start" gap={0} minWidth={menuWidth ?? button.current?.offsetWidth} onClose={() => setOpen(false)} label={ariaLabel} testId={testId ? `${testId}-menu` : undefined} /> : null}
    </>
  )
}

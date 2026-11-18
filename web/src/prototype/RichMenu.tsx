/**
 * A light popup menu whose rows draw their own content (upstream `menu_item_with`): the easing
 * menu's curve thumbnails, the device colour menu's swatches, a scrolling list of destinations.
 * Same frame and rows as `Menu tone="light"`; arrows, Enter and Escape work.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Icon } from '../icons/Icon'
import { Layer } from '../ui'
import type { Placement } from '../ui/position'

export type RichItem = { key: string; render: (active: boolean) => ReactNode; onSelect: () => void; height?: number; testId?: string } | { key: string; separator: true }

export function RichMenu({ anchor, items, onClose, width, maxHeight, placement = 'below-start', label, testId }: { anchor: Element | null; items: RichItem[]; onClose: () => void; width?: number; maxHeight?: number; placement?: Placement; label?: string; testId?: string }) {
  const [active, setActive] = useState(-1)
  const list = useRef<HTMLDivElement>(null)
  const choices = items.map((item, index) => ('separator' in item ? -1 : index)).filter((index) => index >= 0)
  useEffect(() => {
    list.current?.focus()
  }, [])
  const choose = (index: number) => {
    const item = items[index]
    if (!item || 'separator' in item) return
    onClose()
    item.onSelect()
  }
  return (
    <Layer anchor={anchor} placement={placement} gap={4} onClose={onClose} testId={testId} role="menu" label={label}>
      <div
        ref={list}
        className="design-menu design-rich-menu"
        data-tone="light"
        tabIndex={-1}
        style={{ minWidth: width ?? (anchor instanceof HTMLElement ? anchor.offsetWidth : undefined), maxHeight, overflowY: maxHeight ? 'auto' : undefined }}
        onKeyDown={(event) => {
          const at = choices.indexOf(active)
          if (event.key === 'ArrowDown') setActive(choices[Math.min(choices.length - 1, at + 1)] ?? -1)
          else if (event.key === 'ArrowUp') setActive(choices[Math.max(0, at - 1)] ?? -1)
          else if (event.key === 'Enter' || event.key === ' ') choose(active)
          else return
          event.preventDefault()
          event.stopPropagation()
        }}
      >
        {items.map((item, index) =>
          'separator' in item ? (
            <div key={item.key} className="design-menu-separator" />
          ) : (
            <button
              key={item.key}
              type="button"
              role="menuitem"
              className="design-menu-row"
              data-active={index === active || undefined}
              style={item.height ? { height: item.height } : undefined}
              onPointerEnter={() => setActive(index)}
              onPointerLeave={() => setActive(-1)}
              onClick={() => choose(index)}
              data-testid={item.testId}
            >
              {item.render(index === active)}
            </button>
          ),
        )}
      </div>
    </Layer>
  )
}

/** A dropdown button (`dropdown`) that opens a `RichMenu`. */
export function RichDropdown({ label, icon, swatch, items, width, menuWidth, maxHeight, ariaLabel, testId, tip }: { label: string; icon?: ReactNode; swatch?: string; items: () => RichItem[]; width?: number | string; menuWidth?: number; maxHeight?: number; ariaLabel?: string; testId?: string; tip?: string }) {
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button ref={button} type="button" className="design-dropdown" style={{ width }} aria-haspopup="menu" aria-expanded={open} aria-label={ariaLabel} data-tip={tip} data-testid={testId} onClick={() => setOpen((now) => !now)}>
        {swatch ? <span className="design-proto-dropdown-swatch" style={{ background: swatch }} /> : null}
        {icon ? <span className="design-dropdown-lead">{icon}</span> : null}
        <span className="design-dropdown-label">{label}</span>
        <Icon name="Chevron" size={12} className="design-chevron" />
      </button>
      {open ? <RichMenu anchor={button.current} items={items()} width={menuWidth} maxHeight={maxHeight} onClose={() => setOpen(false)} label={ariaLabel} testId={testId ? `${testId}-menu` : undefined} /> : null}
    </>
  )
}

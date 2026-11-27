/**
 * The left panel's fields: the search row under the Pages and Layers headers (`search_field`) and
 * the inline rename field of the file name, page and layer rows (`rename_field`).
 */
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react'
import { Icon } from '../icons/Icon'

/**
 * The 32 px search row: a 24 px `field` box inset 12, a 12 px search glyph, a live filter. It takes
 * the keyboard when opened; Escape clears it and closes the row.
 */
export function SearchRow({ value, placeholder, onChange, onClose, testId }: { value: string; placeholder: string; onChange: (value: string) => void; onClose: () => void; testId?: string }) {
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => {
    input.current?.focus()
  }, [])
  return (
    <div className="design-left-search">
      <label className="design-left-search-field">
        <Icon name="Search" size={12} />
        <input
          ref={input}
          value={value}
          placeholder={placeholder}
          aria-label={placeholder}
          spellCheck={false}
          autoComplete="off"
          data-testid={testId}
          onChange={(event) => onChange(event.target.value)}
          onFocus={(event) => event.currentTarget.select()}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault()
              event.stopPropagation()
              onChange('')
              onClose()
            }
          }}
        />
      </label>
    </div>
  )
}

/**
 * A rename in place (`text_field_inline` while editing): `panel` fill and a 1 px accent border
 * 2 px around the text's box, the whole name selected. Enter or a blur commits a change that is not
 * blank; Escape cancels; Tab and Shift+Tab commit and ask for the next or previous row.
 */
export function RenameInput({
  value,
  onDone,
  onTab,
  style,
  bold,
  testId,
}: {
  value: string
  /** The name to keep (null: none); `going` is true when Tab goes on to rename another row. */
  onDone: (name: string | null, going?: boolean) => void
  onTab?: (back: boolean) => void
  style: CSSProperties
  bold?: boolean
  testId?: string
}) {
  const [text, setText] = useState(value)
  const input = useRef<HTMLInputElement>(null)
  const done = useRef(false)
  useLayoutEffect(() => {
    const el = input.current
    if (!el) return
    el.focus({ preventScroll: true })
    el.select()
  }, [])
  const finish = (commit: boolean, going = false) => {
    if (done.current) return
    done.current = true
    onDone(commit && text !== value && text.trim() !== '' ? text : null, going)
  }
  const keyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    event.stopPropagation()
    if (event.key === 'Enter') {
      event.preventDefault()
      finish(true)
    } else if (event.key === 'Escape') {
      event.preventDefault()
      finish(false)
    } else if (event.key === 'Tab' && onTab) {
      event.preventDefault()
      finish(true, true)
      onTab(event.shiftKey)
    }
  }
  return (
    <span className="design-left-rename" style={style}>
      <input
        ref={input}
        className={bold ? 'design-s11' : 'design-r11'}
        value={text}
        spellCheck={false}
        autoComplete="off"
        data-testid={testId}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={keyDown}
        onBlur={() => finish(true)}
        onPointerDown={(event) => event.stopPropagation()}
        onDoubleClick={(event) => event.stopPropagation()}
      />
    </span>
  )
}

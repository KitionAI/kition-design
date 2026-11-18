/** The text field (2.1 §7 `text_field`): select-all on focus, Enter or blur commits a change, Escape cancels. */
import { useEffect, useRef, useState, type CSSProperties } from 'react'

export type TextFieldProps = {
  value: string
  onCommit: (value: string) => void
  placeholder?: string
  width?: CSSProperties['width']
  disabled?: boolean
  /** Edits call `onCommit` on every change (`text_input`, for search boxes). */
  live?: boolean
  /** Shows bullets (`secret_field_named`). */
  secret?: boolean
  ariaLabel?: string
  autoFocus?: boolean
  testId?: string
}

export function TextField({ value, onCommit, placeholder, width, disabled, live = false, secret = false, ariaLabel, autoFocus, testId }: TextFieldProps) {
  const [text, setText] = useState(value)
  const [focused, setFocused] = useState(false)
  const cancelled = useRef(false)
  const pressFocus = useRef(false)
  useEffect(() => {
    if (!focused) setText(value)
  }, [focused, value])
  const commit = () => {
    if (!cancelled.current && text !== value) onCommit(text)
    cancelled.current = false
  }
  return (
    <div className="design-field design-field-text" style={{ width }} data-disabled={disabled || undefined} data-testid={testId}>
      <input
        type={secret ? 'password' : 'text'}
        value={live ? value : text}
        placeholder={placeholder}
        disabled={disabled}
        spellCheck={false}
        autoComplete="off"
        autoFocus={autoFocus}
        aria-label={ariaLabel ?? placeholder}
        onChange={(event) => (live ? onCommit(event.target.value) : setText(event.target.value))}
        // Select-all at once (a frame later, keys typed meanwhile would be selected and replaced);
        // the press that focused keeps the selection rather than placing the caret.
        onMouseDown={(event) => {
          pressFocus.current = document.activeElement !== event.currentTarget
        }}
        onMouseUp={(event) => {
          if (!pressFocus.current) return
          pressFocus.current = false
          event.preventDefault()
        }}
        onFocus={(event) => {
          setFocused(true)
          event.currentTarget.select()
        }}
        onBlur={() => {
          if (!live) commit()
          setFocused(false)
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter') event.currentTarget.blur()
          else if (event.key === 'Escape') {
            cancelled.current = true
            setText(value)
            event.currentTarget.blur()
          } else return
          event.stopPropagation()
        }}
      />
    </div>
  )
}

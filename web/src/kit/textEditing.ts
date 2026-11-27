/**
 * The Edit commands while a text is being edited on the canvas (walkthrough D6, D8): Select all
 * selects the text's characters, Copy and Cut take the selected characters as plain text (nothing
 * when none are selected), and Paste types the copied text at the caret. None of them touches the
 * layers: Cmd+A used to select every layer on the page and Cmd+V to paste a second text layer.
 * They run as the text editing commands (`text.edit.*`), from the keys and the menus alike.
 *
 * The text copied last is kept here too: the system clipboard (menus/clipboard.ts) gets it as well,
 * and a paste of what this editor put there types this one (the clipboard may hold the selection
 * the editor knew a moment before the engine's newest).
 */

/** The text Copy or Cut took last in this editor, for Paste in a text. */
let copied: string | null = null
/**
 * The Copy or Cut still reading the selection from the engine, with the text it took ('' for
 * none): a Paste pressed right after it (Cmd+C, End, Cmd+V typed quickly, or a busy engine) waits
 * for it instead of finding nothing.
 */
let copying: Promise<string> = Promise.resolve('')

/** The characters `text.edit.selection` answered, or '' (none selected, or no answer). */
export function selectedText(answer: string | null): string {
  try {
    const selection = answer ? (JSON.parse(answer) as { text?: unknown } | null) : null
    return typeof selection?.text === 'string' ? selection.text : ''
  } catch {
    return ''
  }
}

/** The text the Copy or Cut running now (or the last one) took: '' when it took none. */
export function textCopying(): Promise<string> {
  return copying
}

/**
 * Keeps `text` as the text a Paste in a text types (the characters of text layers copied as
 * layers), or forgets it (null: layers that are not text were copied).
 */
export function keepCopiedText(text: string | null) {
  copied = text
}

type Run = (command: string, params?: Record<string, unknown>) => Promise<string | null>

/** Whether `params` name the layers a command acts on (then it acts on those, not on the text). */
export const namesLayers = (params?: Record<string, unknown>) => params != null && ('ids' in params || 'id' in params || 'parent' in params)

/**
 * Runs Edit command `id` on the text being edited, through `run`: its answer, or `undefined` when
 * `id` is not one of these (it then runs as usual).
 */
export async function runInText(id: string, params: Record<string, unknown> | undefined, run: Run): Promise<string | null | undefined> {
  if (namesLayers(params)) return undefined
  switch (id) {
    case 'edit.selectAll':
      return run('text.edit.key', { key: 'SelectAll' })
    case 'edit.copy':
    case 'edit.cut': {
      const reading = run('text.edit.selection', {}).then((answer) => ({ answer, text: selectedText(answer) }))
      // Paste waits for this one; the copied text is kept before it goes on.
      copying = reading.then(
        ({ text }) => {
          if (text) copied = text
          return text
        },
        () => '',
      )
      const { answer, text } = await reading
      await copying
      if (!text) return answer
      return id === 'edit.cut' ? run('text.edit.key', { key: 'Backspace' }) : answer
    }
    case 'edit.paste':
      await copying
      return copied ? run('text.edit.insert', { text: copied }) : null
    default:
      return undefined
  }
}

/**
 * The Edit commands while a text is being edited on the canvas (walkthrough D6, D8): Select all
 * selects the text's characters, Copy and Cut take the selected characters as plain text (nothing
 * when none are selected), and Paste types the copied text at the caret. None of them touches the
 * layers: Cmd+A used to select every layer on the page and Cmd+V to paste a second text layer.
 * They run as the text editing commands (`text.edit.*`), from the keys and the menus alike; the
 * copied text is the editor's own (the system clipboard is not read or written here yet).
 */

/** The text Copy or Cut took last in this editor, for Paste in a text. */
let copied: string | null = null

type Run = (command: string, params?: Record<string, unknown>) => Promise<string | null>

/** Whether `params` name the layers a command acts on (then it acts on those, not on the text). */
const namesLayers = (params?: Record<string, unknown>) => params != null && ('ids' in params || 'id' in params || 'parent' in params)

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
      const answer = await run('text.edit.selection', {})
      let text = ''
      try {
        const selection = answer ? (JSON.parse(answer) as { text?: unknown } | null) : null
        text = typeof selection?.text === 'string' ? selection.text : ''
      } catch {
        text = ''
      }
      if (!text) return answer
      copied = text
      return id === 'edit.cut' ? run('text.edit.key', { key: 'Backspace' }) : answer
    }
    case 'edit.paste':
      return copied ? run('text.edit.insert', { text: copied }) : null
    default:
      return undefined
  }
}

/**
 * Preferences (2.7 §4.2): the editor's own rows (keep tool, highlight on hover, rename duplicates,
 * dimensions, number keys for opacity, nudge amount). Every change applies at once. The app's
 * rows (theme, language, interface size, motion, your name, auto save) are Kition's settings.
 */
import { useDesignKit, useQuery } from '../kit/hooks'
import { Checkbox, NumberField } from '../ui'
import { own } from '../menus/strings'
import { Caption, Dialog } from './Dialog'

type Prefs = { keepTool: boolean; highlightOnHover: boolean; renameDuplicates: boolean; showDimensions: boolean; numberKeysOpacity: boolean; nudgeSmall: number; nudgeBig: number }

const TOGGLES: Array<{ id: string; key: keyof Prefs; label: string }> = [
  { id: 'pref.keepTool', key: 'keepTool', label: 'Keep tool selected after use' },
  { id: 'pref.highlightHover', key: 'highlightOnHover', label: 'Highlight layers on hover' },
  { id: 'pref.renameDuplicates', key: 'renameDuplicates', label: 'Rename duplicated layers' },
  { id: 'pref.showDimensions', key: 'showDimensions', label: 'Show dimensions on objects' },
  { id: 'pref.numberKeysOpacity', key: 'numberKeysOpacity', label: 'Use number keys for opacity' },
]

export function PreferencesDialog({ onClose }: { onClose: () => void }) {
  const designKit = useDesignKit()
  const { t, run, refresh } = designKit
  const prefs = useQuery<Prefs>('prefs')
  const set = async (command: string, params: Record<string, unknown>) => {
    await run(command, params)
    // Preferences are no document change: ask the panels again.
    await refresh()
  }
  return (
    <Dialog title={t('Preferences')} width={360} onClose={onClose} testId="design-dialog-preferences" bodyStyle={{ gap: 0 }}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        {TOGGLES.map((toggle) => (
          <Checkbox key={toggle.id} checked={prefs?.[toggle.key] === true} label={t(toggle.label)} onChange={(on) => void set(toggle.id, { on })} testId={`design-pref-${toggle.key}`} />
        ))}
      </div>
      <div className="design-dialog-row" style={{ marginTop: 14 }}>
        <label>{t('Nudge amount')}</label>
        <span className="design-drag-value">
          <NumberField value={prefs?.nudgeSmall ?? 1} min={0.1} max={100} width={36} ariaLabel={t('Nudge amount')} onCommit={(edit) => void set('pref.nudge', { small: edit.kind === 'set' ? edit.value : edit.apply(prefs?.nudgeSmall ?? 1) })} testId="design-pref-nudge-small" />
        </span>
        <span className="design-drag-value">
          <NumberField value={prefs?.nudgeBig ?? 10} min={1} max={1000} width={36} ariaLabel={t('Nudge amount')} onCommit={(edit) => void set('pref.nudge', { big: edit.kind === 'set' ? edit.value : edit.apply(prefs?.nudgeBig ?? 10) })} testId="design-pref-nudge-big" />
        </span>
      </div>
      <Caption tone="secondary" style={{ marginTop: 14 }}>
        {own(designKit, 'preferences.host')}
      </Caption>
    </Dialog>
  )
}

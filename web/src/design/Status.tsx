/**
 * The section and handoff-status block (2.3 "Section / Dev status block", upstream
 * `panels/section_panel.rs`): section colours and Hide contents for sections; the Dev status, the
 * reason a ready layer changed (Resolve), the last change note, and Resize to fit.
 */
import { useState } from 'react'
import { useDesignKit } from '../kit/hooks'
import { Button, Checkbox, Separator, TextField } from '../ui'
import { useDesign, useShortcutTip } from './hooks'
import { COL2, Line, MenuField, Space, Sub } from './fields'
import { common } from './model'

const SECTION_COLORS = [
  ['White', '#FFFFFF'],
  ['Gray', '#EDEDED'],
  ['Red', '#FFE0DE'],
  ['Orange', '#FFE8D1'],
  ['Yellow', '#FFF4C2'],
  ['Green', '#DDF5E3'],
  ['Blue', '#DCEBFF'],
  ['Violet', '#EBE3FF'],
] as const

const STATUSES = [
  ['none', 'No status'],
  ['readyForDev', 'Ready for dev'],
  ['completed', 'Completed'],
] as const

export function StatusSection() {
  const { t, run } = useDesignKit()
  const { answer, nodes, ids, setProps } = useDesign()
  const tipFor = useShortcutTip()
  const [draft, setDraft] = useState('')
  if (!nodes.length || answer.hasStatus !== true) return null
  const sections = nodes.every((n) => n.type === 'SECTION')
  const fill = common(nodes, (n) => {
    const first = n.fills?.[0] as { type?: string; color?: string } | undefined
    return first?.type === 'solid' && typeof first.color === 'string' ? first.color.replace('#', '').slice(0, 6).toUpperCase() : null
  })
  const hidden = nodes.every((n) => n.sectionContentsHidden === true)
  const status = common(nodes, (n) => `${n.devStatus ?? 'none'}${n.devStatus === 'readyForDev' && n.devChanged ? ':changed' : ''}`)
  const label = status === null ? t('Mixed') : status.endsWith(':changed') ? t('Changed') : t(STATUSES.find(([id]) => id === status)?.[1] ?? 'No status')
  const changed = nodes.some((n) => n.devStatus === 'readyForDev' && n.devChanged)
  const note = common(nodes, (n) => n.devNote ?? '')
  const resolve = () => {
    const reason = draft.trim()
    void run('dev.resolveChange', reason ? { ids, reason } : { ids })
    setDraft('')
  }
  return (
    <div data-testid="design-design-status">
      {sections ? (
        <>
          <Sub a={t('Section color')} />
          <Line style={{ gap: 6 }}>
            {SECTION_COLORS.map(([name, hex]) => (
              <button
                key={hex}
                type="button"
                role="radio"
                className="design-d-dot"
                aria-checked={fill === hex.slice(1)}
                aria-label={t(name)}
                data-tip={t(name)}
                style={{ background: hex }}
                onClick={() => setProps({ fills: [hex] })}
                data-testid={`design-design-section-color-${name}`}
              />
            ))}
          </Line>
          <Line h={28}>
            <Checkbox checked={hidden} label={t('Hide contents')} onChange={() => void run('section.toggleContents', { ids, hidden: !hidden })} testId="design-design-hide-contents" />
          </Line>
        </>
      ) : null}
      <Sub a={t('Status')} />
      <Line>
        <MenuField label={label} icon="Code" mixed={status === null} width={COL2} items={STATUSES.map(([id, name]) => ({ label: t(name), onSelect: () => setProps({ devStatus: id }), testId: `design-design-status-${id}` }))} testId="design-design-status-menu" />
      </Line>
      {changed ? (
        <Line>
          <TextField value={draft} placeholder={t('Why it changed')} width="calc(var(--design-col) * 2 - 64px)" onCommit={setDraft} testId="design-design-status-reason" />
          <Button variant="primary" tip={t('Back to Ready for dev')} onClick={resolve} testId="design-design-status-resolve">
            {t('Resolve')}
          </Button>
        </Line>
      ) : note ? (
        <Line h={24}>
          <span className="design-d-label design-secondary" style={{ width: COL2 }}>
            {t('Last change: {note}', { note })}
          </span>
        </Line>
      ) : null}
      <Line>
        <button type="button" className="design-d-ghost" style={{ width: COL2 }} data-tip={tipFor(t('Fit to contents'), 'object.resizeToFit')} onClick={() => void run('object.resizeToFit', { ids })} data-testid="design-design-resize-to-fit">
          {t('Resize to fit')}
        </button>
      </Line>
      <Space h={4} />
      <Separator />
    </div>
  )
}

/**
 * Variable modes (2.3, upstream `panels/right/paints.rs` `variable_modes`): for frame-like layers,
 * a row per collection with more than one mode (`query('variable.modeRows')`): the collection's
 * name and a mode dropdown, Auto (the mode the layer inherits, named) or a mode of its own
 * (`variable.setMode`). Library collections come under their library's name; choosing a mode of
 * one this file does not use yet copies it in (`library.setVariableMode`).
 */
import { useDesignKit, useQuery } from '../kit/hooks'
import { SectionHeader, Separator } from '../ui'
import { useDesign } from './hooks'
import { Line, MenuField, Space, Sub } from './fields'

type Mode = { id: string; name: string }
/** `collection`: the id frames keep the mode under (null until a library's collection is used here); `key`: a library collection's id in its library. */
type ModeRow = { collection: string | null; key: string | null; name: string; library: { id: string; name: string } | null; modes: Mode[]; own: string | null; mixed: boolean; inherited: Mode | null }
type ModeRows = { show: boolean; rows: ModeRow[] }

export function VariableModesSection() {
  const { t, run } = useDesignKit()
  const { ids } = useDesign()
  const answer = useQuery<ModeRows>('variable.modeRows', { ids })
  if (!answer?.show) return null
  return (
    <div data-testid="design-design-variable-modes">
      <SectionHeader title={t('Variable modes')} />
      {answer.rows.map((row, i) => {
        const auto = t('Auto ({mode})', { mode: row.inherited?.name ?? '' })
        const label = row.mixed ? t('Mixed') : row.own ? (row.modes.find((m) => m.id === row.own)?.name ?? auto) : auto
        const set = (mode: string | null) => {
          if (row.library && row.key && mode !== null) void run('library.setVariableMode', { library: row.library.id, collection: row.key, ids, mode })
          else if (row.collection) void run('variable.setMode', { ids, collection: row.collection, mode })
        }
        const heading = row.library && row.library.id !== answer.rows[i - 1]?.library?.id ? <Sub a={row.library.name} /> : null
        return (
          <div key={`${row.library?.id ?? ''}/${row.key ?? row.collection ?? row.name}`}>
            {heading}
            <Line>
              <span className="design-d-mode-name" data-tip={row.library ? t('{name} (from {library})', { name: row.name, library: row.library.name }) : row.name}>
                {row.name}
              </span>
              <MenuField
                label={label}
                mixed={row.mixed}
                width="calc(var(--design-col) * 1.3)"
                menuWidth={180}
                ariaLabel={row.name}
                testId={`design-design-mode-${row.name}`}
                items={[
                  { label: auto, checked: !row.mixed && row.own === null, onSelect: () => set(null), testId: `design-design-mode-${row.name}-auto` },
                  { type: 'separator' },
                  ...row.modes.map((mode) => ({ label: mode.name, checked: !row.mixed && row.own === mode.id, onSelect: () => set(mode.id), testId: `design-design-mode-${row.name}-${mode.name}` })),
                ]}
              />
            </Line>
          </div>
        )
      })}
      <Space />
      <Separator />
    </div>
  )
}

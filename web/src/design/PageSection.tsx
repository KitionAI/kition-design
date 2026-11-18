/**
 * The Design tab with nothing selected (2.3 "Page section", upstream `panels/right.rs`
 * `page_section`): the page's background, the file's colour profile, Local variables, Local
 * styles, and the page's Export.
 */
import { useRef } from 'react'
import { dispatchUiRequests } from '../kit/requests'
import { useDesignKit, useInspect, useQuery } from '../kit/hooks'
import { SectionHeader, Separator, TextField } from '../ui'
import { ActionField, Line, MenuField } from './fields'
import { LocalStyles } from '../left/StylePicker'
import { PageExport } from './Export'
import { parseHex, type SelectionAnswer } from './model'

const PROFILES = [
  ['srgb', 'sRGB'],
  ['displayP3', 'Display P3'],
] as const

/** CSS colour names `page.setBackground` takes too (`Color::from_hex_or_name`), checked by the browser. */
function cssColor(text: string): string | null {
  const hex = parseHex(text)
  if (hex) return `#${text.trim().replace(/^#/, '')}`
  const probe = document.createElement('span')
  probe.style.color = ''
  probe.style.color = text.trim()
  return probe.style.color ? text.trim() : null
}

type Style = { id: string; name: string; type: string }

export function PageSection({ answer }: { answer: SelectionAnswer | null }) {
  const { t, run } = useDesignKit()
  const inspect = useInspect()
  const styles = useQuery<Style[]>('styles') ?? []
  const swatch = useRef<HTMLButtonElement>(null)
  const page = answer?.page
  const background = (page?.background ?? inspect.background ?? '#F5F5F5').replace(/^#/, '').slice(0, 6).toUpperCase()
  const profile = page?.colorProfile ?? inspect.colorProfile ?? 'srgb'
  const variables = page?.localVariables ?? 0
  const pageName = inspect.pages.find((p) => p.id === inspect.page)?.name ?? t('Page')
  return (
    <div data-testid="design-design-panel" data-selection="none">
      <SectionHeader title={t('Page')} />
      <Line testId="design-design-page">
        <button
          ref={swatch}
          type="button"
          className="design-d-swatch design-d-swatch-16"
          style={{ ['--swatch' as string]: `#${background}` }}
          aria-label={t('Background')}
          onClick={() => {
            const rect = swatch.current?.getBoundingClientRect()
            dispatchUiRequests([{ kind: 'picker.open', data: { target: 'pageBackground', color: `#${background}`, anchor: rect ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null } }])
          }}
          data-testid="design-design-page-swatch"
        />
        <TextField
          value={background}
          width={96}
          ariaLabel={t('Background')}
          onCommit={(text) => {
            const color = cssColor(text)
            if (color) void run('page.setBackground', { color })
          }}
          testId="design-design-page-background"
        />
        <span className="design-d-label">100 %</span>
      </Line>
      <Separator />
      <SectionHeader title={t('Color profile')} />
      <Line>
        <MenuField
          label={PROFILES.find(([id]) => id === profile)?.[1] ?? profile}
          width={140}
          items={PROFILES.map(([id, label]) => ({ label, onSelect: () => (id === profile ? undefined : dispatchUiRequests([{ kind: 'dialog.colorProfile', data: { profile: id } }])), testId: `design-design-profile-${id}` }))}
          testId="design-design-profile"
        />
      </Line>
      <Separator />
      <SectionHeader title={t('Local variables')} dim={variables === 0} />
      <Line>
        <ActionField label={t('Open variables ({count})', { count: variables })} icon="Styles" width={200} onClick={() => dispatchUiRequests([{ kind: 'variables', data: {} }])} testId="design-design-open-variables" />
      </Line>
      <Separator />
      <SectionHeader title={t('Local styles')} dim={styles.length === 0} />
      <LocalStyles />
      <Separator />
      <PageExport page={inspect.page} name={pageName} />
    </div>
  )
}

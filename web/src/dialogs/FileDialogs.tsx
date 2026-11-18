/**
 * The File and Text dialogs (2.7 §4.1, §4.6-§4.9): Color profile, Missing fonts, Import report,
 * Export, and the kit's About (upstream's About is Kition's; this one names the kit, its version
 * and licence).
 */
import { useState } from 'react'
import manifest from '../../../kit.json'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Checkbox, NumberField, Radio } from '../ui'
import { fileName, fromBase64, saveExport } from '../menus/invoke'
import { own } from '../menus/strings'
import { Button, Caption, Dialog } from './Dialog'

const PROFILES = [
  { id: 'srgb', label: 'sRGB' },
  { id: 'displayP3', label: 'Display P3' },
] as const

/** Color profile (§4.6): the file's profile; another one converts (Keep appearance) or reinterprets (Keep values). */
export function ColorProfileDialog({ onClose }: { onClose: () => void }) {
  const { t, run, inspect, refresh } = useDesignKit()
  const current = inspect.colorProfile === 'displayP3' || inspect.colorProfile === 'display-p3' ? 'displayP3' : 'srgb'
  const [chosen, setChosen] = useState<string>(current)
  const label = (id: string) => PROFILES.find((profile) => profile.id === id)?.label ?? id
  const apply = async (convert: boolean) => {
    if (await run('file.colorProfile', { profile: chosen, convert })) {
      await refresh()
      onClose()
    }
  }
  return (
    <Dialog title={t('Color profile')} width={340} onClose={onClose} testId="design-dialog-color-profile">
      <Caption tone="secondary">{t('Color values in this file are in this color space.')}</Caption>
      {/* 8 px around the radios (`add_space(8)`), 24 px rows 6 apart. */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6, margin: '8px 0' }}>
        {PROFILES.map((profile) => (
          <Radio key={profile.id} checked={chosen === profile.id} label={profile.id === current ? t('{profile} (current)', { profile: profile.label }) : profile.label} onChange={() => setChosen(profile.id)} testId={`design-profile-${profile.id}`} />
        ))}
      </div>
      {chosen === current ? (
        <Caption tone="secondary">{t('Choose another profile to change it.')}</Caption>
      ) : (
        <>
          <Caption>{t('Keep appearance converts every color value to {profile} so colors look the same. Keep values leaves the numbers as they are, so colors shift.', { profile: label(chosen) })}</Caption>
          {chosen !== 'srgb' ? <Caption tone="secondary" style={{ marginTop: 4 }}>{t('The canvas previews colors in sRGB; exported PNG, JPG and WebP files are tagged Display P3.')}</Caption> : null}
          <div className="design-dialog-buttons" style={{ marginTop: 12 }}>
            <Button variant="primary" onClick={() => void apply(true)} testId="design-profile-convert">
              {t('Keep appearance')}
            </Button>
            <Button onClick={() => void apply(false)} testId="design-profile-keep">
              {t('Keep values')}
            </Button>
          </div>
        </>
      )}
    </Dialog>
  )
}

/** Missing fonts (§4.7): the families the file names that the editor lacks, with their text layers. */
export function MissingFontsDialog({ onClose }: { onClose: () => void }) {
  const { t } = useDesignKit()
  const missing = useQuery<{ families: Array<{ family: string; layers: number }> }>('missingFonts')
  return (
    <Dialog title={t('Missing fonts')} width={380} onClose={onClose} testId="design-dialog-missing-fonts">
      {/* egui's default labels: regular 12 (`text`; the layer counts `text_secondary`). */}
      {!missing ? (
        <div className="design-dialog-label">{t('Checking the fonts installed on this computer…')}</div>
      ) : !missing.families.length ? (
        <div className="design-dialog-label">{t('All the fonts this file uses are installed')}</div>
      ) : (
        <>
          <Caption tone="secondary">{t("This file uses fonts that aren't installed on this computer. Their text is drawn in a fallback font until you install them.")}</Caption>
          <div className="design-dialog-list" style={{ marginTop: 6 }}>
            {missing.families.map((font) => (
              <div key={font.family} className="design-dialog-row design-dialog-label" style={{ minHeight: 0 }}>
                <span>{font.family}</span>
                <span data-tone="secondary">{font.layers === 0 ? t('only in a text style or variable') : t('{n} text layers', { n: font.layers })}</span>
              </div>
            ))}
          </div>
        </>
      )}
      <div className="design-dialog-buttons" style={{ marginTop: 12 }}>
        <Button variant="primary" onClick={onClose} testId="design-missing-close">
          {t('Close')}
        </Button>
      </div>
    </Dialog>
  )
}

type Summary = { name?: string; pages?: number; layers?: number; images?: number }

const MAX_NOTES = 500
const MAX_NOTE_CHARS = 2000

/** Import report (§4.8): what an import added and its compatibility notes. */
export function ImportReportDialog({ data, onClose }: { data: Record<string, unknown>; onClose: () => void }) {
  const designKit = useDesignKit()
  const { t } = designKit
  const summary = (data.summary ?? {}) as Summary
  const notes = (Array.isArray(data.warnings) ? data.warnings : []).map((note) => (typeof note === 'string' ? note : JSON.stringify(note)))
  const shown = notes.slice(0, MAX_NOTES).map((note) => {
    const text = t(note).slice(0, MAX_NOTE_CHARS)
    return text.charAt(0).toUpperCase() + text.slice(1)
  })
  const counts = [`${t('Pages')}: ${summary.pages ?? 0}`, `${t('Layers')}: ${summary.layers ?? 0}`, `${t('Images')}: ${summary.images ?? 0}`].join(' · ')
  return (
    <Dialog
      title={t('Import report')}
      width={420}
      onClose={onClose}
      testId="design-dialog-import-report"
      footer={
        <Button variant="primary" onClick={onClose}>
          {t('Close')}
        </Button>
      }
    >
      <span className="design-dialog-strong">{String(summary.name ?? data.name ?? t('Imported document'))}</span>
      <Caption>{t(data.how === 'placed' ? 'Placed on the current page' : 'Pages added to the document')}</Caption>
      <Caption>{counts}</Caption>
      <div style={{ height: 8 }} />
      {!shown.length ? (
        <Caption>{t('No compatibility warnings reported')}</Caption>
      ) : (
        <>
          <span className="design-dialog-strong">{t('Compatibility notes')}</span>
          <Caption tone="secondary">{own(designKit, 'import.notes')}</Caption>
          <div className="design-dialog-list" style={{ marginTop: 4 }}>
            {shown.map((note, index) => (
              <Caption key={index}>{note}</Caption>
            ))}
          </div>
          {notes.length > MAX_NOTES ? <Caption>{t('{n} more notes omitted from this report', { n: notes.length - MAX_NOTES })}</Caption> : null}
        </>
      )}
    </Dialog>
  )
}

type Job = { id: number; file: string }
type Preset = { label: string; format: string; scale: number; extension: string }

const PRESETS: Preset[] = [
  { label: 'PNG @1x', format: 'png', scale: 1, extension: 'png' },
  { label: 'PNG @2x', format: 'png', scale: 2, extension: 'png' },
  { label: 'PNG @3x', format: 'png', scale: 3, extension: 'png' },
  { label: 'JPG @2x', format: 'jpg', scale: 2, extension: 'jpg' },
  { label: 'WebP @2x', format: 'webp', scale: 2, extension: 'webp' },
  { label: 'SVG', format: 'svg', scale: 1, extension: 'svg' },
  { label: 'PDF', format: 'pdf', scale: 1, extension: 'pdf' },
  { label: 'XML', format: 'xml', scale: 1, extension: 'xml' },
]

const SHOWN_JOBS = 6

/** Export (§4.9): the layers' export settings, the selection (or the page) as one file, and the whole file as .fig. */
export function ExportDialog({ onClose }: { onClose: () => void }) {
  const designKit = useDesignKit()
  const { t, run, inspect, toast } = designKit
  const page = useQuery<{ nodes?: unknown[]; page?: { id: number } }>('selection')
  const settings = useQuery<{ jobs: Job[] }>('exportSettings', inspect.selection.length ? { ids: inspect.selection } : {})
  const document = useQuery<{ pages?: Array<{ id: number; children?: Array<{ id: number }> }> }>('document', { depth: 1 }, { skip: inspect.selection.length > 0 })
  const [bleed, setBleed] = useState(0)
  const [cropMarks, setCropMarks] = useState(false)
  const [bleedMarks, setBleedMarks] = useState(false)
  const [cmyk, setCmyk] = useState(false)
  const jobs = settings?.jobs ?? []
  const ids = inspect.selection.length ? inspect.selection : (document?.pages?.find((p) => p.id === (page?.page?.id ?? inspect.page))?.children ?? document?.pages?.[0]?.children ?? []).map((child) => child.id)
  const print = { bleed: `${bleed}mm`, cropMarks, bleedMarks, cmyk }

  const exported = (name: string, warnings: unknown) => {
    const n = Array.isArray(warnings) ? warnings.length : 0
    toast(n ? t('Exported {name}; {n} kinds of content were left out', { name, n }) : t('Exported {name}', { name }))
  }

  const saveOne = async (preset: Preset) => {
    if (!ids.length) {
      toast(t('Nothing to export'))
      return
    }
    onClose()
    const answer = await run<{ data?: string; warnings?: unknown[] }>('file.export', { ids, format: preset.format, scale: preset.scale, ...(preset.format === 'pdf' ? print : {}) })
    if (typeof answer?.data !== 'string') return
    const base = ids.length === 1 ? ((await designKit.query<{ name?: string }>('node', { id: ids[0] }).catch(() => null))?.name ?? 'Export') : 'Export'
    const suffix = preset.scale !== 1 && preset.format !== 'svg' && preset.format !== 'pdf' && preset.format !== 'xml' ? `@${preset.scale}x` : ''
    const written = await saveExport(designKit, fileName(`${base}${suffix}`, preset.extension), fromBase64(answer.data))
    if (written) exported(written, answer.warnings)
  }

  const exportLayers = async () => {
    onClose()
    const answer = await run<{ files?: Array<{ name: string; data?: string }> }>('file.exportLayers', inspect.selection.length ? { ids: inspect.selection } : {})
    for (const file of answer?.files ?? []) {
      if (typeof file.data !== 'string') continue
      const written = await saveExport(designKit, file.name.split('/').pop() ?? file.name, fromBase64(file.data))
      if (written) toast(t('Exported {name}', { name: written }))
    }
  }

  const saveFig = async () => {
    onClose()
    const answer = await run<{ data?: string; warnings?: unknown[] }>('file.export', { format: 'fig' })
    if (typeof answer?.data !== 'string') return
    const written = await saveExport(designKit, fileName(inspect.name.replace(/\.[^.]+$/, '') || 'Untitled', 'fig'), fromBase64(answer.data))
    if (written) exported(written, answer.warnings)
  }

  const layers = new Set(jobs.map((job) => job.id)).size
  return (
    <Dialog title={t('Export')} width={320} onClose={onClose} testId="design-dialog-export">
      {jobs.length ? (
        <>
          <Caption tone="secondary">{t("With the layers' export settings")}</Caption>
          {jobs.slice(0, SHOWN_JOBS).map((job, index) => (
            <div key={index} style={{ font: 'var(--design-regular) 12px/1.4 var(--design-font)' }}>
              {job.file}
            </div>
          ))}
          {jobs.length > SHOWN_JOBS ? <Caption tone="secondary">{t('and {n} more', { n: jobs.length - SHOWN_JOBS })}</Caption> : null}
          <div style={{ marginTop: 6 }}>
            <Button variant="primary" onClick={() => void exportLayers()} testId="design-export-layers">
              {t('Export {n} layers', { n: layers })}
            </Button>
          </div>
          <div className="design-dialog-divider" />
        </>
      ) : null}
      <Caption tone="secondary">{t('{n} layers as one file', { n: ids.length })}</Caption>
      {PRESETS.map((preset, index) => (
        <button key={preset.label} type="button" className="design-export-row" style={index === 0 ? { marginTop: 4 } : undefined} onClick={() => void saveOne(preset)} data-testid={`design-export-${preset.format}-${preset.scale}`}>
          {preset.label}
        </button>
      ))}
      <Caption tone="secondary" style={{ marginTop: 6 }}>
        {t('PDF print options')}
      </Caption>
      <div className="design-dialog-row">
        <Caption tone="secondary">{t('Bleed')}</Caption>
        <span style={{ flex: 1, minWidth: 0, marginRight: 4 }}>
          <NumberField value={bleed} min={0} max={100} suffix=" mm" width="100%" ariaLabel={t('Bleed')} onCommit={(edit) => setBleed(Math.max(0, edit.kind === 'set' ? edit.value : edit.apply(bleed)))} />
        </span>
      </div>
      <Checkbox checked={cropMarks} label={t('Crop marks')} onChange={setCropMarks} />
      <Checkbox checked={bleedMarks} label={t('Bleed marks')} onChange={setBleedMarks} />
      {bleedMarks && bleed === 0 ? <Caption tone="secondary">{t('Bleed marks need a bleed')}</Caption> : null}
      <Checkbox checked={cmyk} label={t('CMYK colour')} onChange={setCmyk} />
      <div className="design-dialog-divider" />
      <Caption tone="secondary">{t('The whole file')}</Caption>
      <button type="button" className="design-export-row" style={{ marginTop: 4 }} onClick={() => void saveFig()} data-testid="design-export-fig">
        {t('.fig (experimental)')}
      </button>
      <Caption tone="secondary">{t('Other apps may not open every .fig file exported here.')}</Caption>
    </Dialog>
  )
}

/** About (Help menu of the `?` button): the kit's name, version and licence (Kition's own About covers the app). */
export function AboutDialog({ onClose }: { onClose: () => void }) {
  const designKit = useDesignKit()
  const { t } = designKit
  return (
    <Dialog title={t('About Kition Design')} width={660} onClose={onClose} testId="design-dialog-about">
      <span className="design-dialog-strong" style={{ fontSize: 18 }}>
        Kition Design
      </span>
      <Caption tone="secondary">{t('Version {version}', { version: manifest.version })}</Caption>
      <div style={{ height: 12 }} />
      <Caption>{own(designKit, 'about.blurb')}</Caption>
      <Caption tone="secondary">{own(designKit, 'about.license')}</Caption>
    </Dialog>
  )
}

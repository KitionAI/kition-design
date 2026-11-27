/**
 * Export (2.3 "Export section", upstream `panels/export_panel.rs`): a row per export setting (size,
 * suffix, format, advanced options, remove; a PDF's print settings), the Preview disclosure, and
 * the Export button, which renders each job with the binding's `exportNodeWith` (the setting's
 * size, contents only, SVG and PDF options) and hands the file to the host (`exportFile`). The
 * page's Export section exports the page as one PNG.
 */
import { useEffect, useRef, useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Checkbox, IconButton, Menu, Note, Popover, Radio, SectionHeader, TextField, type MenuEntry } from '../ui'
import { useDesign } from './hooks'
import { Line, MenuField, WideButton } from './fields'
import { common, fmt, type ExportSetting, type ExportSize } from './model'

const FORMATS: Array<[string, string]> = [
  ['png', 'PNG'],
  ['jpg', 'JPG'],
  ['svg', 'SVG'],
  ['pdf', 'PDF'],
  ['webp', 'WebP'],
  ['vectorDrawable', 'XML'],
]
const EXT: Record<string, string> = { png: 'png', jpg: 'jpg', svg: 'svg', pdf: 'pdf', webp: 'webp', vectorDrawable: 'xml' }
const RASTER = new Set(['png', 'jpg', 'webp'])
const SIZE_PRESETS = ['0.5x', '0.75x', '1x', '1.5x', '2x', '3x', '4x', '512w', '512h']
const DEFAULT_SETTING: ExportSetting = { size: { by: 'scale', value: 1 }, suffix: '', format: 'png' }

const sizeLabel = (s: ExportSize) => `${fmt(s.value)}${s.by === 'scale' ? 'x' : s.by === 'width' ? 'w' : 'h'}`

/** Parses "2x", "@2x", "2", "512w", "120h" (as `ExportSize::parse`). */
function parseSize(text: string): ExportSize | null {
  const m = /^@?\s*(\d+(?:\.\d+)?|\.\d+)\s*([xwh])?$/i.exec(text.trim())
  if (!m) return null
  const value = Number(m[1])
  if (!(value > 0)) return null
  const unit = (m[2] ?? 'x').toLowerCase()
  return { by: unit === 'w' ? 'width' : unit === 'h' ? 'height' : 'scale', value }
}

type Job = { id: number; setting: ExportSetting; file: string }

/** Renders `jobs` with `exportNodeWith` (each with its whole setting) and writes each through the host; answers the files written. */
async function exportJobs(designKit: ReturnType<typeof useDesignKit>, jobs: Job[]) {
  const written: string[] = []
  for (const job of jobs) {
    const answer = await designKit.invoke<Uint8Array | ArrayBuffer>('exportNodeWith', String(job.id), JSON.stringify({ setting: job.setting }))
    const bytes = answer instanceof Uint8Array ? answer : new Uint8Array(answer)
    written.push(await designKit.editor.host.exportFile(job.file, bytes))
  }
  return written
}

export function ExportSection() {
  const designKit = useDesignKit()
  const { t, run } = designKit
  const { nodes, ids, setProps } = useDesign()
  const shared = common(nodes, (n) => n.exports ?? [])
  const mixed = shared === null && nodes.length > 1
  const list = shared ?? []
  const jobs = useQuery<{ jobs: Job[] }>('exportSettings', { ids })?.jobs ?? []
  const text = nodes.every((n) => n.type === 'TEXT')
  const layers = [...new Set(jobs.map((j) => j.id))]
  const name = layers.length === 1 ? (nodes.find((n) => n.id === layers[0])?.name ?? '') : ''
  const run_ = async () => {
    try {
      // The settings as the engine has them now: a format picked just before the click may not
      // have reached this panel's answer yet.
      const now = (await designKit.query<{ jobs: Job[] }>('exportSettings', { ids }).catch(() => null))?.jobs ?? jobs
      const written = await exportJobs(designKit, now)
      designKit.toast(written.length === 1 ? t('Exported {name}', { name: written[0].split('/').pop() ?? '' }) : t('Exported {n} files', { n: written.length }))
    } catch (error) {
      designKit.toast(error instanceof Error ? error.message : String(error))
    }
  }
  return (
    <div data-testid="design-design-export">
      <SectionHeader
        title={t('Export')}
        dim={list.length === 0}
        actions={[{ icon: 'Plus', label: mixed ? t('Replace mixed export settings') : t('Add export setting'), onClick: () => (mixed ? setProps({ exports: [DEFAULT_SETTING] }) : void run('export.add', { ids })), testId: 'design-design-export-add' }]}
      />
      {mixed ? <Note>{t('Click + to replace mixed content')}</Note> : null}
      {list.map((setting, i) => (
        <SettingRow key={i} index={i} setting={setting} text={text} />
      ))}
      {jobs.length ? (
        <>
          <Preview id={jobs[0].id} />
          <WideButton label={layers.length === 1 ? t('Export {name}', { name }) : t('Export {n} layers', { n: layers.length })} onClick={() => void run_()} testId="design-design-export-run" />
        </>
      ) : null}
    </div>
  )
}

function SettingRow({ index, setting, text }: { index: number; setting: ExportSetting; text: boolean }) {
  const { t, run } = useDesignKit()
  const { ids } = useDesign()
  const more = useRef<HTMLSpanElement>(null)
  const [open, setOpen] = useState(false)
  const update = (change: Record<string, unknown>) => void run('export.set', { ids, index, ...change })
  const raster = RASTER.has(setting.format)
  const option = (key: keyof ExportSetting, on: boolean, label: string) => <Checkbox checked={on} label={label} onChange={() => update({ [key]: !on })} testId={`design-design-export-${index}-${key}`} />
  return (
    <>
      <Line style={{ gap: 4 }} testId={`design-design-export-${index}`}>
        {raster ? <SizeCombo value={sizeLabel(setting.size)} onCommit={(s) => update({ size: s })} testId={`design-design-export-${index}-size`} /> : null}
        {/* Measured upstream: the suffix field ends 12 px before the format dropdown (4 elsewhere). */}
        <span className="design-d-grow" style={{ minWidth: 32, marginRight: 8 }}>
          <TextField value={setting.suffix} placeholder={t('Suffix')} width="100%" onCommit={(suffix) => update({ suffix })} testId={`design-design-export-${index}-suffix`} />
        </span>
        <MenuField
          label={FORMATS.find(([id]) => id === setting.format)?.[1] ?? setting.format}
          tip={setting.format === 'vectorDrawable' ? t('Android Vector Drawable') : undefined}
          width={54}
          items={FORMATS.map(([id, label]) => ({ label, onSelect: () => update({ format: EXT[id] }), testId: `design-design-export-${index}-format-${id}` }))}
          testId={`design-design-export-${index}-format`}
        />
        <span ref={more} className="design-d-anchor">
          <IconButton iconSize={16} icon="Dots" size={22} label={t('Export settings')} onClick={() => setOpen(!open)} testId={`design-design-export-${index}-more`} />
        </span>
        <IconButton iconSize={16} icon="Minus" size={22} label={t('Remove export setting')} onClick={() => void run('export.remove', { ids, index })} testId={`design-design-export-${index}-remove`} />
      </Line>
      {open ? (
        <Popover open anchor={more.current} placement="below-end" gap={0} onClose={() => setOpen(false)} width={216} testId={`design-design-export-${index}-options`}>
          <div className="design-d-options">
            {option('contentsOnly', setting.contentsOnly !== false, t('Ignore overlapping layers'))}
            {text || setting.includeBoundingBox ? option('includeBoundingBox', setting.includeBoundingBox === true, t('Include bounding box')) : null}
            {setting.format === 'svg' ? (
              <>
                {option('svgOutlineText', setting.svgOutlineText !== false, t('Outline text'))}
                {option('svgIdAttribute', setting.svgIdAttribute === true, t('Include id attribute'))}
                {option('svgSimplifyStroke', setting.svgSimplifyStroke !== false, t('Simplify stroke'))}
              </>
            ) : null}
            {setting.format === 'vectorDrawable' ? (
              <>
                {option('svgIdAttribute', setting.svgIdAttribute === true, t('Include id attribute'))}
                <div className="design-secondary design-d-options-label">{t('Compatibility')}</div>
                {(
                  [
                    ['bestEffort', 'Best effort'],
                    ['exact', 'Exact only'],
                    ['ignore', 'Ignore unsupported'],
                  ] as const
                ).map(([mode, label]) => (
                  <Radio key={mode} checked={(setting.vdCompat ?? 'bestEffort') === mode} label={t(label)} onChange={() => update({ vdCompat: mode })} />
                ))}
              </>
            ) : null}
          </div>
        </Popover>
      ) : null}
      {setting.format === 'pdf' ? <PrintRow index={index} setting={setting} /> : null}
    </>
  )
}

/** The size field: typed ("2x", "300w") or picked from the presets (`combo_field`). */
function SizeCombo({ value, onCommit, testId }: { value: string; onCommit: (size: string) => void; testId?: string }) {
  const { t } = useDesignKit()
  const chevron = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const commit = (text: string) => {
    const size = parseSize(text)
    if (size && sizeLabel(size) !== value) onCommit(text.trim())
  }
  const items: MenuEntry[] = SIZE_PRESETS.map((preset) => ({ label: preset, onSelect: () => commit(preset), testId: testId ? `${testId}-${preset}` : undefined }))
  return (
    <span className="design-d-combo" style={{ width: 52 }}>
      <TextField value={value} width="100%" ariaLabel={t('Size')} onCommit={commit} testId={testId} />
      <button ref={chevron} type="button" className="design-d-combo-chevron" aria-label={t('Presets')} onClick={() => setOpen(!open)} data-testid={testId ? `${testId}-presets` : undefined}>
        <Icon name="Chevron" size={12} />
      </button>
      {open ? <Menu tone="light" items={items} anchor={chevron.current} placement="below-start" gap={0} minWidth={52} onClose={() => setOpen(false)} /> : null}
    </span>
  )
}

/** "3 mm bleed, crop marks, CMYK" (or "RGB, no bleed"). */
function printSummary(p: NonNullable<ExportSetting['print']>, t: (key: string, values?: Record<string, string | number>) => string) {
  const parts: string[] = []
  const bleed = p.bleed ?? 0
  if (bleed > 0) parts.push(t('{length} bleed', { length: `${fmt(bleed * 0.352778)} mm` }))
  if (p.cropMarks) parts.push(t('crop marks'))
  if (p.bleedMarks && bleed > 0) parts.push(t('bleed marks'))
  parts.push(p.cmyk ? 'CMYK' : 'RGB')
  if (bleed <= 0 && !p.cropMarks && !p.bleedMarks) parts.push(t('no bleed'))
  const s = parts.join(', ')
  return s.charAt(0).toUpperCase() + s.slice(1)
}

/** Parses "3 mm", "0.125 in", "9 pt" (points per pixel: one) into points; null otherwise. */
function parseLength(text: string): number | null {
  const m = /^(\d+(?:\.\d+)?|\.\d+)\s*(mm|in|pt|px)?$/i.exec(text.trim())
  if (!m) return null
  const v = Number(m[1])
  const unit = (m[2] ?? 'mm').toLowerCase()
  return unit === 'in' ? v * 72 : unit === 'mm' ? v / 0.352778 : v
}

function PrintRow({ index, setting }: { index: number; setting: ExportSetting }) {
  const { t, run } = useDesignKit()
  const { ids } = useDesign()
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const p = { bleed: 0, cropMarks: false, bleedMarks: false, cmyk: false, ...(setting.print ?? {}) }
  const update = (change: Partial<typeof p>) => void run('export.set', { ids, index, ...p, ...change })
  return (
    <Line h={26}>
      <button ref={button} type="button" className="design-d-print" data-tip={t('PDF print settings')} onClick={() => setOpen(!open)} data-testid={`design-design-export-${index}-print`}>
        <span>{printSummary(p, t)}</span>
        <Icon name="Dots" size={14} />
      </button>
      {open ? (
        <Popover open anchor={button.current} placement="below-end" onClose={() => setOpen(false)} width={216}>
          <div className="design-d-options">
            <div className="design-d-options-row">
              <span className="design-secondary">{t('Bleed')}</span>
              <TextField
                value={p.bleed > 0 ? `${fmt(p.bleed * 0.352778)} mm` : ''}
                placeholder="0 mm"
                width="100%"
                onCommit={(text) => {
                  const b = parseLength(text)
                  if (b !== null && b <= 144) update({ bleed: b })
                }}
              />
            </div>
            <Checkbox checked={p.cropMarks} label={t('Crop marks')} onChange={() => update({ cropMarks: !p.cropMarks })} />
            <Checkbox checked={p.bleedMarks} label={t('Bleed marks')} onChange={() => update({ bleedMarks: !p.bleedMarks })} />
            <Checkbox checked={p.cmyk} label={t('CMYK colour')} onChange={() => update({ cmyk: !p.cmyk })} />
            {p.bleedMarks && p.bleed <= 0 ? <div className="design-secondary">{t('Bleed marks need a bleed')}</div> : null}
          </div>
        </Popover>
      ) : null}
    </Line>
  )
}

/** The Preview disclosure's state is one for the panel (`app.ui.export_thumb` upstream). */
let previewOpen = false

function Preview({ id }: { id: number }) {
  const designKit = useDesignKit()
  const [open, setOpen] = useState(previewOpen)
  const [url, setUrl] = useState<string | null>(null)
  const [size, setSize] = useState<{ w: number; h: number } | null>(null)
  const box = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return undefined
    let alive = true
    let made: string | null = null
    // Re-rendered at most every 0.3 s while the document changes.
    const timer = window.setTimeout(() => {
      const px = Math.round(((box.current?.clientWidth ?? 208) - 16) * (window.devicePixelRatio || 1))
      designKit
        .invoke<Uint8Array | ArrayBuffer>('nodeThumbnail', String(id), Math.max(16, px))
        .then((answer) => {
          if (!alive) return
          const bytes = answer instanceof Uint8Array ? answer : new Uint8Array(answer)
          made = URL.createObjectURL(new Blob([bytes as BlobPart], { type: 'image/png' }))
          const img = new Image()
          img.onload = () => alive && setSize({ w: img.naturalWidth, h: img.naturalHeight })
          img.src = made
          setUrl(made)
        })
        .catch(() => alive && setUrl(null))
    }, 300)
    return () => {
      alive = false
      window.clearTimeout(timer)
      if (made) URL.revokeObjectURL(made)
    }
  }, [designKit, designKit.revision, id, open])
  const dpr = window.devicePixelRatio || 1
  const fit = (w: number, h: number) => {
    const bw = (box.current?.clientWidth ?? 208) - 16
    const k = Math.min(bw / w, (160 - 16) / h, 1 / dpr)
    return { width: w * k, height: h * k }
  }
  return (
    <>
      <Line h={28}>
        <button
          type="button"
          className="design-d-disclosure"
          aria-expanded={open}
          onClick={() => {
            previewOpen = !open
            setOpen(!open)
          }}
          data-testid="design-design-export-preview"
        >
          <Icon name={open ? 'Chevron' : 'ChevronRight'} size={12} />
          <span>{designKit.t('Preview')}</span>
        </button>
      </Line>
      {open ? (
        <div className="design-d-preview-wrap">
          <div ref={box} className="design-d-preview" data-testid="design-design-export-preview-box">
            {url && size ? <img src={url} alt="" style={fit(size.w, size.h)} /> : null}
          </div>
        </div>
      ) : null}
    </>
  )
}

/** The page's Export section (nothing selected): the page's layers as one PNG. */
export function PageExport({ page, name }: { page: number; name: string }) {
  const designKit = useDesignKit()
  return (
    <>
      <SectionHeader title={designKit.t('Export')} />
      <WideButton
        label={designKit.t('Export page as PNG')}
        testId="design-design-export-page"
        onClick={() =>
          void exportJobs(designKit, [{ id: page, setting: DEFAULT_SETTING, file: `${name}.png` }]).then(
            (written) => designKit.toast(designKit.t('Exported {name}', { name: written[0]?.split('/').pop() ?? '' })),
            (error: unknown) => designKit.toast(error instanceof Error ? error.message : String(error)),
          )
        }
      />
    </>
  )
}

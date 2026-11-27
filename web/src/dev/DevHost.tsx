/**
 * Always mounted over the canvas: Dev Mode's focus bar (`dev_mode::focus_bar`), the version
 * preview banner (`versions::preview_banner`, which has the top of the canvas first), and the
 * Compare changes window (`dev_compare.rs`); and the UI requests that open them.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Icon } from '../icons/Icon'
import { useDesignKit, useQuery } from '../kit/hooks'
import { useUiRequest } from '../kit/requests'
import type { Rect } from '../kit/types'
import { Button, IconButton } from '../ui'
import { focusOn, statusColor } from './DevPanel'
import { loadCheckpoint, refusal, restoreVersion, useCheckpoints } from './checkpoints'
import { STATUS_COLORS, openVersionHistory, relativeTime, setDev, useDev, type FocusState } from './store'
import { MenuDropdown, toBytes } from './widgets'
import './dev.css'


export function DevHost() {
  const { run, invoke, refresh, ui, doc } = useDesignKit()
  const dev = useDev()
  useUiRequest('panel.versionHistory', (data) => openVersionHistory((data as { save?: boolean } | null)?.save === true))
  useUiRequest('view.devCompare', (data) => {
    const d = data as { id?: number; snapshot?: string | null } | null
    if (typeof d?.id === 'number') setDev({ compare: { id: d.id, snapshot: d.snapshot ?? null } })
  })
  // The engine's Dev Mode (measure on hover, no handles, edits refused) follows the Inspect tab.
  // Told when the tab or the document changes (not again with every render).
  const inspecting = ui.rightTab === 'inspect'
  const latest = useRef({ invoke, refresh })
  latest.current = { invoke, refresh }
  useEffect(() => {
    void latest.current.invoke<boolean>('setDevMode', inspecting).then(
      (changed) => (changed ? latest.current.refresh() : undefined),
      () => undefined,
    )
  }, [doc, inspecting])
  const focus = useQuery<FocusState | null>('dev.focusState')
  // Escape leaves the focus view (when nothing else takes it).
  useEffect(() => {
    if (!focus) return undefined
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return
      const target = event.target
      if (target instanceof HTMLElement && (target.closest('input, textarea, [contenteditable="true"]') || document.querySelector('.design-layer'))) return
      void run('dev.unfocus', {})
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [focus, run])
  const preview = useQuery<{ id: string; name: string; time: number; version: string } | null>('version.previewState')
  return (
    <>
      {preview ? <VersionBanner version={preview} /> : focus ? <FocusBar focus={focus} /> : null}
      {dev.compare ? <CompareWindow id={dev.compare.id} snapshot={dev.compare.snapshot} /> : null}
    </>
  )
}

function VersionBanner({ version }: { version: { id: string; name: string; time: number; version: string } }) {
  const designKit = useDesignKit()
  const { t, run, toast } = designKit
  const name = version.name || t('Untitled version')
  return (
    <div className="design-dev-banner" data-testid="design-version-banner">
      <span className="design-dev-banner-icon">
        <Icon name="Clock" size={14} />
      </span>
      <span className="design-dev-banner-name">{t('Viewing “{name}”', { name })}</span>
      <span className="design-dev-banner-dim">{relativeTime(t, version.time / 1000)}</span>
      <Button onClick={() => void run('version.exitPreview', {})} testId="design-version-back">
        {t('Back to current')}
      </Button>
      <Button
        variant="primary"
        onClick={async () => {
          try {
            if ((await restoreVersion(designKit, version.id)) != null) toast(t('Restored “{name}”', { name }))
          } catch (error) {
            toast(refusal(t, error))
          }
        }}
        testId="design-version-restore"
      >
        {t('Restore this version')}
      </Button>
    </div>
  )
}

/** The focus view's bar (`dev_mode::focus_bar`), from `dev.focusState`. */
function FocusBar({ focus }: { focus: FocusState }) {
  const { t, run } = useDesignKit()
  const node = useQuery<{ type: string }>('node', { id: focus.id })
  const color = statusColor(focus.status, focus.changed)
  const label = focus.status === 'readyForDev' ? t(focus.changed ? 'Changed' : 'Ready for dev') : focus.status === 'completed' ? t('Completed') : null
  const at = focus.index
  return (
    <div className="design-dev-banner" data-testid="design-focus-bar">
      <span className="design-dev-banner-icon">
        <Icon name={node?.type === 'SECTION' ? 'Section' : 'Frame'} size={14} />
      </span>
      <span className="design-dev-banner-name">{focus.name}</span>
      {label && color ? (
        <span className="design-dev-badge" style={{ background: color }}>
          {label}
        </span>
      ) : null}
      {focus.canCompare ? (
        <Button variant="primary" onClick={() => void run('dev.compare', { id: focus.id, show: true })} testId="design-focus-compare">
          {t('Compare changes')}
        </Button>
      ) : null}
      {focus.canStep ? (
        <>
          <IconButton icon="ChevronLeft" label={t('Previous ready for dev')} onClick={() => void focusOn(run, null, 'previous')} testId="design-focus-previous" />
          <span className="design-dev-banner-dim">{at != null ? t('{n} of {total}', { n: at + 1, total: focus.count }) : t('{n} ready', { n: focus.count })}</span>
          <IconButton icon="ChevronRight" label={t('Next ready for dev')} onClick={() => void focusOn(run, null, 'next')} testId="design-focus-next" />
        </>
      ) : null}
      <IconButton icon="Close" label={t('Exit focus view (Esc)')} onClick={() => void run('dev.unfocus', {})} testId="design-focus-exit" />
    </div>
  )
}

type Change = { id: number; name: string; type: string; change: 'changed' | 'added' | 'removed'; depth: number; layers: number; properties: Array<{ text: string }> }
/** One side's picture (`dev.compare`'s `before` / `after`): its size in pixels and the outlined layers. */
type Side = { area: Rect; scale: number; width: number; height: number; boxes: Array<{ id: number; change: Change['change']; rect: [number, number, number, number] }> }
type Comparison = { id: number; name: string; changed: boolean; snapshot: boolean; truncated: boolean; changes: Change[]; added: number; removed: number; modified: number; version: string | null; before: Side | null; after: Side | null }
type Box = { id: number; kind: Change['change']; rect: Rect }

const KIND_COLORS = { added: '#087d4f', removed: '#f24822', changed: STATUS_COLORS.changed } as const
const LIST_W = 330

/** A side's picture (`devCompareRender`) as an object URL; null while it draws or when it can't. */
function useSidePicture(id: number, snapshot: string | null, side: 'before' | 'after', known: boolean): string | null {
  const { invoke, revision } = useDesignKit()
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    if (!known) {
      setUrl(null)
      return undefined
    }
    let live = true
    let made: string | null = null
    void invoke<Uint8Array>('devCompareRender', JSON.stringify({ id, snapshot, side })).then(
      (bytes) => {
        if (!live) return
        made = URL.createObjectURL(new Blob([toBytes(bytes) as BlobPart], { type: 'image/png' }))
        setUrl(made)
      },
      () => {
        if (live) setUrl(null)
      },
    )
    return () => {
      live = false
      if (made) {
        const old = made
        setTimeout(() => URL.revokeObjectURL(old), 1000)
      }
    }
  }, [id, invoke, known, revision, side, snapshot])
  return url
}

/** A side's outlines as fractions of its picture. */
const boxesOf = (side: Side | null): Box[] => (side ? side.boxes.map((b) => ({ id: b.id, kind: b.change, rect: { x: b.rect[0] / side.width, y: b.rect[1] / side.height, width: (b.rect[2] - b.rect[0]) / side.width, height: (b.rect[3] - b.rect[1]) / side.height } })) : [])

function CompareWindow({ id, snapshot }: { id: number; snapshot: string | null }) {
  const designKit = useDesignKit()
  const { t, run, toast } = designKit
  const [mode, setMode] = useState<'side' | 'overlay'>('side')
  // The snapshot (base64) of the saved version compared with; null: with when it was marked ready.
  const [against, setAgainst] = useState<string | null>(snapshot)
  const [hot, setHot] = useState<number | null>(null)
  const [split, setSplit] = useState(0.4)
  const cmp = useQuery<Comparison>('dev.compare', { id, snapshot: against })
  const node = useQuery<{ name: string; devStatus?: string }>('node', { id })
  const ready = node?.devStatus === 'readyForDev'
  const close = () => setDev({ compare: null })
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      if (document.querySelector('.design-layer')) return
      event.preventDefault()
      event.stopPropagation()
      close()
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [])
  const before = useSidePicture(id, against, 'before', cmp?.before != null)
  const after = useSidePicture(id, against, 'after', cmp?.after != null)
  // The saved versions (the host's checkpoints); the one compared with is named by its replica version.
  const versions = useCheckpoints(true).list
  const comparedWith = against && cmp?.version ? versions.find((v) => v.version === cmp.version) : undefined
  const versionName = against ? (comparedWith?.label ?? t('Saved version')) : null
  const width = Math.max(560, Math.min(1120, window.innerWidth - 64))
  const height = Math.max(320, Math.min(700, window.innerHeight - 96))
  const summary = !cmp
    ? ''
    : !cmp.snapshot
      ? t('Changes unknown')
      : cmp.changes.length === 0
        ? t('No changes')
        : t('{n} layers · {parts}', {
            n: cmp.changes.length,
            parts: (
              [
                [cmp.modified, '{n} changed'],
                [cmp.added, '{n} added'],
                [cmp.removed, '{n} removed'],
              ] as const
            )
              .filter(([n]) => n > 0)
              .map(([n, key]) => t(key, { n }))
              .join(', '),
          })
  let message: string | null = null
  if (cmp && !cmp.snapshot) message = against ? t("This version can't be read.") : cmp.changed ? t("What this changed from isn't known: it changed before Kition Design kept a copy of ready-for-dev frames. Resolve the change (or mark it ready again) to compare from now on.") : t('Nothing changed since this was marked ready for dev.')
  else if (cmp && !cmp.changed && against) message = t('Nothing changed since this version.')
  const picture = (side: 'before' | 'after') => {
    const s = side === 'before' ? cmp?.before : cmp?.after
    if (!s) return side === 'before' ? <span className="design-dev-compare-message">{t('Changes unknown')}</span> : null
    return <Picture size={s} image={side === 'before' ? before : after} boxes={boxesOf(s)} hot={hot} testId={`design-compare-${side === 'after' ? 'now' : 'before'}`} labels={[side === 'before' ? versionName ?? t('Before') : t('Now')]} />
  }
  return createPortal(
    <>
      <div className="design-dev-compare-scrim" onPointerDown={close} />
      <div className="design-dev-compare" role="dialog" aria-label={t('Compare changes')} style={{ width, height, left: (window.innerWidth - width) / 2, top: (window.innerHeight - height) / 2 }} data-testid="design-compare">
        <div className="design-dev-compare-head">
          <span className="design-dev-compare-title">{t('Compare changes')}</span>
          <span className="design-dev-compare-frame">{cmp?.name ?? node?.name ?? ''}</span>
          <div className="design-dev-switch">
            <button type="button" aria-pressed={mode === 'side'} onClick={() => setMode('side')} data-testid="design-compare-side">
              {t('Side by side')}
            </button>
            <button type="button" aria-pressed={mode === 'overlay'} onClick={() => setMode('overlay')} data-testid="design-compare-overlay">
              {t('Overlay')}
            </button>
          </div>
          <IconButton icon="Close" label={t('Close')} onClick={close} testId="design-compare-close" />
        </div>
        <div className="design-dev-compare-body">
          <div className="design-dev-compare-images">
            {message ? (
              <span className="design-dev-compare-message">{message}</span>
            ) : mode === 'side' ? (
              <>
                <div className="design-dev-compare-side">
                  <div className="design-dev-compare-side-label" />
                  <div className="design-dev-compare-stage">{picture('before')}</div>
                </div>
                <div className="design-dev-compare-side">
                  <div className="design-dev-compare-side-label" />
                  <div className="design-dev-compare-stage">{picture('after')}</div>
                </div>
              </>
            ) : (
              <div className="design-dev-compare-side">
                <div className="design-dev-compare-side-label" />
                <div className="design-dev-compare-stage">
                  {cmp?.after ? <Picture size={cmp.after} under={before} image={after} boxes={[...boxesOf(cmp.before), ...boxesOf(cmp.after)]} hot={hot} testId="design-compare-overlay-picture" split={split} onSplit={setSplit} labels={[versionName ?? t('Before'), t('Now')]} /> : null}
                </div>
              </div>
            )}
          </div>
          <div className="design-dev-compare-list" style={{ width: LIST_W }}>
            <div className="design-dev-compare-summary" data-testid="design-compare-summary">
              {summary}
            </div>
            <div className="design-dev-compare-rows">
              {(cmp?.changes ?? []).map((c) => (
                <div key={`${c.id}:${c.change}`} onPointerEnter={() => setHot(c.id)} onPointerLeave={() => setHot(null)}>
                  <div
                    className="design-dev-compare-row"
                    data-change={c.change}
                    style={{ paddingLeft: 16 + Math.min(c.depth, 6) * 10 }}
                    data-tip={c.change === 'removed' ? t(against ? 'Removed since this version' : 'Removed since it was marked') : t('Select layer')}
                    onClick={() => {
                      if (c.change !== 'removed') void run('select.set', { ids: [c.id] })
                    }}
                  >
                    <span className="design-dev-dot" style={{ background: KIND_COLORS[c.change], width: 6, height: 6, marginRight: 0 }} />
                    <span className="design-dev-compare-row-name">{c.layers > 1 ? t('{name} ({layers} layers)', { name: c.name, layers: c.layers }) : c.name}</span>
                    <span className="design-dev-compare-tag" style={{ color: KIND_COLORS[c.change], background: `color-mix(in srgb, ${KIND_COLORS[c.change]} 15%, transparent)` }}>
                      {t(c.change === 'added' ? 'Added' : c.change === 'removed' ? 'Removed' : 'Changed')}
                    </span>
                  </div>
                  {c.properties.length ? (
                    <div className="design-dev-compare-props" style={{ paddingLeft: 16 + Math.min(c.depth, 6) * 10 + 13 }}>
                      {c.properties.map((p, k) => (
                        <div key={k}>{p.text}</div>
                      ))}
                    </div>
                  ) : null}
                </div>
              ))}
              {cmp?.truncated ? <div className="design-note">{t('Only the first {n} changed layers are listed.', { n: cmp.changes.length })}</div> : null}
            </div>
          </div>
        </div>
        <div className="design-dev-compare-foot">
          <span className="design-dev-label">{t('Compare with')}</span>
          <MenuDropdown
            label={versionName ?? t('When marked ready')}
            icon="Clock"
            width={220}
            testId="design-compare-with"
            items={() => [
              ...(ready ? [{ label: t('When marked ready'), checked: against == null, onSelect: () => setAgainst(null) }] : []),
              ...versions.map((v) => ({
                label: v.label || t('Untitled version'),
                checked: against != null && comparedWith?.id === v.id,
                shortcut: relativeTime(t, v.time / 1000),
                onSelect: async () => {
                  try {
                    setAgainst((await loadCheckpoint(designKit, v.id)).snapshot)
                  } catch (error) {
                    toast(refusal(t, error))
                  }
                },
              })),
            ]}
          />
          <span className="design-dev-spacer" />
          <Button onClick={close} testId="design-compare-close-button">
            {t('Close')}
          </Button>
          {against == null ? (
            <Button
              variant="primary"
              disabled={!cmp?.changed}
              tip={t('Back to Ready for dev: the next change compares with how it is now')}
              onClick={async () => {
                if ((await run('dev.resolveChange', { ids: [id] })) != null) {
                  toast(t('Change resolved'))
                  close()
                }
              }}
              testId="design-compare-resolve"
            >
              {t('Resolve change')}
            </Button>
          ) : null}
        </div>
      </div>
    </>,
    document.body,
  )
}

/**
 * A side's picture fitted into its stage (at most 100 % of its pixels), the changed layers
 * outlined; in overlay mode `under` (before) shows left of the split and `image` (now) right.
 */
function Picture({ size: side, image, under, boxes, hot, testId, split, onSplit, labels }: { size: { width: number; height: number }; image: string | null; under?: string | null; boxes: Box[]; hot: number | null; testId: string; split?: number; onSplit?: (value: number) => void; /** Above the picture's left and right edges (upstream draws them at the image, not the column). */ labels?: [string, string?] }) {
  const stage = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState<{ width: number; height: number } | null>(null)
  useEffect(() => {
    const el = stage.current?.parentElement
    if (!el) return undefined
    const fit = () => {
      const k = Math.min(el.clientWidth / side.width, el.clientHeight / side.height, 1)
      setSize({ width: side.width * k, height: side.height * k })
    }
    fit()
    const observer = new ResizeObserver(fit)
    observer.observe(el)
    return () => observer.disconnect()
  }, [side.height, side.width])
  const outlines = useMemo(
    () =>
      boxes.map((b) => (
        <span
          key={`${b.id}:${b.kind}`}
          className="design-dev-compare-box"
          data-hot={hot === b.id || undefined}
          style={{ left: `${b.rect.x * 100}%`, top: `${b.rect.y * 100}%`, width: `${b.rect.width * 100}%`, height: `${b.rect.height * 100}%`, borderColor: KIND_COLORS[b.kind], background: hot === b.id ? `color-mix(in srgb, ${KIND_COLORS[b.kind]} 18%, transparent)` : undefined }}
        />
      )),
    [boxes, hot],
  )
  const drag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!onSplit) return
    const el = event.currentTarget
    el.setPointerCapture(event.pointerId)
    const at = (e: { clientX: number }) => {
      const r = el.getBoundingClientRect()
      onSplit(Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)))
    }
    at(event)
    const move = (e: PointerEvent) => at(e)
    const up = () => {
      el.removeEventListener('pointermove', move)
      el.removeEventListener('pointerup', up)
    }
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerup', up)
  }
  return (
    <div ref={stage} className="design-dev-compare-picture" style={{ ...(size ?? { width: 0, height: 0 }), cursor: onSplit ? 'ew-resize' : undefined }} data-testid={testId} data-loaded={image ? 'true' : undefined} onPointerDown={drag}>
      {labels ? (
        <div className="design-dev-compare-labels">
          <span>{labels[0]}</span>
          {labels[1] ? <span>{labels[1]}</span> : null}
        </div>
      ) : null}
      {under ? <img src={under} alt="" style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', clipPath: split != null ? `inset(0 ${(1 - split) * 100}% 0 0)` : undefined }} draggable={false} data-testid={`${testId}-under`} /> : null}
      {image ? <img src={image} alt="" style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', clipPath: split != null ? `inset(0 0 0 ${split * 100}%)` : undefined }} draggable={false} data-testid={`${testId}-image`} /> : null}
      {outlines}
      {split != null ? (
        <span className="design-dev-split" style={{ left: `${split * 100}%` }}>
          <span className="design-dev-split-knob">
            <Icon name="DistributeH" size={12} />
          </span>
        </span>
      ) : null}
    </div>
  )
}

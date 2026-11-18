/**
 * The right panel's header (2.1 §3): the top bar (your avatar and the people menu, the Present
 * split button, Share) and the tab row (Design, Prototype, the zoom dropdown and its menu).
 * Kition has no online share here, so Share stays disabled and says so.
 */
import { useRef, useState } from 'react'
import { Icon } from '../icons/Icon'
import { useChecked, useDesignKit, useView } from '../kit/hooks'
import { dispatchUiRequests } from '../kit/requests'
import { Menu, type MenuEntry, type MenuItem } from '../ui/Menu'
import { prettyShortcut, tip } from '../ui/shortcuts'

export function RightHeader() {
  const { t, ui } = useDesignKit()
  return (
    <>
      <TopBar />
      <div className="design-right-tabs" role="tablist" aria-label={t('Properties')}>
        {(['design', 'prototype'] as const).map((tab) => (
          <button
            key={tab}
            type="button"
            role="tab"
            className="design-right-tab"
            aria-selected={ui.rightTab === tab}
            data-testid={`design-tab-${tab}`}
            onClick={() => ui.setRightTab(tab)}
          >
            {t(tab === 'design' ? 'Design' : 'Prototype')}
          </button>
        ))}
        <ZoomButton />
      </div>
    </>
  )
}

function TopBar() {
  const { t, run } = useDesignKit()
  const [people, setPeople] = useState(false)
  const [present, setPresent] = useState(false)
  const peopleButton = useRef<HTMLButtonElement>(null)
  const presentRef = useRef<HTMLDivElement>(null)
  // The host names no person (the Kit UI protocol has no user): upstream's "You" initial.
  const name = ''
  const initial = 'Y'
  return (
    <div className="design-right-top">
      <span className="design-avatar" aria-hidden="true">
        {initial}
      </span>
      <button ref={peopleButton} type="button" className="design-people-chevron" aria-label={t('People in this file')} aria-expanded={people} onClick={() => setPeople((open) => !open)} data-testid="design-people">
        <Icon name="Chevron" size={10} />
      </button>
      {people ? (
        <Menu
          tone="light"
          anchor={peopleButton.current}
          placement="below-end"
          gap={0}
          minWidth={220}
          onClose={() => setPeople(false)}
          items={[
            // Upstream's row for you: your name and a 16 px avatar at the right; then a note.
            { label: t('{name} (you)', { name: name || t('You') }), onSelect: () => undefined, art: <span className="design-avatar-small">{initial}</span> },
            { type: 'custom', render: () => <div className="design-menu-note design-r11">{t('Not in a live session')}</div> },
          ]}
        />
      ) : null}
      <div ref={presentRef} className="design-present" data-open={present || undefined}>
        <button type="button" className="design-present-play" aria-label={t('Present')} data-tip={tip(t('Present'), 'Cmd+Alt+Enter')} onClick={() => dispatchUiRequests([{ kind: 'present', data: {} }])} data-testid="design-present">
          <svg width="16" height="16" viewBox="-8 -8 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" aria-hidden="true">
            <path d="M-5 -7 L7 0 L-5 7 Z" />
          </svg>
        </button>
        <button type="button" className="design-present-menu" aria-label={t('Present or preview')} aria-haspopup="menu" aria-expanded={present} data-tip={t('Present or preview')} onClick={() => setPresent((open) => !open)} data-testid="design-present-menu">
          <Icon name="Chevron" size={10} />
        </button>
      </div>
      {present ? (
        <Menu
          tone="light"
          anchor={presentRef.current}
          placement="below-end"
          gap={0}
          minWidth={200}
          onClose={() => setPresent(false)}
          items={[
            { label: t('Present'), shortcut: prettyShortcut('Cmd+Alt+Enter'), onSelect: () => dispatchUiRequests([{ kind: 'present', data: {} }]) },
            { label: t('Preview'), onSelect: () => void run('proto.preview', {}) },
          ]}
        />
      ) : null}
      <button type="button" className="design-share" disabled aria-disabled="true" data-tip={t('share.disabled')} data-testid="design-share">
        {t('Share')}
      </button>
    </div>
  )
}

function ZoomButton() {
  const { t, run, viewTo } = useDesignKit()
  const view = useView()
  const checked = useChecked()
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const zoom = view?.zoom ?? 1
  const check = (id: string, label: string, shortcut: string | null, params: Record<string, unknown> = {}): MenuItem => ({
    label: t(label),
    checked: checked.has(id),
    shortcut: prettyShortcut(shortcut),
    onSelect: () => void run(id, params),
    testId: `design-zoom-${label.replace(/\W+/g, '-').toLowerCase()}`,
  })
  const items: MenuEntry[] = [
    { label: t('Zoom in'), shortcut: prettyShortcut('Cmd+='), onSelect: () => void viewTo('view.zoomIn') },
    { label: t('Zoom out'), shortcut: prettyShortcut('Cmd+-'), onSelect: () => void viewTo('view.zoomOut') },
    { label: t('Zoom to fit'), shortcut: prettyShortcut('Shift+1'), onSelect: () => void viewTo('view.zoomFit') },
    { label: t('Zoom to 50%'), onSelect: () => void viewTo('view.zoom50') },
    { label: t('Zoom to 100%'), shortcut: prettyShortcut('Shift+0'), onSelect: () => void viewTo('view.zoom100') },
    { label: t('Zoom to 200%'), onSelect: () => void viewTo('view.zoom200') },
    { label: t('Zoom to selection'), shortcut: prettyShortcut('Shift+2'), onSelect: () => void viewTo('view.zoomSelection') },
    { type: 'separator' },
    // `checked` does not say which scale is on (see web/README.md, engine requests): 1x shows it.
    check('view.pixelPreview', 'Pixel preview 1x', null, { on: !checked.has('view.pixelPreview'), scale: 1 }),
    { ...(check('view.pixelPreview', 'Pixel preview 2x', null, { on: !checked.has('view.pixelPreview'), scale: 2 }) as MenuItem), checked: false },
    check('view.pixelGrid', 'Pixel grid', "Shift+'"),
    check('view.snapPixel', 'Snap to pixel grid', "Cmd+Shift+'"),
    check('view.layoutGrids', 'Layout grids', 'Shift+G'),
    check('view.rulers', 'Rulers', 'Shift+R'),
    check('view.outlines', 'Outlines', 'Cmd+Shift+O'),
  ]
  return (
    <>
      <button ref={button} type="button" className="design-zoom-button" aria-haspopup="menu" aria-expanded={open} aria-label={t('Zoom')} onClick={() => setOpen((now) => !now)} data-testid="design-zoom">
        {Math.round(zoom * 100)}%
        <Icon name="Chevron" size={10} />
      </button>
      {open ? <Menu tone="light" anchor={button.current} placement="below-end" gap={0} minWidth={200} onClose={() => setOpen(false)} items={items} label={t('Zoom')} testId="design-zoom-menu" /> : null}
    </>
  )
}

/**
 * The left panel (2.2, `layers.rs` `left_panel`): the 64 px header (the file name, renamed in
 * place, and "Drafts"; or "Assets") with Collapse panel, then Pages and Layers (the File tab) or
 * the Assets tab. Its parts sit 6 px apart, as egui's item spacing lays them out.
 */
import { useLayoutEffect, useRef, useState } from 'react'
import { useDesignKit } from '../kit/hooks'
import { useUiRequest } from '../kit/requests'
import { Icon } from '../icons/Icon'
import { Assets } from './Assets'
import { RenameInput } from './fields'
import { Layers } from './Layers'
import { Pages } from './Pages'
import { setLeft, useLeft } from './store'
import './left.css'

export function LeftPanel() {
  const designKit = useDesignKit()
  const { t, ui, inspect, run } = designKit
  const left = useLeft()
  const root = useRef<HTMLDivElement>(null)
  const [height, setHeight] = useState(800)
  const [renaming, setRenaming] = useState(false)
  useLayoutEffect(() => {
    const el = root.current
    if (!el) return undefined
    const observer = new ResizeObserver(() => setHeight(el.clientHeight))
    observer.observe(el)
    setHeight(el.clientHeight)
    return () => observer.disconnect()
  }, [])
  useUiRequest('panel.libraries', () => {
    ui.setLeftTab('assets')
    ui.setLeftOpen(true)
    setLeft({ libraries: true })
  })

  const assets = ui.leftTab === 'assets'
  // The room under the Pages header (and its search row) for the list, the splitter and the layers.
  const room = height - 64 - 6 - 40 - 6 - (left.pagesOpen && left.pageSearch ? 32 + 6 : 0)
  return (
    <div ref={root} className="design-left-panel" data-testid="design-left-panel">
      <div className="design-file-header" data-testid="design-file-header">
        {assets ? (
          <span className="design-file-title design-s13" style={{ left: 16 }}>
            {t('Assets')}
          </span>
        ) : renaming ? (
          <RenameInput
            value={inspect.name}
            bold
            style={{ left: 12, right: 46, top: 12, height: 24 }}
            testId="design-file-rename"
            onDone={(name) => {
              setRenaming(false)
              if (name != null) void run('file.rename', { name })
            }}
          />
        ) : (
          <button type="button" className="design-file-name design-s13" aria-label={t('Name')} onClick={() => setRenaming(true)} data-testid="design-file-name">
            {inspect.name}
          </button>
        )}
        {assets ? null : <span className="design-file-drafts design-r11">{t('Drafts')}</span>}
        <button type="button" className="design-left-header-button" style={{ right: 12, top: 12 }} aria-label={t('Collapse panel')} data-tip={t('Collapse panel')} data-testid="design-left-collapse" onClick={() => ui.setLeftOpen(false)}>
          <Icon name="Panels" size={14} />
        </button>
      </div>
      {assets ? (
        <Assets />
      ) : (
        <>
          <Pages room={room} />
          <Layers />
        </>
      )}
    </div>
  )
}

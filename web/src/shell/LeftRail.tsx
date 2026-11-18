/** The left rail (2.1 §2): the main menu button, then File, Assets and Variables. */
import { useRef } from 'react'
import { Icon, type IconName } from '../icons/Icon'
import { useDesignKit } from '../kit/hooks'
import { dispatchUiRequests } from '../kit/requests'
import { MainMenu } from '../menus/MainMenu'
import type { LeftTab } from '../kit/types'

export function LeftRail() {
  const { t, ui } = useDesignKit()
  const logo = useRef<HTMLButtonElement>(null)
  const items: Array<{ key: LeftTab | 'variables'; label: string; icon: IconName }> = [
    { key: 'file', label: 'File', icon: 'Page' },
    { key: 'assets', label: 'Assets', icon: 'Assets' },
    { key: 'variables', label: 'Variables', icon: 'Styles' },
  ]
  return (
    <>
      <button ref={logo} type="button" className="design-rail-logo" aria-label={t('Main menu')} aria-haspopup="menu" aria-expanded={ui.mainMenu} data-tip={t('Main menu')} data-testid="design-main-menu-button" onClick={() => ui.setMainMenu(!ui.mainMenu)}>
        <Icon name="PenNib" size={18} />
      </button>
      <span className="design-rail-divider" />
      <div role="tablist" aria-orientation="vertical" aria-label={t('Sidebar')}>
        {items.map((item, index) => {
          const selected = item.key !== 'variables' && ui.leftOpen && ui.leftTab === item.key
          return (
            <button
              key={item.key}
              type="button"
              role={item.key === 'variables' ? undefined : 'tab'}
              className="design-rail-item"
              style={{ top: 65 + index * 60 }}
              aria-selected={item.key === 'variables' ? undefined : selected}
              data-tip={t(item.label)}
              data-design-prefer={selected || undefined}
              data-testid={`design-rail-${item.key}`}
              onClick={() => {
                if (item.key === 'variables') dispatchUiRequests([{ kind: 'variables', data: {} }])
                else if (selected) ui.setLeftOpen(false)
                else {
                  ui.setLeftTab(item.key)
                  ui.setLeftOpen(true)
                }
              }}
            >
              <span className="design-rail-item-box">
                <Icon name={item.icon} size={16} />
              </span>
              <span className="design-rail-item-label">{t(item.label)}</span>
            </button>
          )
        })}
      </div>
      {ui.mainMenu ? <MainMenu anchor={logo.current} onClose={() => ui.setMainMenu(false)} /> : null}
    </>
  )
}

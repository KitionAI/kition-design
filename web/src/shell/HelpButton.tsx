/**
 * The help button (2.1 §4.7): a `?` circle at the canvas's bottom-right; its menu asks for help UI
 * (the menus package shows it). Getting started is upstream's home-screen tips, which Kition does
 * not have, so it is left out; Help page opens through the host's `host/openUrl`, and only shows
 * when the host serves it (`open-url`).
 */
import { useRef, useState } from 'react'
import { useDesignKit } from '../kit/hooks'
import { dispatchUiRequests } from '../kit/requests'
import { Menu } from '../ui/Menu'
import { prettyShortcut } from '../ui/shortcuts'

/** Where Help page goes. */
export const HELP_URL = 'https://kition.ai'

export function HelpButton() {
  const { t, editor } = useDesignKit()
  const { host } = editor
  const canOpenUrl = host.init.capabilities.includes('open-url')
  const openHelp = () => void host.peer.request('host/openUrl', { url: HELP_URL }).catch(() => undefined)
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const ask = (kind: string) => () => dispatchUiRequests([{ kind, data: {} }])
  return (
    <>
      <button ref={button} type="button" className="design-help" aria-label={t('Help and resources')} aria-haspopup="menu" aria-expanded={open} data-tip={t('Help and resources')} onClick={() => setOpen((now) => !now)} data-testid="design-help">
        ?
      </button>
      {open ? (
        <Menu
          tone="light"
          anchor={button.current}
          placement="above-start"
          gap={0}
          minWidth={220}
          onClose={() => setOpen(false)}
          items={[
            { label: t('Keyboard shortcuts'), shortcut: prettyShortcut('Ctrl+Shift+?'), onSelect: ask('panel.shortcuts') },
            ...(canOpenUrl ? [{ label: t('Help page'), onSelect: openHelp }] : []),
            { label: t('About Kition Design'), onSelect: ask('dialog.about') },
          ]}
        />
      ) : null}
    </>
  )
}

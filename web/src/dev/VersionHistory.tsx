/**
 * The version history panel (2.6, upstream `panels/versions.rs`): replaces Pages and Layers in
 * the left panel while open. Saved versions are the host's checkpoints (Kit UI protocol section
 * 7m, `checkpoints.ts`): the save form saves the design and records one, the timeline lists them
 * newest first (older ones a page at a time), and a row's menu renames, restores or deletes it. A
 * click previews a version on the canvas (the banner over the canvas has Back to current and
 * Restore). A version the host's history no longer reaches is listed and can't be shown; without
 * the capability the panel says why it can't save versions.
 */
import { useRef, useState, type KeyboardEvent, type MouseEvent } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Button, IconButton, Menu, Note, Separator, tip, type MenuEntry } from '../ui'
import { deleteVersion, ownCheckpoint, previewVersion, refusal, renameVersion, restoreVersion, saveVersion, useCheckpoints, type KitCheckpoint } from './checkpoints'
import { relativeTime, setDev, useDev } from './store'
import './dev.css'

/** Whether the version history panel is open (it replaces the left panel's body). */
export function useVersionHistoryOpen(): boolean {
  return useDev().versions.open
}

type FormProps = { title: string; placeholder: string; description?: string | null; submit: string; onSubmit: (name: string, description: string) => void; onCancel: () => void; requireTitle?: boolean; busy?: boolean; testId?: string }

/** The save and rename form: a title, a description, Save and Cancel. */
function Form({ title, placeholder, description, submit, onSubmit, onCancel, requireTitle, busy, testId }: FormProps) {
  const { t } = useDesignKit()
  const [name, setName] = useState(title)
  const [text, setText] = useState(description ?? '')
  const send = () => {
    if ((requireTitle && !name.trim()) || busy) return
    onSubmit(name.trim(), text.trim())
  }
  const keys = (multiline: boolean) => (event: KeyboardEvent) => {
    event.stopPropagation()
    if (event.key === 'Escape') onCancel()
    else if (event.key === 'Enter' && (!multiline || event.metaKey || event.ctrlKey)) {
      event.preventDefault()
      send()
    }
  }
  return (
    <div className="design-versions-form" data-testid={testId}>
      <input autoFocus value={name} placeholder={placeholder} aria-label={placeholder} spellCheck={false} onChange={(event) => setName(event.target.value)} onKeyDown={keys(false)} data-testid={testId ? `${testId}-title` : undefined} />
      {description !== null ? <textarea rows={3} value={text} placeholder={t('Describe what changed')} aria-label={t('Describe what changed')} onChange={(event) => setText(event.target.value)} onKeyDown={keys(true)} data-testid={testId ? `${testId}-description` : undefined} /> : null}
      <div className="design-versions-form-buttons">
        <Button variant="primary" onClick={send} disabled={(requireTitle && !name.trim()) || busy} testId={testId ? `${testId}-submit` : undefined}>
          {submit}
        </Button>
        <Button onClick={onCancel}>{t('Cancel')}</Button>
      </div>
    </div>
  )
}

export function VersionHistory() {
  const designKit = useDesignKit()
  const { t, run, toast } = designKit
  const dev = useDev()
  const versions = useCheckpoints(true)
  const previewing = useQuery<{ id: string } | null>('version.previewState')?.id ?? null
  const [renaming, setRenaming] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [menu, setMenu] = useState<{ id: string; anchor: Element | { left: number; top: number; right: number; bottom: number } } | null>(null)
  const close = () => {
    setDev((now) => ({ versions: { ...now.versions, open: false, saving: false } }))
    void run('version.exitPreview', {})
  }
  const saving = dev.versions.saving && versions.available
  const setSaving = (on: boolean) => setDev((now) => ({ versions: { ...now.versions, saving: on } }))
  /** Runs a host call, saying why when the host refuses it. */
  const attempt = async <R,>(work: () => Promise<R>): Promise<R | null> => {
    try {
      return await work()
    } catch (error) {
      toast(refusal(t, error))
      return null
    }
  }
  const nameOf = (v: KitCheckpoint) => v.label || t('Untitled version')

  const menuItems = (v: KitCheckpoint): MenuEntry[] => [
    {
      label: t('Restore this version'),
      onSelect: async () => {
        if ((await attempt(() => restoreVersion(designKit, v.id))) != null) toast(t('Restored “{name}”', { name: nameOf(v) }))
      },
      testId: 'design-version-menu-restore',
    },
    ...(ownCheckpoint(v)
      ? [
          { label: t('Rename'), onSelect: () => setRenaming(v.id), testId: 'design-version-menu-rename' },
          { label: t('Delete'), onSelect: () => setDeleting(v.id), testId: 'design-version-menu-delete' },
        ]
      : []),
  ]

  return (
    <div className="design-versions" data-testid="design-version-history" data-available={versions.available}>
      <div className="design-versions-head">
        <span className="design-versions-title">{t('Version history')}</span>
        <IconButton icon="Plus" label={tip(t('Save version'), 'Cmd+Alt+S')} pressed={saving} disabled={!versions.available} onClick={() => setSaving(!saving)} testId="design-version-save-toggle" />
        <IconButton icon="Close" label={t('Close version history')} onClick={close} testId="design-version-close" />
      </div>
      {!versions.available ? (
        <div className="design-versions-scroll" data-testid="design-version-unavailable">
          <Note>{t('Saved versions are kept by the app that opened this file, and this one keeps none. Save a copy of the file to keep a version of it.')}</Note>
        </div>
      ) : (
        <>
          {saving ? (
            <>
              <Form
                title=""
                placeholder={t('Title')}
                submit={t('Save')}
                busy={busy}
                testId="design-version-save-form"
                onCancel={() => setSaving(false)}
                onSubmit={async (name, description) => {
                  setBusy(true)
                  try {
                    const label = name || t('Version {n}', { n: versions.list.length + 1 })
                    if (await saveVersion(designKit, label, description)) setSaving(false)
                    else toast(t('The design was saved, but no version of it was kept'))
                  } catch (error) {
                    toast(refusal(t, error))
                  } finally {
                    setBusy(false)
                  }
                }}
              />
              <Separator />
            </>
          ) : null}
          <div className="design-versions-scroll">
            <div className="design-versions-timeline">
              <div className="design-version-row" data-current="true" data-selected={!previewing || undefined} data-tip={t('Back to the current version')} onClick={() => void run('version.exitPreview', {})} data-testid="design-version-current">
                <span className="design-version-dot" />
                {t('Current version')}
              </div>
              {versions.list.map((v, index) =>
                renaming === v.id ? (
                  <Form
                    key={v.id}
                    title={v.label ?? ''}
                    placeholder={t('Title')}
                    description={v.description ?? ''}
                    submit={t('Save')}
                    requireTitle
                    testId="design-version-rename-form"
                    onCancel={() => setRenaming(null)}
                    onSubmit={async (name, description) => {
                      if ((await attempt(() => renameVersion(designKit, v, name, description))) != null) setRenaming(null)
                    }}
                  />
                ) : deleting === v.id ? (
                  <div key={v.id} className="design-versions-confirm">
                    <span>{t('Delete “{name}”? This can\'t be undone.', { name: nameOf(v) })}</span>
                    <div className="design-versions-form-buttons">
                      <Button
                        variant="primary"
                        testId="design-version-delete-confirm"
                        onClick={async () => {
                          await attempt(() => deleteVersion(designKit, v))
                          setDeleting(null)
                        }}
                      >
                        {t('Delete')}
                      </Button>
                      <Button onClick={() => setDeleting(null)}>{t('Cancel')}</Button>
                    </div>
                  </div>
                ) : (
                  <VersionRow
                    key={v.id}
                    version={v}
                    index={index}
                    name={nameOf(v)}
                    previewing={previewing === v.id}
                    menuOpen={menu?.id === v.id}
                    onPreview={() => void attempt(() => previewVersion(designKit, v))}
                    onMenu={(anchor) => setMenu({ id: v.id, anchor })}
                    onRename={() => {
                      if (ownCheckpoint(v)) setRenaming(v.id)
                    }}
                  />
                ),
              )}
            </div>
            {versions.more ? (
              <div className="design-versions-form-buttons" style={{ padding: '8px 16px' }}>
                <Button onClick={versions.older} testId="design-version-older">
                  {t('Show older versions')}
                </Button>
              </div>
            ) : null}
            {versions.error ? <Note>{versions.error}</Note> : null}
            {versions.loaded && versions.list.length === 0 && !versions.error ? <Note>{t('No saved versions yet. Save a version to mark a point in your work you can come back to.')}</Note> : null}
          </div>
        </>
      )}
      {menu ? (
        <Menu
          tone="light"
          anchor={menu.anchor}
          placement="below-end"
          minWidth={180}
          onClose={() => setMenu(null)}
          items={(() => {
            const v = versions.list.find((item) => item.id === menu.id)
            return v ? menuItems(v) : []
          })()}
          testId="design-version-menu"
        />
      ) : null}
    </div>
  )
}

type RowProps = {
  version: KitCheckpoint
  index: number
  name: string
  previewing: boolean
  menuOpen: boolean
  onPreview: () => void
  onMenu: (anchor: Element | { left: number; top: number; right: number; bottom: number }) => void
  onRename: () => void
}

function VersionRow({ version, index, name, previewing, menuOpen, onPreview, onMenu, onRename }: RowProps) {
  const { t } = useDesignKit()
  const more = useRef<HTMLSpanElement>(null)
  return (
    <div
      className="design-version-row"
      data-version="true"
      data-selected={previewing || undefined}
      data-testid={`design-version-${index}`}
      data-checkpoint={version.id}
      onClick={onPreview}
      onDoubleClick={onRename}
      onContextMenu={(event: MouseEvent) => {
        event.preventDefault()
        onMenu({ left: event.clientX, top: event.clientY, right: event.clientX, bottom: event.clientY })
      }}
    >
      <span className="design-version-dot" />
      <div className="design-version-name">{name}</div>
      <div className="design-version-time">{relativeTime(t, version.time / 1000)}</div>
      {version.description ? <div className="design-version-desc">{version.description}</div> : null}
      <span ref={more} className="design-version-more" aria-expanded={menuOpen} onClick={(event) => event.stopPropagation()}>
        <IconButton icon="Dots" label={t('More')} onClick={() => more.current && onMenu(more.current)} testId={`design-version-more-${index}`} />
      </span>
    </div>
  )
}

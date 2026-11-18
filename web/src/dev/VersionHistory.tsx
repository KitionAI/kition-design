/**
 * The version history panel (2.6, upstream `panels/versions.rs`): replaces Pages and Layers in
 * the left panel while open. The save form, branches, the timeline (current version, saved
 * versions with their menu, folded autosaves), rename and delete forms. A click previews a
 * version on the canvas (the banner over the canvas has Back to current and Restore).
 */
import { useRef, useState, type KeyboardEvent, type MouseEvent } from 'react'
import { Icon } from '../icons/Icon'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Button, IconButton, Menu, Note, SectionHeader, Separator, tip, type MenuEntry } from '../ui'
import { relativeTime, setDev, useDev } from './store'
import './dev.css'

type Version = { id: string; index: number; name: string; description?: string; time: number; auto?: boolean; previewing?: boolean }
type Branch = { id: string; name: string; time?: number; status?: string; active?: boolean; merged?: boolean; archived?: boolean }

/** Whether the version history panel is open (it replaces the left panel's body). */
export function useVersionHistoryOpen(): boolean {
  return useDev().versions.open
}

type FormProps = { title: string; placeholder: string; description?: string | null; submit: string; onSubmit: (name: string, description: string) => void; onCancel: () => void; requireTitle?: boolean; testId?: string }

/** The save, rename and branch form: a title, a description (not for branches), Save and Cancel. */
function Form({ title, placeholder, description, submit, onSubmit, onCancel, requireTitle, testId }: FormProps) {
  const { t } = useDesignKit()
  const [name, setName] = useState(title)
  const [text, setText] = useState(description ?? '')
  const send = () => {
    if (requireTitle && !name.trim()) return
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
        <Button variant="primary" onClick={send} disabled={requireTitle && !name.trim()} testId={testId ? `${testId}-submit` : undefined}>
          {submit}
        </Button>
        <Button onClick={onCancel}>{t('Cancel')}</Button>
      </div>
    </div>
  )
}

export function VersionHistory() {
  const { t, run, toast: dispatchToast, editor } = useDesignKit()
  const dev = useDev()
  const versions = useQuery<Version[]>('versions') ?? []
  // Branches open in tabs of their own, which a kit does not have: the engine's catalog leaves
  // `branch.open` out, and the section shows only where it is offered.
  const branching = editor.byId.has('branch.open')
  const branches = useQuery<Branch[]>('branches', {}, { skip: !branching }) ?? []
  const previewing = versions.find((v) => v.previewing) ?? null
  const [renaming, setRenaming] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<string | null>(null)
  const [unfolded, setUnfolded] = useState<Set<string>>(new Set())
  const [menu, setMenu] = useState<{ id: string; anchor: Element | { left: number; top: number; right: number; bottom: number } } | null>(null)
  const close = () => {
    setDev((now) => ({ versions: { ...now.versions, open: false, saving: false, branching: false } }))
    void run('version.exitPreview', {})
  }
  const saving = dev.versions.saving
  const setSaving = (on: boolean) => setDev((now) => ({ versions: { ...now.versions, saving: on } }))
  const setBranching = (on: boolean) => setDev((now) => ({ versions: { ...now.versions, branching: on } }))

  // Newest first; runs of two or more autosaves fold into one row.
  const newest = [...versions].reverse()
  type Item = { kind: 'version'; v: Version } | { kind: 'fold'; key: string; list: Version[] }
  const items: Item[] = []
  for (let i = 0; i < newest.length; ) {
    let j = i
    while (j < newest.length && newest[j].auto) j += 1
    if (j - i >= 2) {
      const list = newest.slice(i, j)
      const key = list[0].id
      const open = unfolded.has(key) || list.some((v) => v.previewing)
      items.push({ kind: 'fold', key, list })
      if (open) for (const v of list) items.push({ kind: 'version', v })
      i = j
    } else {
      items.push({ kind: 'version', v: newest[i] })
      i += 1
    }
  }

  const menuItems = (v: Version): MenuEntry[] => [
    {
      label: t('Restore this version'),
      onSelect: async () => {
        if ((await run('version.restore', { id: v.id })) != null) dispatchToast(t('Restored “{name}”', { name: nameOf(v) }))
      },
    },
    { label: t('Rename'), onSelect: () => setRenaming(v.id) },
    {
      label: t('Duplicate'),
      onSelect: async () => {
        if ((await run('version.duplicate', { id: v.id })) != null) dispatchToast(t('Opened “{name}” in a new tab', { name: nameOf(v) }))
      },
    },
    { label: t('Delete'), onSelect: () => setDeleting(v.id), testId: 'design-version-menu-delete' },
  ]
  const nameOf = (v: Version) => v.name || t('Autosave')

  return (
    <div className="design-versions" data-testid="design-version-history">
      <div className="design-versions-head">
        <span className="design-versions-title">{t('Version history')}</span>
        <IconButton icon="Plus" label={tip(t('Save version'), 'Cmd+Alt+S')} pressed={saving} onClick={() => setSaving(!saving)} testId="design-version-save-toggle" />
        <IconButton icon="Close" label={t('Close version history')} onClick={close} testId="design-version-close" />
      </div>
      {saving ? (
        <>
          <Form
            title=""
            placeholder={t('Title')}
            submit={t('Save')}
            testId="design-version-save-form"
            onCancel={() => setSaving(false)}
            onSubmit={async (name, description) => {
              // Both always go (an empty title is the engine's "Version n"): without values the
              // command would only open this form again (menus/invoke.ts).
              if ((await run('file.saveVersion', { name, description })) != null) setSaving(false)
            }}
          />
          <Separator />
        </>
      ) : null}
      <div className="design-versions-scroll">
        {branching ? (
          <>
            <SectionHeader title={t('Branches')} actions={[{ icon: 'Plus', label: t('Create branch'), onClick: () => setBranching(!dev.versions.branching) }]} />
            {dev.versions.branching ? (
              <Form
                title=""
                placeholder={t('Branch name')}
                description={null}
                submit={t('Create')}
                onCancel={() => setBranching(false)}
                onSubmit={async (name) => {
                  if ((await run('branch.create', name ? { name } : {})) != null) setBranching(false)
                }}
              />
            ) : null}
            {branches.length === 0 ? (
              <Note>{t('No branches. A branch is a copy of the file to try changes in, then review and merge.')}</Note>
            ) : (
              branches.map((b) => <BranchRow key={b.id} branch={b} />)
            )}
            <div style={{ height: 8 }} />
            <Separator />
            <div style={{ height: 4 }} />
          </>
        ) : null}
        <div className="design-versions-timeline">
          <div className="design-version-row" data-current="true" data-selected={!previewing || undefined} data-tip={t('Back to the current version')} onClick={() => void run('version.exitPreview', {})} data-testid="design-version-current">
            <span className="design-version-dot" />
            {t('Current version')}
          </div>
          {items.map((item) =>
            item.kind === 'fold' ? (
              <div
                key={`fold:${item.key}`}
                className="design-version-row design-version-fold"
                data-tip={t(unfolded.has(item.key) ? 'Fold autosaves' : 'Show autosaves')}
                onClick={() =>
                  setUnfolded((now) => {
                    const next = new Set(now)
                    if (next.has(item.key)) next.delete(item.key)
                    else next.add(item.key)
                    return next
                  })
                }
              >
                <span className="design-version-dot" />
                <Icon name={unfolded.has(item.key) || item.list.some((v) => v.previewing) ? 'Chevron' : 'ChevronRight'} size={12} />
                {t('{n} autosave versions', { n: item.list.length })}
              </div>
            ) : renaming === item.v.id ? (
              <Form
                key={item.v.id}
                title={item.v.name}
                placeholder={t('Title')}
                description={item.v.description ?? ''}
                submit={t('Save')}
                requireTitle
                onCancel={() => setRenaming(null)}
                onSubmit={async (name, description) => {
                  if ((await run('version.rename', { id: item.v.id, name, description })) != null) setRenaming(null)
                }}
              />
            ) : deleting === item.v.id ? (
              <div key={item.v.id} className="design-versions-confirm">
                <span>{t('Delete “{name}”? This can\'t be undone.', { name: nameOf(item.v) })}</span>
                <div className="design-versions-form-buttons">
                  <Button
                    variant="primary"
                    testId="design-version-delete-confirm"
                    onClick={async () => {
                      await run('version.delete', { id: item.v.id })
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
                key={item.v.id}
                version={item.v}
                name={nameOf(item.v)}
                menuOpen={menu?.id === item.v.id}
                onMenu={(anchor) => setMenu({ id: item.v.id, anchor })}
                onRename={() => setRenaming(item.v.id)}
              />
            ),
          )}
        </div>
        {versions.length === 0 ? <Note>{t('No saved versions yet. Save a version to mark a point in your work you can come back to.')}</Note> : null}
      </div>
      {menu ? (
        <Menu
          tone="light"
          anchor={menu.anchor}
          placement="below-end"
          minWidth={180}
          onClose={() => setMenu(null)}
          items={(() => {
            const v = versions.find((item) => item.id === menu.id)
            return v ? menuItems(v) : []
          })()}
          testId="design-version-menu"
        />
      ) : null}
    </div>
  )
}

function VersionRow({ version, name, menuOpen, onMenu, onRename }: { version: Version; name: string; menuOpen: boolean; onMenu: (anchor: Element | { left: number; top: number; right: number; bottom: number }) => void; onRename: () => void }) {
  const { t, run } = useDesignKit()
  const more = useRef<HTMLSpanElement>(null)
  return (
    <div
      className="design-version-row"
      data-version="true"
      data-selected={version.previewing || undefined}
      data-testid={`design-version-${version.index}`}
      onClick={() => void run('version.preview', { id: version.id })}
      onDoubleClick={onRename}
      onContextMenu={(event: MouseEvent) => {
        event.preventDefault()
        onMenu({ left: event.clientX, top: event.clientY, right: event.clientX, bottom: event.clientY })
      }}
    >
      <span className="design-version-dot" />
      <div className="design-version-name">{name}</div>
      <div className="design-version-time">{relativeTime(t, version.time)}</div>
      {version.description ? <div className="design-version-desc">{version.description}</div> : null}
      <span ref={more} className="design-version-more" aria-expanded={menuOpen} onClick={(event) => event.stopPropagation()}>
        <IconButton icon="Dots" label={t('More')} onClick={() => more.current && onMenu(more.current)} testId={`design-version-more-${version.index}`} />
      </span>
    </div>
  )
}

function BranchRow({ branch }: { branch: Branch }) {
  const { t, run, toast } = useDesignKit()
  const [menu, setMenu] = useState<Element | { left: number; top: number; right: number; bottom: number } | null>(null)
  const more = useRef<HTMLSpanElement>(null)
  const state = branch.status ?? (branch.archived ? 'archived' : branch.merged ? 'merged' : 'active')
  const active = state === 'active'
  const label = t(state === 'merged' ? 'Merged' : state === 'archived' ? 'Archived' : 'Active')
  const items: MenuEntry[] = [
    { label: t('Open'), onSelect: () => void run('branch.open', { id: branch.id }) },
    // Review and merge (upstream's branch review dialog) is left out: branch review is deferred in Kition.
    ...(active
      ? [
          {
            label: t('Archive'),
            onSelect: async () => {
              if ((await run('branch.archive', { id: branch.id })) != null) toast(t('Branch archived'))
            },
          },
        ]
      : []),
  ]
  return (
    <div
      className="design-version-row design-branch-row"
      data-active={active}
      data-tip={t('Open branch')}
      onClick={() => void run('branch.open', { id: branch.id })}
      onContextMenu={(event) => {
        event.preventDefault()
        setMenu({ left: event.clientX, top: event.clientY, right: event.clientX, bottom: event.clientY })
      }}
    >
      <div className="design-branch-name">{branch.name}</div>
      <div className="design-branch-sub">
        {label}
        {branch.time ? ` · ${relativeTime(t, branch.time)}` : ''}
      </div>
      <span ref={more} className="design-version-more" onClick={(event) => event.stopPropagation()}>
        <IconButton icon="Dots" label={t('More')} onClick={() => setMenu(more.current)} />
      </span>
      {menu ? <Menu tone="light" anchor={menu} placement="below-end" minWidth={180} onClose={() => setMenu(null)} items={items} /> : null}
    </div>
  )
}

/**
 * The Assets tab's libraries view (`assets_panel.rs` `libraries_view`): publish this file as a
 * library (its components, styles and variable collections, with a description), and the
 * libraries this file uses (remove, swap one for another, add a library file of the workspace
 * with "Add library file…", or one by a team library's link), from `query('library.list')`
 * (asked again after every change). A library file is chosen by link with the host's chooser
 * (`links/pick`, Kit UI protocol 2.5): it stays where it is, never copied, and the host names it
 * relative to this file's folder; it is read and lent to the engine (`libraryFiles.ts`), then
 * added (`library.add`). Without `links-pick` (a read-only file, a host without the chooser) the
 * button is not shown.
 */
import { useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Button, Checkbox, TextField } from '../ui'
import { Menu } from '../ui/Menu'
import { devRefuses } from './dev'
import { lendLibraryFile } from './libraryFiles'
import { setLeft } from './store'

type Item = { id: number | string; name: string; selected: boolean }
type Library = { id: string; name: string; path: string; version?: number; error?: string; components?: unknown[]; styles?: unknown[]; variables?: unknown[] }
type Listing = {
  libraries: Library[]
  published: { version: number; description: string } | null
  publishable?: { components: Item[]; styles: Item[]; collections: Item[] }
}

/** The upstream text names its own file type; the kit's files are `.kdesign`. */

export function Libraries() {
  const designKit = useDesignKit()
  const { t, run, inspect } = designKit
  const listing = useQuery<Listing>('library.list', { assets: true })
  const [picked, setPicked] = useState<Record<string, boolean>>({})
  const [description, setDescription] = useState<string | null>(null)
  const [swap, setSwap] = useState<{ from: Library; x: number; y: number } | null>(null)
  const groups: Array<[string, string, Item[]]> = listing?.publishable
    ? [
        ['Components', 'c', listing.publishable.components],
        ['Styles', 's', listing.publishable.styles],
        ['Variable collections', 'v', listing.publishable.collections],
      ]
    : []
  const include = groups.flatMap(([, prefix, items]) => items.filter((item) => picked[`${prefix}${item.id}`] ?? item.selected).map((item) => item.id))
  const nothing = groups.every(([, , items]) => items.length === 0)
  const published = listing?.published ?? null
  const text = description ?? published?.description ?? ''

  const publish = async () => {
    if (devRefuses(designKit, 'library.publish')) return
    const answer = await run<{ version?: number }>('library.publish', { description: text, include })
    if (!answer) return
    setPicked({})
    setDescription(null)
    designKit.toast(t('Published version {v}: save the file so other files can use it', { v: answer.version ?? 0 }))
  }

  const canPick = designKit.editor.host.init.capabilities.includes('links-pick')
  const addFile = async () => {
    if (devRefuses(designKit, 'library.add')) return
    try {
      const [picked] = await designKit.editor.host.pickLinks({ purpose: 'Choose a library file', accept: ['kdesign'] })
      if (!picked) return
      const { path } = picked
      await lendLibraryFile(designKit, path)
      const answer = await run<{ name?: string }>('library.add', { path })
      if (answer) designKit.toast(t('Added the library "{name}"', { name: answer.name ?? '' }))
    } catch (error) {
      designKit.toast(error instanceof Error ? error.message : String(error))
    }
  }

  const libraries = listing?.libraries ?? []
  return (
    <>
      <div className="design-libraries-header">
        <button type="button" className="design-left-header-button" style={{ left: 12 }} aria-label={t('Back to assets')} data-tip={t('Back to assets')} onClick={() => setLeft({ libraries: false })} data-testid="design-libraries-back">
          <Icon name="ChevronLeft" size={14} />
        </button>
        <span className="design-s12" style={{ position: 'absolute', left: 42 }}>
          {t('Libraries')}
        </span>
      </div>
      <hr className="design-separator" />
      <div className="design-assets-scroll design-libraries" data-testid="design-libraries">
        <div className="design-assets-title">{t('This file')}</div>
        <div className="design-assets-hint">{published ? t('{name} · published version {v}', { name: inspect.name, v: published.version }) : t('{name} · not published', { name: inspect.name })}</div>
        {listing && nothing ? <div className="design-assets-hint">{t('Create components, styles or variables to publish them as a library.')}</div> : null}
        {listing && !nothing ? (
          <>
            {groups.map(([label, prefix, items]) =>
              items.length ? (
                <div key={prefix}>
                  <div className="design-sublabels" style={{ paddingLeft: 16 }}>
                    <span>{t(label)}</span>
                  </div>
                  {items.map((item) => (
                    <div key={String(item.id)} className="design-libraries-check">
                      <Checkbox checked={picked[`${prefix}${item.id}`] ?? item.selected} label={item.name} onChange={(on) => setPicked((now) => ({ ...now, [`${prefix}${item.id}`]: on }))} />
                    </div>
                  ))}
                </div>
              ) : null,
            )}
            <div className="design-libraries-field">
              <TextField value={text} placeholder={t('Description (optional)')} onCommit={setDescription} testId="design-libraries-description" />
            </div>
            <div className="design-libraries-publish">
              <Button variant="primary" disabled={include.length === 0} onClick={() => void publish()} testId="design-libraries-publish">
                {published ? t('Publish changes') : t('Publish')}
              </Button>
            </div>
          </>
        ) : null}
        <hr className="design-separator" />
        <div className="design-assets-title">{t('Libraries in this file')}</div>
        {listing && libraries.length === 0 ? <div className="design-assets-hint">{t("No libraries yet. Add a .kdesign file that was published as a library, or paste a team library's link.")}</div> : null}
        {libraries.map((library) => (
          <div key={library.id} className="design-library-row" data-tip={library.path} data-testid={`design-library-${library.name}`}>
            <span className="design-library-name design-s11">{library.name}</span>
            <span className="design-library-status design-r10" data-error={library.error ? true : undefined}>
              {library.error ?? t('Version {v} · {components} · {styles} · {variables}', {
                v: library.version ?? 0,
                components: t('{n} components', { n: library.components?.length ?? 0 }),
                styles: t('{n} styles', { n: library.styles?.length ?? 0 }),
                variables: t('{n} variables', { n: library.variables?.length ?? 0 }),
              })}
            </span>
            {libraries.length >= 2 ? (
              <button type="button" className="design-left-header-button" style={{ right: 40 }} aria-label={t('Swap library: use another library\'s assets of the same names')} data-tip={t('Swap library: use another library\'s assets of the same names')} onClick={(event) => setSwap({ from: library, x: event.clientX, y: event.clientY })}>
                <Icon name="Swap" size={14} />
              </button>
            ) : null}
            <button type="button" className="design-left-header-button" style={{ right: 12 }} aria-label={t('Remove library from this file')} data-tip={t('Remove library from this file')} data-testid={`design-library-remove-${library.name}`} onClick={() => !devRefuses(designKit, 'library.remove') && void run('library.remove', { id: library.id })}>
              <Icon name="Close" size={14} />
            </button>
          </div>
        ))}
        {canPick ? (
          <div className="design-libraries-add">
            <Button onClick={() => void addFile()} testId="design-libraries-add-file">
              {t('Add library file…')}
            </Button>
          </div>
        ) : null}
        <div style={{ padding: '4px 16px 8px' }}>
          <TextField
            value=""
            placeholder={t('Paste a team library link (a view link)')}
            onCommit={(link) => {
              if (link.trim() && !devRefuses(designKit, 'library.add')) void run<{ name?: string }>('library.add', { path: link.trim() }).then((answer) => {
                  if (answer) designKit.toast(t('Added the library "{name}"', { name: answer.name ?? '' }))
                })
            }}
          />
        </div>
      </div>
      {swap ? (
        <Menu
          tone="light"
          anchor={{ left: swap.x, top: swap.y, right: swap.x, bottom: swap.y }}
          onClose={() => setSwap(null)}
          items={libraries.filter((l) => l.id !== swap.from.id).map((to) => ({ label: t('Swap to {name}', { name: to.name }), onSelect: () => void run('library.swap', { from: swap.from.id, to: to.id }) }))}
        />
      ) : null}
    </>
  )
}

/**
 * The component blocks under the selection header (2.3 "Components", upstream
 * `panels/component_panel.rs`): Combine as variants for several main components; a component's or
 * set's properties (create, rename, default, delete, variant values, preferred components), Add
 * variant and Description; an instance's swap, actions and property values; a layer inside an
 * instance's overrides; a slot's fill menu; and a layer inside a main component's property binding.
 */
import { useDesignKit, useInspect, useQuery } from '../kit/hooks'
import { Icon, type IconName } from '../icons/Icon'
import { Checkbox, IconButton, Note, Separator, TextField, type MenuEntry } from '../ui'
import { useDesign, useShortcutTip } from './hooks'
import { ActionField, Line, MenuField, Space, menuHeading, useMenu } from './fields'
import type { NodeJson } from './model'

type Prop = { name: string; type: 'variant' | 'boolean' | 'text' | 'instanceSwap' | 'slot'; default?: unknown; options?: string[]; value?: unknown; preferred?: number[] }
type PropsAnswer = { component: number | null; set: number | null; conflicts: unknown[]; properties: Prop[] }
type ComponentEntry = { id: number; name: string; page: number; type: string; folder?: string[] }

const TYPE_ICON: Record<Prop['type'], IconName> = { boolean: 'Eye', text: 'Text', instanceSwap: 'Instance', variant: 'ComponentSet', slot: 'Slot' }

/** Local components and sets; `swappable` leaves out the variants inside sets (swap menus list their sets). */
function useLocalComponents() {
  const all = useQuery<ComponentEntry[]>('components') ?? []
  const sets = new Set(all.filter((c) => c.type === 'COMPONENT_SET').map((c) => c.name))
  const swappable = all.filter((c) => c.type === 'COMPONENT_SET' || !(c.folder?.length && sets.has(c.folder[c.folder.length - 1])))
  return { all, swappable }
}

export function ComponentBlocks() {
  const { answer, nodes } = useDesign()
  if (nodes.length > 1) return <Combine />
  const n = nodes[0]
  if (!n) return null
  const nested = n.insideInstance === true
  // The main component a layer sits in (its properties can control the layer).
  const owner = answer.component?.owningComponent ?? null
  return (
    <>
      {n.type === 'INSTANCE' ? <InstanceBlock node={n} nested={nested} /> : null}
      {n.type === 'COMPONENT' || n.type === 'COMPONENT_SET' ? (
        <>
          <PropertiesBlock node={n} />
          <Description node={n} />
        </>
      ) : null}
      {nested && n.slot ? <SlotBlock node={n} /> : null}
      {n.type !== 'INSTANCE' && nested ? <Overrides node={n} /> : null}
      {n.type !== 'COMPONENT' && n.type !== 'COMPONENT_SET' && !nested && owner !== null && owner !== n.id ? <BindBlock node={n} owner={owner} /> : null}
    </>
  )
}

function Combine() {
  const { strings, run } = useDesignKit()
  const { answer, ids } = useDesign()
  if (answer.canCombineVariants !== true) return null
  return (
    <Line>
      <ActionField label={strings.command('component.combineVariants', 'Combine as variants')} icon="ComponentSet" width={160} onClick={() => void run('component.combineVariants', { ids })} testId="design-design-combine" />
    </Line>
  )
}

/** A block's heading row (36 tall): the title and, at the right, its buttons. */
function Heading({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="design-row design-d-heading" style={{ height: 36 }}>
      <span className="design-d-heading-title">{title}</span>
      <span className="design-d-heading-actions">{children}</span>
    </div>
  )
}

function PropertiesBlock({ node }: { node: NodeJson }) {
  const { t, run } = useDesignKit()
  const answer = useQuery<PropsAnswer>('componentProperties', { id: node.id })
  const { all, swappable } = useLocalComponents()
  const canAddVariant = useDesign().answer.canAddVariant === true
  if (!answer) return null
  const owner = answer.set ?? answer.component ?? node.id
  const isSet = answer.set !== null
  const props = answer.properties
  const names = props.map((p) => p.name)
  const fresh = Array.from({ length: names.length + 1 }, (_, k) => `Property ${k + 1}`).find((name) => !names.includes(name)) ?? 'Property'
  const swapDefault = all.find((c) => c.id !== owner && c.id !== answer.component)?.id ?? null
  const create: MenuEntry[] = [
    { label: t('Variant'), onSelect: () => void run('component.addProperty', { id: owner, name: fresh, type: 'variant', default: 'Default' }), testId: 'design-design-add-property-variant' },
    { label: t('Boolean'), onSelect: () => void run('component.addProperty', { id: owner, name: fresh, type: 'boolean', default: true }), testId: 'design-design-add-property-boolean' },
    { label: t('Text'), onSelect: () => void run('component.addProperty', { id: owner, name: fresh, type: 'text', default: 'Text' }), testId: 'design-design-add-property-text' },
    swapDefault === null
      ? { type: 'custom', render: () => <div className="design-d-menu-note design-tertiary">{t('Instance swap (no other component)')}</div> }
      : { label: t('Instance swap'), onSelect: () => void run('component.addProperty', { id: owner, name: fresh, type: 'instanceSwap', default: swapDefault }) },
  ]
  const edit = (name: string, change: Record<string, unknown>) => void run('component.editProperty', { id: owner, name, ...change })
  const variants = props.filter((p) => p.type === 'variant')
  const shared = props.filter((p) => p.type !== 'variant')
  return (
    <div data-testid="design-design-properties">
      <Separator />
      <Heading title={t('Properties')}>
        <MenuButton icon="Plus" label={t('Create component property')} items={create} menuWidth={180} testId="design-design-add-property" />
      </Heading>
      {!props.length ? <Note>{<span className="design-tertiary">{t('Add properties to make instances configurable')}</span>}</Note> : null}
      {isSet && answer.conflicts.length ? <div className="design-note design-d-warning">{t('Some variants have the same property values: give each one a unique combination')}</div> : null}
      {variants.map((p) => {
        const values = p.options ?? []
        const rows = []
        for (let i = 0; i < values.length; i += 3) rows.push(values.slice(i, i + 3))
        return (
          <div key={p.name}>
            <Line>
              <Icon name="ComponentSet" size={14} className="design-d-component-icon" />
              <TextField value={p.name} width={110} placeholder={t('Name')} onCommit={(newName) => newName && edit(p.name, { newName })} testId={`design-design-prop-${p.name}-name`} />
              <MenuField label={String(p.default ?? values[0] ?? '')} width={70} tip={t('Default value (the top-left variant)')} items={values.map((v) => ({ label: v, onSelect: () => edit(p.name, { default: v }) }))} />
              <IconButton icon="Minus" size={22} label={t('Delete property')} onClick={() => void run('component.deleteProperty', { id: owner, name: p.name })} testId={`design-design-prop-${p.name}-delete`} />
            </Line>
            {rows.map((chunk, r) => (
              <Line key={r} h={30} style={{ paddingLeft: 38 }}>
                {chunk.map((value) => (
                  <TextField key={value} value={value} width={62} onCommit={(newValue) => newValue && newValue !== value && edit(p.name, { value, newValue })} testId={`design-design-prop-${p.name}-value-${value}`} />
                ))}
                {r === rows.length - 1 ? (
                  <IconButton
                    icon="Plus"
                    size={22}
                    label={t('Add variant with a new value')}
                    onClick={() => void run('component.addVariant', { id: owner, property: p.name, value: Array.from({ length: values.length + 2 }, (_, k) => `Value ${k + values.length + 1}`).find((v) => !values.includes(v)) })}
                    testId={`design-design-prop-${p.name}-add-value`}
                  />
                ) : null}
              </Line>
            ))}
          </div>
        )
      })}
      {shared.map((p) => (
        <div key={p.name}>
          <Line>
            <Icon name={TYPE_ICON[p.type]} size={14} className="design-d-component-icon" />
            <TextField value={p.name} width={96} placeholder={t('Name')} onCommit={(newName) => newName && edit(p.name, { newName })} testId={`design-design-prop-${p.name}-name`} />
            <span className="design-d-grow">
              {p.type === 'boolean' ? <Checkbox checked={p.default === true} label={p.default === true ? t('True') : t('False')} onChange={(on) => edit(p.name, { default: on })} testId={`design-design-prop-${p.name}-default`} /> : null}
              {p.type === 'text' ? <TextField value={String(p.default ?? '')} width="100%" placeholder={t('Default')} onCommit={(v) => edit(p.name, { default: v })} testId={`design-design-prop-${p.name}-default`} /> : null}
              {p.type === 'instanceSwap' ? (
                <MenuField
                  label={all.find((c) => c.id === Number(p.default))?.name ?? t('Missing')}
                  width="100%"
                  menuWidth={200}
                  items={swappable.map((c) => ({ label: c.name, checked: c.id === Number(p.default), onSelect: () => edit(p.name, { default: c.id }) }))}
                />
              ) : null}
            </span>
            <IconButton icon="Minus" size={22} label={t('Delete property')} onClick={() => void run('component.deleteProperty', { id: owner, name: p.name })} testId={`design-design-prop-${p.name}-delete`} />
          </Line>
          {p.type === 'instanceSwap' || p.type === 'slot' ? (
            <Line h={30} style={{ paddingLeft: 38 }}>
              <MenuField
                label={(p.preferred ?? []).length ? t('{n} preferred components', { n: (p.preferred ?? []).length }) : t('No preferred components')}
                width="100%"
                menuWidth={200}
                tip={p.type === 'slot' ? t('Offered first when filling the slot (any content is allowed)') : t('Offered first when swapping the instance (any component is allowed)')}
                items={all
                  .filter((c) => c.id !== owner && c.id !== answer.component)
                  .map((c) => {
                    const on = (p.preferred ?? []).includes(c.id)
                    return { label: c.name, checked: on, onSelect: () => edit(p.name, { preferred: on ? (p.preferred ?? []).filter((x) => x !== c.id) : [...(p.preferred ?? []), c.id] }) }
                  })}
              />
            </Line>
          ) : null}
        </div>
      ))}
      {canAddVariant ? (
        <Line>
          <ActionField label={t('Add variant')} icon="Plus" width={120} onClick={() => void run('component.addVariant', { id: owner })} testId="design-design-add-variant" />
        </Line>
      ) : null}
    </div>
  )
}

function Description({ node }: { node: NodeJson }) {
  const { t, run } = useDesignKit()
  return (
    <div data-testid="design-design-description">
      <Separator />
      <div className="design-sublabels">
        <span>{t('Description')}</span>
      </div>
      <div className="design-d-description">
        <textarea
          key={`${node.id}:${node.description ?? ''}`}
          className="design-d-textarea"
          rows={3}
          defaultValue={node.description ?? ''}
          placeholder={t('Describe how and when to use this component')}
          aria-label={t('Description')}
          onKeyDown={(event) => event.stopPropagation()}
          onBlur={(event) => {
            const next = event.currentTarget.value.slice(0, 10000)
            if (next !== (node.description ?? '')) void run('node.set', { id: node.id, props: { description: next } })
          }}
          data-testid="design-design-description-field"
        />
      </div>
      <Space />
    </div>
  )
}

/** An icon button that opens a light menu. */
function MenuButton({ icon, label, items, menuWidth, testId }: { icon: IconName; label: string; items: MenuEntry[]; menuWidth?: number; testId?: string }) {
  return <IconMenu icon={icon} label={label} items={items} menuWidth={menuWidth} testId={testId} />
}


function IconMenu({ icon, label, items, menuWidth, testId }: { icon: IconName; label: string; items: MenuEntry[]; menuWidth?: number; testId?: string }) {
  const menu = useMenu()
  return (
    <span ref={menu.ref} className="design-d-anchor">
      <IconButton icon={icon} label={label} onClick={menu.toggle} testId={testId} />
      {menu.show(items, menuWidth, testId ? `${testId}-menu` : undefined)}
    </span>
  )
}

function InstanceBlock({ node, nested }: { node: NodeJson; nested: boolean }) {
  const { t, run } = useDesignKit()
  const inspect = useInspect()
  const tipFor = useShortcutTip()
  const answer = useQuery<PropsAnswer>('componentProperties', { id: node.id })
  const { all, swappable } = useLocalComponents()
  const id = node.id
  const main = node.mainComponent ?? null
  const missing = main === null || node.mainComponentName == null
  const setId = answer?.set ?? null
  const setEntry = setId !== null ? all.find((c) => c.id === setId) : undefined
  const name = setEntry?.name ?? node.mainComponentName ?? t('Main component deleted')
  const pageName = (page: number) => inspect.pages.find((p) => p.id === page)?.name ?? ''
  const swapItems: MenuEntry[] = swappable.flatMap((c, i): MenuEntry[] => [
    ...(i === 0 || swappable[i - 1].page !== c.page ? [menuHeading(pageName(c.page))] : []),
    { label: c.name, checked: c.id === setId || c.id === main, onSelect: () => (c.id === setId || c.id === main ? undefined : void run('instance.swap', { ids: [id], component: c.id })), testId: `design-design-swap-${c.name}` },
  ])
  const more: MenuEntry[] = [
    ...(!missing ? [{ label: t('Go to main component'), onSelect: () => void run('component.goToMain', { id }) }, { label: t('Push changes to main component'), onSelect: () => void run('component.pushChanges', { id }) }] : []),
    ...(missing && !nested ? [{ label: t('Restore main component'), onSelect: () => void run('component.restoreMain', { id }) }] : []),
    { label: t('Reset all changes'), onSelect: () => void run('component.resetInstance', { ids: [id] }) },
    ...(!nested ? [{ label: t('Detach instance'), shortcut: tipFor('', 'component.detach').trim(), onSelect: () => void run('component.detach', { ids: [id] }) }] : []),
  ]
  const set = (properties: Record<string, unknown>) => void run('instance.setProperties', { ids: [id], properties })
  return (
    <div data-testid="design-design-instance">
      <Separator />
      <Heading title={nested ? t('Nested instance') : t('Instance')}>
        <IconButton icon="Reset" label={t('Reset all changes')} onClick={() => void run('component.resetInstance', { ids: [id] })} testId="design-design-instance-reset" />
        {!nested ? <IconButton icon="Detach" label={tipFor(t('Detach instance'), 'component.detach')} onClick={() => void run('component.detach', { ids: [id] })} testId="design-design-instance-detach" /> : null}
        <IconMenu icon="Dots" label={t('More instance actions')} items={more} menuWidth={220} testId="design-design-instance-more" />
      </Heading>
      <Line>
        <span className="design-d-grow">
          <MenuField label={name} icon="Instance" width="100%" menuWidth={200} tip={t('Swap instance')} items={swapItems} testId="design-design-instance-swap" />
        </span>
        {missing ? (
          <IconButton icon="Component" label={t('Restore main component')} onClick={() => void run('component.restoreMain', { id })} />
        ) : (
          <IconButton icon="Component" label={t('Go to main component')} onClick={() => void run('component.goToMain', { id })} testId="design-design-go-to-main" />
        )}
      </Line>
      {(answer?.properties ?? []).map((p) => (
        <Line key={p.name}>
          <span className="design-d-prop-label" data-tip={p.name}>
            <Icon name={TYPE_ICON[p.type]} size={12} />
            <span>{p.name}</span>
          </span>
          <span className="design-d-grow">
            {p.type === 'variant' ? <MenuField label={String(p.value ?? p.default ?? '')} width="100%" items={(p.options ?? []).map((v) => ({ label: v, onSelect: () => set({ [p.name]: v }), testId: `design-design-instance-${p.name}-${v}` }))} testId={`design-design-instance-${p.name}`} /> : null}
            {p.type === 'boolean' ? <Checkbox checked={(p.value ?? p.default) === true} label="" onChange={(on) => set({ [p.name]: on })} testId={`design-design-instance-${p.name}`} /> : null}
            {p.type === 'text' ? <TextField value={String(p.value ?? p.default ?? '')} width="100%" onCommit={(v) => set({ [p.name]: v })} testId={`design-design-instance-${p.name}`} /> : null}
            {p.type === 'instanceSwap' ? (
              <MenuField
                label={all.find((c) => c.id === Number(p.value ?? p.default))?.name ?? t('None')}
                icon="Instance"
                width="100%"
                menuWidth={200}
                items={[
                  ...((p.preferred ?? []).length ? [menuHeading(t('Preferred')), ...(p.preferred ?? []).map((c) => ({ label: all.find((x) => x.id === c)?.name ?? String(c), onSelect: () => set({ [p.name]: c }) })), menuHeading(t('All components'))] : []),
                  ...all.map((c) => ({ label: c.name, checked: c.id === Number(p.value ?? p.default), onSelect: () => set({ [p.name]: c.id }) })),
                ]}
              />
            ) : null}
          </span>
        </Line>
      ))}
    </div>
  )
}

function SlotBlock({ node }: { node: NodeJson }) {
  const { t, run } = useDesignKit()
  const { all } = useLocalComponents()
  const label = (node.childCount ?? 0) === 0 ? t('Empty') : t('Default content')
  return (
    <div data-testid="design-design-slot">
      <Separator />
      <Heading title={t('Slot')} />
      <Line>
        <span className="design-d-prop-label">
          <Icon name="Slot" size={12} />
          <span>{node.slot}</span>
        </span>
        <span className="design-d-grow">
          <MenuField
            label={label}
            icon="Slot"
            width="100%"
            menuWidth={200}
            tip={t('Add content to this slot')}
            items={[menuHeading(t('All components')), ...all.map((c) => ({ label: c.name, onSelect: () => void run('instance.slotInsert', { id: node.id, component: c.id }) })), { label: t('Select slot'), onSelect: () => void run('select.set', { ids: [node.id] }) }, { label: t('Reset slot'), onSelect: () => void run('instance.resetSlot', { id: node.id }) }]}
          />
        </span>
      </Line>
    </div>
  )
}

const OVERRIDE_LABELS: Record<string, string> = {
  fills: 'Fill',
  strokes: 'Stroke',
  effects: 'Effects',
  opacity: 'Opacity & blend',
  visible: 'Visibility',
  name: 'Layer name',
  corners: 'Corner radius',
  characters: 'Text content',
  textStyle: 'Text style',
  padding: 'Padding',
  gap: 'Gap',
  alignment: 'Alignment',
  grids: 'Layout grids',
  exports: 'Export settings',
  interactions: 'Interactions',
  modes: 'Variable modes',
  size: 'Size',
  resizing: 'Resizing',
  position: 'Position',
  component: 'Swapped instance',
  props: 'Instance properties',
  slot: 'Slot content',
}

function Overrides({ node }: { node: NodeJson }) {
  const { t, run } = useDesignKit()
  const groups = (node.overrides ?? []).filter((g) => g in OVERRIDE_LABELS)
  if (!groups.length) return null
  return (
    <div data-testid="design-design-overrides">
      <Separator />
      <Heading title={t('Overrides')}>
        <IconButton icon="Reset" label={t('Reset all overrides on this layer')} onClick={() => void run('component.resetOverride', { id: node.id })} testId="design-design-overrides-reset" />
      </Heading>
      {groups.map((g) => (
        <Line key={g} h={26}>
          <span className="design-d-grow design-secondary">{t(OVERRIDE_LABELS[g])}</span>
          <IconButton icon="Close" size={20} label={t('Reset this override')} onClick={() => void run('component.resetOverride', { id: node.id, group: g })} testId={`design-design-override-${g}`} />
        </Line>
      ))}
    </div>
  )
}

/** Binding a layer inside main component `owner` to one of its properties (`propRefs` says which one controls each field). */
function BindBlock({ node: n, owner }: { node: NodeJson; owner: number }) {
  const { t, run } = useDesignKit()
  const answer = useQuery<PropsAnswer>('componentProperties', { id: owner })
  const fields: Array<[string, string, string, IconName]> = [['visible', 'Visibility', 'boolean', 'Eye']]
  if (n.type === 'TEXT') fields.push(['characters', 'Text', 'text', 'Text'])
  if (n.type === 'INSTANCE') fields.push(['mainComponent', 'Instance', 'instanceSwap', 'Instance'])
  if (n.type === 'FRAME') fields.push(['slot', 'Slot', 'slot', 'Slot'])
  const defs = answer?.properties ?? []
  return (
    <div data-testid="design-design-bind">
      <Separator />
      <Heading title={t('Component property')} />
      {fields.map(([field, label, type, icon]) => {
        const matching = defs.filter((d) => d.type === type)
        const bound = n.propRefs?.[field] ?? (field === 'slot' && n.slot ? n.slot : null)
        const name = Array.from({ length: defs.length + 2 }, (_, k) => (k === 0 ? label : `${label} ${k + 1}`)).find((x) => !defs.some((d) => d.name === x)) ?? label
        const items: MenuEntry[] = [
          ...matching.map((d) => ({ label: d.name, onSelect: () => void run('component.bindProperty', { layer: n.id, property: d.name, field }) })),
          ...(type === 'slot'
            ? bound
              ? [{ label: t('Remove slot'), onSelect: () => void run('component.removeSlot', { id: n.id }) }]
              : [{ label: t('Make slot'), onSelect: () => void run('component.makeSlot', { id: n.id }) }]
            : [{ label: t('Create property…'), onSelect: () => void run('component.addProperty', { id: n.id, name, type }) }, { label: t('Detach property'), onSelect: () => void run('component.bindProperty', { layer: n.id, property: null, field }) }]),
        ]
        return (
          <Line key={field}>
            <Icon name={icon} size={14} className={bound ? 'design-d-component-icon' : 'design-d-icon-secondary'} />
            {/* egui lays the label and the dropdown out left to right, 8 apart. */}
            <span className="design-secondary">{t(label)}</span>
            <MenuField label={bound ?? t('Not bound')} width={120} menuWidth={180} tip={t('Apply a component property')} items={items} testId={`design-design-bind-${field}`} />
          </Line>
        )
      })}
    </div>
  )
}

/**
 * The floating bottom toolbar (2.1 §4): six tool groups with their flyouts (dark in both themes),
 * Resources and Actions, and the mode switcher (Draw, Design, Prototype, Inspect); or, instead of
 * it, the view-only, vector edit and shape builder pills.
 */
import { useMemo, useRef, useState } from 'react'
import { isMac } from '@kition/sdk/editor'
import { Icon, type IconName } from '../icons/Icon'
import { useDesignKit, useQuery, useTools } from '../kit/hooks'
import { regionProps } from '../kit/regions'
import type { RightTab } from '../kit/types'
import { Menu } from '../ui/Menu'
import { prettyShortcut, tip } from '../ui/shortcuts'
import { ActionsPalette } from '../menus/ActionsPalette'
import { ALL_TOOLS, TOOL_GROUPS, VECTOR_TOOLS, type ToolDef, type ToolGroup } from './tools'

/** The main pill's width (2.1 §4.1): groups, carets, side buttons, divider and mode switcher. */
const PILL_WIDTH = 554

export function Toolbar() {
  const { inspect, readOnly, t } = useDesignKit()
  const activeTool = useTools((tools) => tools.active)
  const pill = useRef<HTMLDivElement>(null)
  let content
  if (readOnly) content = <ViewOnlyPill />
  else if (inspect.vectorEdit) content = <VectorEditPill tool={inspect.vectorEdit.tool} />
  else if (activeTool === 'shapeBuilder') content = <ShapeBuilderPill />
  else content = <MainPill pill={pill} />
  const main = !readOnly && !inspect.vectorEdit && activeTool !== 'shapeBuilder'
  return (
    <div ref={pill} className="design-pill" style={main ? { width: PILL_WIDTH } : { paddingRight: 4 }} {...regionProps('toolbar', t('region.toolbar'))} data-testid="design-toolbar" onPointerDown={(event) => event.stopPropagation()}>
      {content}
    </div>
  )
}

function useToolInfo() {
  const list = useTools((tools) => tools.tools)
  // The engine's own labels and shortcuts win over the static table (they follow its keymap).
  return useMemo(() => {
    const byId = new Map(list.map((tool) => [tool.id, tool]))
    return (def: ToolDef): ToolDef => {
      const engine = byId.get(def.id)
      return engine ? { ...def, label: engine.label || def.label, shortcut: engine.shortcut ?? def.shortcut } : def
    }
  }, [list])
}

function MainPill({ pill }: { pill: React.RefObject<HTMLDivElement | null> }) {
  const { tools, ui, t } = useDesignKit()
  const info = useToolInfo()
  const [last, setLast] = useState<Record<string, string>>({})
  const [flyout, setFlyout] = useState<string | null>(null)
  const buttons = useRef<Record<string, HTMLButtonElement | null>>({})
  const active = useTools((current) => current.active)

  const choose = (id: string) => {
    if (ui.rightTab === 'inspect' && !['move', 'hand', 'comment'].includes(id)) ui.setRightTab('design')
    tools.select(id)
  }

  const shownTool = (group: ToolGroup) => group.tools.find((tool) => tool.id === active) ?? group.tools.find((tool) => tool.id === last[group.key]) ?? group.tools[0]

  const openGroup = flyout ? TOOL_GROUPS.find((group) => group.key === flyout) : null
  const anchor = openGroup ? buttons.current[openGroup.key] : null
  const pillRect = pill.current?.getBoundingClientRect()
  const anchorRect = anchor && pillRect ? { left: anchor.getBoundingClientRect().left, top: pillRect.top, right: anchor.getBoundingClientRect().left + 220, bottom: pillRect.top } : null

  const mode = ui.rightTab === 'prototype' ? 2 : ui.rightTab === 'inspect' ? 3 : active === 'pencil' ? 0 : 1
  const setMode = (next: number) => {
    const tab: RightTab = next === 2 ? 'prototype' : next === 3 ? 'inspect' : 'design'
    if (next === 0) {
      ui.setRightTab('design')
      tools.select('pencil')
      return
    }
    if (next === 1 && active === 'pencil') tools.select('move')
    // Leaving Design ends the Pen tool (`end_pen_outside_design`).
    if (tab !== 'design' && active === 'pen') tools.select('move')
    ui.setRightTab(tab)
  }
  const modes: Array<{ label: string; icon: IconName }> = [
    { label: 'Draw', icon: 'Pencil' },
    { label: 'Design', icon: 'Frame' },
    { label: 'Prototype', icon: 'Prototype' },
    { label: 'Inspect', icon: 'Code' },
  ]

  return (
    <>
      {TOOL_GROUPS.map((group) => {
        const tool = info(shownTool(group))
        const pressed = group.tools.some((item) => item.id === active)
        return (
          <span key={group.key} style={{ display: 'contents' }}>
            <button
              ref={(el) => void (buttons.current[group.key] = el)}
              type="button"
              className="design-tool"
              aria-pressed={pressed}
              aria-label={t(tool.label)}
              data-tip={tip(t(tool.label), tool.shortcut)}
              data-design-prefer={pressed || undefined}
              data-testid={`design-tool-${group.key}`}
              data-tool={tool.id}
              onClick={() => choose(tool.id)}
            >
              <Icon name={tool.icon} size={18} />
            </button>
            {group.tools.length > 1 ? (
              <button
                type="button"
                className="design-caret"
                aria-haspopup="menu"
                aria-expanded={flyout === group.key}
                aria-label={t(group.flyout)}
                data-testid={`design-caret-${group.key}`}
                onClick={() => setFlyout((now) => (now === group.key ? null : group.key))}
              >
                <Icon name="Chevron" size={14} />
              </button>
            ) : null}
          </span>
        )
      })}
      <button type="button" className="design-tool" aria-label={t('Resources')} data-tip={tip(t('Resources'), 'Shift+I')} data-testid="design-resources" onClick={() => {
        ui.setLeftTab('assets')
        ui.setLeftOpen(true)
      }}>
        <Icon name="Assets" size={18} />
      </button>
      <button type="button" className="design-tool" aria-label={t('Actions')} aria-pressed={ui.actions} data-tip={tip(t('Actions'), 'Cmd+K')} data-testid="design-actions" onClick={() => ui.setActions(!ui.actions)}>
        <Icon name="Actions" size={18} />
      </button>
      <span className="design-pill-divider" />
      <div className="design-modes" role="group" aria-label={t('Mode')}>
        {modes.map((item, index) => (
          <button key={item.label} type="button" className="design-mode" aria-pressed={mode === index} aria-label={t(item.label)} data-tip={t(item.label)} data-testid={`design-mode-${item.label.toLowerCase()}`} onClick={() => setMode(index)}>
            <Icon name={item.icon} size={18} />
          </button>
        ))}
      </div>
      {openGroup && anchorRect ? (
        <Menu
          tone="dark"
          variant="flyout"
          anchor={anchorRect}
          placement="above-start"
          gap={8}
          minWidth={220}
          label={t(openGroup.flyout)}
          testId={`design-flyout-${openGroup.key}`}
          onClose={() => setFlyout(null)}
          items={openGroup.tools.map((def) => {
            const tool = info(def)
            return {
              label: t(tool.label),
              icon: tool.icon,
              checked: tool.id === active,
              shortcut: prettyShortcut(tool.shortcut),
              testId: `design-flyout-tool-${tool.id}`,
              onSelect: () => {
                setLast((now) => ({ ...now, [openGroup.key]: tool.id }))
                choose(tool.id)
              },
            }
          })}
        />
      ) : null}
      {ui.actions ? <ActionsPalette anchor={pill.current} onClose={() => ui.setActions(false)} /> : null}
    </>
  )
}

function ViewOnlyPill() {
  const { tools, t } = useDesignKit()
  const active = useTools((current) => current.active)
  const info = useToolInfo()
  const shown = ['move', 'hand'].map((id) => info(ALL_TOOLS.find((tool) => tool.id === id)!))
  return (
    <>
      {shown.map((tool) => (
        <button key={tool.id} type="button" className="design-tool" aria-pressed={active === tool.id} aria-label={t(tool.label)} data-tip={tip(t(tool.label), tool.shortcut)} onClick={() => tools.select(tool.id)} data-testid={`design-tool-${tool.id}`}>
          <Icon name={tool.icon} size={18} />
        </button>
      ))}
      <span className="design-pill-divider" />
      <span className="design-pill-badge" data-tip={t('view.onlyTip')} data-testid="design-view-only">
        <Icon name="Eye" size={14} />
        {t('View only')}
      </span>
    </>
  )
}

function VectorEditPill({ tool }: { tool: string }) {
  const { run, t } = useDesignKit()
  const answer = useQuery<{ tools?: Array<{ id: string; label: string; shortcut: string | null }> }>('vectorEdit')
  const shown = VECTOR_TOOLS.map((def) => {
    const engine = answer?.tools?.find((item) => item.id === def.id)
    return engine ? { ...def, label: engine.label, shortcut: engine.shortcut } : def
  })
  return (
    <>
      {shown.map((def) => (
        <button key={def.id} type="button" className="design-tool" aria-pressed={tool === def.id} aria-label={t(def.label)} data-tip={def.id === 'bend' ? `${t('Bend')}  hold ${isMac() ? '⌘' : 'Ctrl'}` : tip(t(def.label), def.shortcut)} onClick={() => void run('vector.edit.tool', { tool: def.id })} data-testid={`design-vector-tool-${def.id}`}>
          <Icon name={def.icon} size={18} />
        </button>
      ))}
      <span className="design-pill-divider" style={{ marginRight: 9 }} />
      <button type="button" className="design-done" data-tip={tip(t('Done editing'), 'Escape')} onClick={() => void run('vector.edit.end', {})} data-testid="design-vector-done">
        {t('Done')}
      </button>
    </>
  )
}

function ShapeBuilderPill() {
  const { run, t, tools } = useDesignKit()
  const [remove, setRemove] = useState(false)
  // Upstream asks for overlapping shapes until the selection makes regions.
  const regions = useQuery<{ regions: unknown[] }>('shapeBuilderRegions')
  const empty = !regions?.regions?.length
  const set = (next: boolean) => {
    setRemove(next)
    void run('shapeBuilder.mode', { mode: next ? 'remove' : 'merge' })
  }
  return (
    <>
      <button type="button" className="design-tool" aria-pressed={!remove} aria-label={t('Merge regions')} data-tip={t('Merge regions')} onClick={() => set(false)}>
        <Icon name="ShapeBuilder" size={18} />
      </button>
      <button type="button" className="design-tool" aria-pressed={remove} aria-label={t('Remove regions')} data-tip={t('Remove regions')} onClick={() => set(true)}>
        <Icon name="ShapeBuilderRemove" size={18} />
      </button>
      <span className="design-pill-divider" style={{ marginRight: 9 }} />
      <span className="design-pill-hint">{t(empty ? 'Select overlapping shapes' : remove ? 'Click or drag to remove · ⌥ merges' : 'Click or drag to merge · ⌥ removes')}</span>
      <span className="design-pill-divider" style={{ marginRight: 9 }} />
      <button type="button" className="design-done" data-tip={tip(t('Done'), 'Enter')} onClick={() => tools.select('move')} data-testid="design-builder-done">
        {t('Done')}
      </button>
    </>
  )
}

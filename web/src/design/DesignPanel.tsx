/**
 * The Design tab (2.3), owned by the Design panel package: the section router per selection kind,
 * as upstream's `right_panel` and `design` (`panels/right.rs`). Nothing selected shows the page;
 * the Frame tool shows the frame presets; a selection shows its header and component blocks, then
 * Status, Position, Layout, Arc or Polygon/Star, Appearance, Variable modes, the paints package's
 * Typography, Fill, Stroke and Effects, Selection colors, Layout grid and Export.
 */
import { useMemo } from 'react'
import { useDesignKit, useInspect, useQuery } from '../kit/hooks'
import { Separator } from '../ui'
import { PaintSections } from '../paint/PaintSections'
import { AppearanceSection } from './Appearance'
import { ComponentBlocks } from './Components'
import { ExportSection } from './Export'
import { FramePresets } from './FramePresets'
import { SelectionHeader } from './Header'
import { DesignContext, type Design } from './hooks'
import { LayoutSection } from './Layout'
import { LayoutGridSection } from './LayoutGrid'
import type { SelectionAnswer } from './model'
import { PageSection } from './PageSection'
import { PositionSection } from './Position'
import { SelectionColorsSection } from './SelectionColors'
import { ArcSection, PointsSection } from './Shapes'
import { StatusSection } from './Status'
import { VariableModesSection } from './VariableModes'
import './design.css'

export function DesignPanel() {
  const { run } = useDesignKit()
  const inspect = useInspect()
  const answer = useQuery<SelectionAnswer>('selection')
  const design = useMemo<Design | null>(() => {
    if (!answer || answer.count === 0) return null
    return {
      answer,
      nodes: answer.nodes,
      ids: answer.ids,
      setProps: (props, ids = answer.ids) => {
        if (ids.length) void run('node.set', { ids, props })
      },
    }
  }, [answer, run])

  if (inspect.tool === 'frame') {
    return (
      <div className="design-design">
        <FramePresets />
      </div>
    )
  }
  if (inspect.selection.length === 0 || !design) {
    return (
      <div className="design-design">
        <PageSection answer={answer && answer.count === 0 ? answer : null} />
      </div>
    )
  }
  const anyText = design.nodes.some((n) => n.type === 'TEXT')
  return (
    <DesignContext.Provider value={design}>
      <div className="design-design" data-testid="design-design-panel" data-selection={design.answer.kind ?? 'Mixed'}>
        <SelectionHeader />
        <ComponentBlocks />
        <Separator />
        <StatusSection />
        <PositionSection />
        <Separator />
        <LayoutSection />
        <Separator />
        <ArcSection />
        <PointsSection />
        <AppearanceSection />
        <Separator />
        <VariableModesSection />
        <PaintSections text={anyText} />
        <SelectionColorsSection />
        {design.nodes.every((n) => ['FRAME', 'COMPONENT', 'COMPONENT_SET', 'INSTANCE'].includes(n.type)) ? (
          <>
            <LayoutGridSection />
            <Separator />
          </>
        ) : null}
        <ExportSection />
      </div>
    </DesignContext.Provider>
  )
}

/**
 * The Frame tool's panel (upstream `panels/frame_presets.rs`): frame presets by category (Phone
 * open at first). Choosing one runs `frame.create`, returns to the Move tool and shows the frame.
 */
import { useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { SectionHeader } from '../ui'

type Preset = { category: string; name: string; width: number; height: number }

/** Which categories are open (egui temp data upstream: one set for the panel). */
const opened = new Map<string, boolean>()

export function FramePresets() {
  const { t, run, viewTo } = useDesignKit()
  const presets = useQuery<Preset[]>('framePresets') ?? []
  const [, redraw] = useState(0)
  const categories = [...new Set(presets.map((p) => p.category))]
  const choose = async (preset: Preset) => {
    await run('frame.create', { preset: preset.name })
    await run('tool.set', { tool: 'move' })
    await viewTo('view.zoomSelection')
  }
  return (
    <div data-testid="design-design-panel" data-selection="frame-tool">
      <SectionHeader title={t('Frame')} />
      {categories.map((category) => {
        const open = opened.get(category) ?? category === 'Phone'
        return (
          <div key={category}>
            <button
              type="button"
              className="design-d-preset-cat"
              aria-expanded={open}
              onClick={() => {
                opened.set(category, !open)
                redraw((v) => v + 1)
              }}
              data-testid={`design-design-preset-cat-${category}`}
            >
              <Icon name={open ? 'Chevron' : 'ChevronRight'} size={10} />
              <span>{t(category)}</span>
            </button>
            {open
              ? presets
                  .filter((p) => p.category === category)
                  .map((p) => (
                    <button key={p.name} type="button" className="design-d-preset" onClick={() => void choose(p)} data-testid={`design-design-frame-preset-${p.name}`}>
                      <span>{p.name}</span>
                      <span className="design-secondary">{`${p.width}×${p.height}`}</span>
                    </button>
                  ))
              : null}
          </div>
        )
      })}
    </div>
  )
}

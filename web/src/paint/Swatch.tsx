/**
 * A paint swatch (`paint_swatch`): a white and #E6E6E6 checker (cells a quarter of the side) under
 * the paint: a solid colour opaque on the left half and with its alpha on the right (all opaque
 * when it has none), a gradient in 16 sampled strips, an image or pattern as a grey tile with its
 * icon; a 1 px `border` edge, radius 2.
 */
import { Icon } from '../icons/Icon'
import { type Color, cssColor } from './color'
import { type Paint, stripes } from './paint'

const checker = (size: number) => {
  const cell = size / 4
  return {
    backgroundColor: '#ffffff',
    backgroundImage: 'linear-gradient(45deg, #e6e6e6 25%, transparent 25%, transparent 75%, #e6e6e6 75%), linear-gradient(45deg, #e6e6e6 25%, transparent 25%, transparent 75%, #e6e6e6 75%)',
    backgroundSize: `${cell * 2}px ${cell * 2}px`,
    backgroundPosition: `0 0, ${cell}px ${cell}px`,
  }
}

export function PaintSwatch({ paint, color, size = 16, className = '' }: { paint?: Paint; color?: Color; size?: number; className?: string }) {
  const p = paint ?? null
  let layer: React.ReactNode = null
  if (color || p?.type === 'solid') {
    const c = color ?? p!.color
    const alpha = color ? color.a : c.a * p!.opacity
    const opaque = cssColor(c, 1)
    layer =
      alpha >= 0.999 ? (
        <span className="design-swatch-fill" style={{ background: opaque }} />
      ) : (
        <span className="design-swatch-fill" style={{ background: `linear-gradient(to right, ${opaque} 50%, ${cssColor(c, alpha)} 50%)` }} />
      )
  } else if (p?.type === 'gradient' && p.gradient) {
    layer = <span className="design-swatch-fill" style={{ background: stripes(p.gradient, 16) }} />
  } else if (p) {
    layer = (
      <span className="design-swatch-fill design-swatch-tile">
        <Icon name={p.type === 'image' ? 'Image' : 'ViewGrid'} size={Math.max(8, size - 4)} />
      </span>
    )
  }
  return (
    <span className={`design-swatch ${className}`} style={{ width: size, height: size, ...checker(size) }}>
      {layer}
    </span>
  )
}

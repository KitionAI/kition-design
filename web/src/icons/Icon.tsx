import { useId, type CSSProperties } from 'react';

import { ICON_SVG_ATTRS, iconMarkup, iconMetrics, type IconName } from './icons';

export type { IconName } from './icons';

export interface IconProps {
  name: IconName;
  /** The box, in CSS pixels (the upstream's points). */
  size?: number;
  className?: string;
  /** An accessible name; without one the icon is decorative (`aria-hidden`). */
  title?: string;
  style?: CSSProperties;
}

/**
 * One of the editor's icons, drawn as the upstream draws it: the 16-unit glyph scaled by whole
 * steps (quarter steps on fractional display scales) and centred in the box, with strokes a whole
 * number of device pixels wide. It paints in `currentColor`.
 */
export function Icon({ name, size = 16, className, title, style }: IconProps) {
  const uid = useId().replace(/[^A-Za-z0-9_-]/g, '');
  const ratio = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1;
  const m = iconMetrics(size, ratio);
  let markup = iconMarkup(name, m, `design-icon-${uid}`);
  if (title) markup = `<title>${escapeText(title)}</title>${markup}`;
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox={m.viewBox}
      {...ICON_SVG_ATTRS}
      strokeWidth={m.stroke}
      className={className}
      style={{ ...style, ['--design-icon-px' as string]: m.px }}
      role={title ? 'img' : undefined}
      aria-label={title || undefined}
      aria-hidden={title ? undefined : true}
      focusable="false"
      dangerouslySetInnerHTML={{ __html: markup }}
    />
  );
}

function escapeText(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

import { useEffect, useRef, useState, type RefObject } from 'react';

/**
 * Upstream's floating scroll bar (egui `ScrollStyle::floating`) over a scroll area whose native
 * bar is hidden (`design-scroll-hidden`): nothing until the pointer is over the area, then a 2 px
 * handle in `text` at 60 % on the area's right edge (its track in `field` at 40 %); over the bar it
 * widens to 10 px at full strength. The handle is `scroll / content` of the area's height (at least
 * 12 px). Drag the handle to scroll; a press on the track jumps there. Render it next to the area,
 * inside a positioned parent.
 */
export function ScrollBar({ target, testId }: { target: RefObject<HTMLElement | null>; testId?: string }) {
  const [box, setBox] = useState<{ top: number; height: number; right: number; start: number; size: number } | null>(null);
  const [area, setArea] = useState(false);
  const [bar, setBar] = useState(false);
  const [drag, setDrag] = useState(false);
  const grab = useRef<{ y: number; scroll: number } | null>(null);

  useEffect(() => {
    const el = target.current;
    if (!el) return;
    const measure = () => {
      const content = el.scrollHeight;
      const view = el.clientHeight;
      if (content <= view + 0.5 || view <= 0) {
        setBox(null);
        return;
      }
      const parent = el.offsetParent as HTMLElement | null;
      const right = parent ? parent.clientWidth - (el.offsetLeft + el.offsetWidth) : 0;
      const size = Math.max(12, (view / content) * view);
      const max = content - view;
      const start = max > 0 ? (el.scrollTop / max) * (view - size) : 0;
      setBox({ top: el.offsetTop, height: view, right, start, size });
    };
    measure();
    const enter = () => setArea(true);
    const leave = () => setArea(false);
    el.addEventListener('scroll', measure, { passive: true });
    el.addEventListener('pointerenter', enter);
    el.addEventListener('pointerleave', leave);
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    for (const child of Array.from(el.children)) observer.observe(child);
    const mutations = new MutationObserver(() => {
      for (const child of Array.from(el.children)) observer.observe(child);
      measure();
    });
    mutations.observe(el, { childList: true });
    return () => {
      el.removeEventListener('scroll', measure);
      el.removeEventListener('pointerenter', enter);
      el.removeEventListener('pointerleave', leave);
      observer.disconnect();
      mutations.disconnect();
    };
  }, [target]);

  if (!box) return null;
  const wide = bar || drag;
  const shown = area || wide;
  const scrollTo = (handleTop: number) => {
    const el = target.current;
    if (!el) return;
    const room = box.height - box.size;
    el.scrollTop = room > 0 ? (Math.min(room, Math.max(0, handleTop)) / room) * (el.scrollHeight - el.clientHeight) : 0;
  };
  return (
    <div
      className="design-scrollbar"
      data-shown={shown || undefined}
      data-wide={wide || undefined}
      data-testid={testId}
      aria-hidden
      style={{ top: box.top, height: box.height, right: box.right }}
      onPointerEnter={() => setBar(true)}
      onPointerLeave={() => setBar(false)}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        const y = event.clientY - event.currentTarget.getBoundingClientRect().top;
        const onHandle = y >= box.start && y <= box.start + box.size;
        if (!onHandle) scrollTo(y - box.size / 2);
        grab.current = { y: event.clientY, scroll: onHandle ? box.start : y - box.size / 2 };
        setDrag(true);
      }}
      onPointerMove={(event) => {
        if (!grab.current) return;
        scrollTo(grab.current.scroll + event.clientY - grab.current.y);
      }}
      onPointerUp={() => {
        grab.current = null;
        setDrag(false);
      }}
      onPointerCancel={() => {
        grab.current = null;
        setDrag(false);
      }}
    >
      <span className="design-scrollbar-handle" style={{ top: box.start, height: box.size }} />
    </div>
  );
}

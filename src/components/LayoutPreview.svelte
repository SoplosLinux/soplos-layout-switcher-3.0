<script lang="ts">
  import type { LayoutPreviewShape, Side } from "../lib/layouts";

  // The picture of a layout the user saved, drawn in the style of the four
  // pictures of the program (public/assets/layouts/*preview.svg): the same
  // canvas, the same orange lines and the same thickness. The top bar, the
  // panel and the dock go on their sides first; the windows fill what is left,
  // as tiles when the layout tiles them and as one window when it does not.

  export let shape: LayoutPreviewShape;

  const W = 52.917;
  const H = 39.688;
  const COLOR = "#FF8800";
  const BAR = 2.41; // the top bar of GNOME, as in the four pictures
  const PANEL = 3.2; // Dash to Panel
  const DOCK = 4.6; // Dash to Dock
  const ICON = 2.07; // an icon of the dock or the panel
  const EDGE = 0.26;

  type Box = { x: number; y: number; w: number; h: number };
  type Line = { x1: number; y1: number; x2: number; y2: number; width: number };
  type Rect = Box & { r: number; width: number; fill: boolean };

  const horizontal = (side: Side) => side === "top" || side === "bottom";

  function draw(shape: LayoutPreviewShape) {
    const lines: Line[] = [];
    const rects: Rect[] = [];
    const circles: { cx: number; cy: number; r: number }[] = [];
    // What is left for the windows, shrunk as each part takes its side.
    const area = { x0: EDGE + 0.5, y0: EDGE + 0.5, x1: W - EDGE - 0.5, y1: H - EDGE - 0.5 };
    const take = (side: Side, size: number) => {
      if (side === "top") area.y0 += size;
      if (side === "bottom") area.y1 -= size;
      if (side === "left") area.x0 += size;
      if (side === "right") area.x1 -= size;
    };

    // A row of icons along a side, from `from` to `to`, in the middle of a band.
    const icons = (side: Side, from: number, to: number, middle: number, count: number) => {
      const step = (to - from) / count;
      for (let i = 0; i < count; i++) {
        const along = from + step * i + (step - ICON) / 2;
        const x = horizontal(side) ? along : middle - ICON / 2;
        const y = horizontal(side) ? middle - ICON / 2 : along;
        rects.push({ x, y, w: ICON, h: ICON, r: 0, width: 0, fill: true });
      }
    };

    if (!shape.panel) {
      // The top bar of GNOME: the activities, the clock and the menus of the
      // system, or three islands with Open Bar.
      if (shape.islands) {
        for (const [x, w] of [
          [1, 6],
          [22.5, 8],
          [42.9, 9],
        ]) {
          rects.push({ x, y: 0.55, w, h: 1.55, r: 0.75, width: 0.4, fill: false });
        }
      } else {
        lines.push({ x1: 0, y1: BAR, x2: W, y2: BAR, width: 0.53 });
        lines.push({ x1: 25.06, y1: 1.32, x2: 27.82, y2: 1.32, width: 0.66 });
        lines.push({ x1: 43.86, y1: 1.32, x2: 51.87, y2: 1.32, width: 0.66 });
      }
      if (shape.menu) circles.push({ cx: 2.6, cy: 1.32, r: 0.7 });
      else if (!shape.islands) lines.push({ x1: 1.07, y1: 1.32, x2: 4.77, y2: 1.32, width: 0.66 });
      take("top", BAR);
    } else {
      // Dash to Panel: one band with the menu, the open programs and the clock.
      const side = shape.panel;
      const at = side === "top" ? PANEL : side === "bottom" ? H - PANEL : side === "left" ? PANEL : W - PANEL;
      if (horizontal(side)) lines.push({ x1: 0, y1: at, x2: W, y2: at, width: 0.53 });
      else lines.push({ x1: at, y1: 0, x2: at, y2: H, width: 0.53 });
      const middle = side === "top" || side === "left" ? PANEL / 2 : (horizontal(side) ? H : W) - PANEL / 2;
      const length = horizontal(side) ? W : H;
      if (shape.menu) {
        circles.push(horizontal(side) ? { cx: 2.2, cy: middle, r: 0.8 } : { cx: middle, cy: 2.2, r: 0.8 });
      }
      icons(side, 4, horizontal(side) ? 22 : 18, middle, horizontal(side) ? 5 : 4);
      if (horizontal(side)) lines.push({ x1: length - 6, y1: middle, x2: length - 2, y2: middle, width: 0.66 });
      else lines.push({ x1: middle, y1: length - 5, x2: middle, y2: length - 2, width: 0.66 });
      take(side, PANEL);
    }

    if (shape.dock) {
      // Dash to Dock, in the middle of its side or along all of it.
      const side = shape.dock;
      const span = horizontal(side) ? area.x1 - area.x0 : area.y1 - area.y0;
      const length = shape.dockFull ? span : Math.min(18, span);
      const start = (horizontal(side) ? area.x0 : area.y0) + (span - length) / 2;
      const across = side === "top" ? area.y0 : side === "bottom" ? area.y1 - DOCK : side === "left" ? area.x0 : area.x1 - DOCK;
      const box = horizontal(side) ? { x: start, y: across, w: length, h: DOCK } : { x: across, y: start, w: DOCK, h: length };
      rects.push({ ...box, r: 0.9, width: 0.53, fill: false });
      const middle = across + DOCK / 2;
      icons(side, start + 0.6, start + length - 0.6, middle, Math.max(3, Math.floor((length - 1.2) / 3.2)));
      take(side, DOCK + 0.6);
    }

    if (shape.desktopIcons) {
      // The icons of the desktop, in a column on the left.
      for (let i = 0; i < 3; i++) {
        rects.push({ x: area.x0 + 0.6, y: area.y0 + 0.8 + i * 3, w: 1.9, h: 1.9, r: 0.3, width: 0.26, fill: false });
      }
      area.x0 += 3.4;
    }

    // The windows, each a box with its title bar.
    const windows: Box[] = [];
    const gap = 0.9;
    const width = area.x1 - area.x0;
    const height = area.y1 - area.y0;
    if (shape.tiling) {
      const half = (width - gap) / 2;
      const third = (height - gap) / 2;
      windows.push({ x: area.x0, y: area.y0 + 0.4, w: half, h: height - 0.4 });
      windows.push({ x: area.x0 + half + gap, y: area.y0 + 0.4, w: half, h: third - 0.4 });
      windows.push({ x: area.x0 + half + gap, y: area.y0 + third + gap, w: half, h: third });
    } else {
      const w = width * 0.62;
      const h = height * 0.66;
      windows.push({ x: area.x0 + (width - w) / 2, y: area.y0 + (height - h) / 2, w, h });
    }
    for (const box of windows) {
      rects.push({ ...box, r: 1.19, width: 0.26, fill: false });
      lines.push({ x1: box.x + 0.15, y1: box.y + 1.2, x2: box.x + box.w - 0.15, y2: box.y + 1.2, width: 0.4 });
    }

    return { lines, rects, circles };
  }

  $: parts = draw(shape);
</script>

<svg viewBox="0 0 {W} {H}" class="w-full" role="img" aria-hidden="true">
  <rect x={EDGE} y={EDGE} width={W - EDGE * 2} height={H - EDGE * 2} fill="none" stroke={COLOR} stroke-width="0.53" />
  {#each parts.rects as box}
    <rect x={box.x} y={box.y} width={box.w} height={box.h} rx={box.r} fill={box.fill ? COLOR : "none"} stroke={box.fill ? "none" : COLOR} stroke-width={box.width} />
  {/each}
  {#each parts.lines as line}
    <line x1={line.x1} y1={line.y1} x2={line.x2} y2={line.y2} stroke={COLOR} stroke-width={line.width} />
  {/each}
  {#each parts.circles as circle}
    <circle cx={circle.cx} cy={circle.cy} r={circle.r} fill={COLOR} />
  {/each}
</svg>

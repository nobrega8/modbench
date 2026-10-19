// Windowing of long tables: only the rows in (or near) view are rendered, and
// two spacer rows stand in for the others, so the scroll bar stays true.

/** Tables shorter than this are rendered whole. */
export const VIRTUAL_FROM = 300;

export interface Window {
  /** First row to render, and the one after the last. */
  start: number;
  end: number;
  /** Height of the rows left out before and after the window, in px. */
  before: number;
  after: number;
}

/**
 * The rows to render for a scroll position, with `overscan` rows of margin
 * on both sides. Rows are `rowHeight` px tall.
 */
export function windowOf(
  count: number,
  scrollTop: number,
  viewport: number,
  rowHeight: number,
  overscan = 20,
): Window {
  if (count < VIRTUAL_FROM || rowHeight <= 0) {
    return { start: 0, end: count, before: 0, after: 0 };
  }
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const visible = Math.ceil(Math.max(0, viewport) / rowHeight);
  const start = Math.min(Math.max(0, first - overscan), count);
  const end = Math.min(count, first + visible + overscan);
  return {
    start,
    end,
    before: start * rowHeight,
    after: (count - end) * rowHeight,
  };
}

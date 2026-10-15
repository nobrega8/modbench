<!--
  Export image… for the Chart tab and the Trend player: saves the charts
  shown, with their legends, units and time span, as PNG or SVG, in the
  current theme or always light (see chart-image.ts).
-->
<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import {
    FONT,
    MONO,
    composeSvg,
    plotMarkup,
    spanText,
    type ChartPart,
    type Palette,
  } from "#lib/chart-image.ts";
  import { contextMenu } from "#lib/contextmenu.svelte.ts";
  import { lastLabel } from "#lib/trend.ts";
  import type { Line } from "./TrendChart.svelte";

  let {
    charts,
    groups,
    from,
    to,
    name,
    disabled = false,
  }: {
    /** The element holding the TrendCharts, in the order of `groups`. */
    charts: HTMLElement | undefined;
    groups: { unit: string; lines: Line[] }[];
    from: number;
    to: number;
    /** File name without extension. */
    name: string;
    disabled?: boolean;
  } = $props();

  let note = $state<{ ok: boolean; text: string } | null>(null);

  /** The theme's colours; `light` reads them as the light theme has them. */
  function palette(light: boolean): Palette {
    const root = document.documentElement;
    const theme = root.getAttribute("data-theme");
    if (light) root.setAttribute("data-theme", "light");
    try {
      const css = getComputedStyle(root);
      const get = (v: string) => css.getPropertyValue(v).trim();
      return {
        background: get("--card"),
        text: get("--text"),
        text2: get("--text-2"),
        text3: get("--text-3"),
        grid: get("--border"),
        axis: get("--border-strong"),
        series: Array.from({ length: 8 }, (_, i) => get(`--series-${i + 1}`)),
      };
    } finally {
      if (theme === null) root.removeAttribute("data-theme");
      else root.setAttribute("data-theme", theme);
    }
  }

  function measurer() {
    const ctx = document.createElement("canvas").getContext("2d")!;
    return (text: string, size: number, mono: boolean) => {
      ctx.font = `${size}px ${mono ? MONO : FONT}`;
      return ctx.measureText(text).width;
    };
  }

  function image(light: boolean) {
    const p = palette(light);
    const plots = Array.from(
      charts?.querySelectorAll<SVGSVGElement>("svg[data-trend-plot]") ?? [],
    );
    const parts: ChartPart[] = groups.flatMap((g, k) => {
      const svg = plots[k];
      if (!svg) return [];
      return [
        {
          unit: g.unit,
          legend: g.lines.map((l) => ({
            name: l.reg.name,
            value: lastLabel(l.samples),
            slot: l.slot,
          })),
          plot: plotMarkup(svg, p),
          width: svg.width.baseVal.value,
          height: svg.height.baseVal.value,
        },
      ];
    });
    return composeSvg(parts, spanText(from, to), p, measurer());
  }

  /** Draws the SVG at twice its size, for sharp text in reports. */
  async function png(svg: string, width: number, height: number) {
    const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
    try {
      const img = new Image();
      img.src = url;
      await img.decode();
      const canvas = document.createElement("canvas");
      canvas.width = width * 2;
      canvas.height = height * 2;
      const ctx = canvas.getContext("2d")!;
      ctx.scale(2, 2);
      ctx.drawImage(img, 0, 0, width, height);
      const blob = await new Promise<Blob | null>((ok) =>
        canvas.toBlob(ok, "image/png"),
      );
      if (!blob) throw new Error("The image could not be drawn.");
      return new Uint8Array(await blob.arrayBuffer());
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  async function exportImage(format: "png" | "svg", light: boolean) {
    note = null;
    // Taken now: the charts keep moving while the dialog is open.
    const shot = image(light);
    const path = await save({
      defaultPath: `${name}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return;
    try {
      if (format === "svg") await api.saveText(path, shot.svg);
      else
        await api.saveBytes(path, await png(shot.svg, shot.width, shot.height));
      note = { ok: true, text: "Saved." };
    } catch (e) {
      note = { ok: false, text: `Not saved: ${errorText(e)}` };
    }
  }

  function menu(e: MouseEvent) {
    contextMenu.show(e, [
      { label: "PNG…", action: () => exportImage("png", false) },
      { label: "SVG…", action: () => exportImage("svg", false) },
      null,
      { label: "PNG, light theme…", action: () => exportImage("png", true) },
      { label: "SVG, light theme…", action: () => exportImage("svg", true) },
    ]);
  }
</script>

{#if note}
  <span class="note" class:bad={!note.ok} role={note.ok ? undefined : "alert"}
    >{note.text}</span
  >
{/if}
<button class="btn" {disabled} aria-haspopup="menu" onclick={menu}
  >Export image…</button
>

<style>
  .note {
    color: var(--text-3);
    font-size: 12px;
  }

  .note.bad {
    color: var(--bad);
  }
</style>

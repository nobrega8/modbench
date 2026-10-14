// Small motion helpers: a pill that slides under the selected option of a
// segmented control, a container that eases to its new height, and the
// fade-in used when fields are swapped. All of them respect the system's
// reduced-motion setting.

import { cubicOut } from "svelte/easing";
import type { TransitionConfig } from "svelte/transition";

const reduced = () =>
  typeof matchMedia === "function" &&
  matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Slides a pill under the selected button (the child with class `on`) of a
 * segmented control or a navigation bar. The buttons keep their own markup;
 * the pill is drawn behind them.
 */
export function slidePill(node: HTMLElement) {
  const pill = document.createElement("span");
  pill.className = "pill";
  pill.setAttribute("aria-hidden", "true");
  node.prepend(pill);
  node.classList.add("sliding");
  let placed = false;

  const place = () => {
    const on = node.querySelector<HTMLElement>(":scope > .on");
    if (!on) {
      pill.style.opacity = "0";
      return;
    }
    // The first placement jumps; later ones slide.
    if (!placed) pill.style.transition = "none";
    pill.style.opacity = "1";
    pill.style.width = `${on.offsetWidth}px`;
    pill.style.height = `${on.offsetHeight}px`;
    pill.style.transform = `translate(${on.offsetLeft}px, ${on.offsetTop}px)`;
    if (!placed) {
      void pill.offsetWidth;
      pill.style.transition = "";
      placed = true;
    }
  };

  const mutations = new MutationObserver(place);
  mutations.observe(node, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["class"],
  });
  const sizes = new ResizeObserver(place);
  sizes.observe(node);
  place();

  return {
    destroy() {
      mutations.disconnect();
      sizes.disconnect();
      pill.remove();
      node.classList.remove("sliding");
    },
  };
}

/**
 * Eases a container to the height of its only child when the child's size
 * changes, instead of jumping. Content may overflow only while it moves.
 */
export function smoothHeight(node: HTMLElement, enabled = true) {
  const inner = node.firstElementChild as HTMLElement | null;
  if (!inner) return {};
  let height = inner.offsetHeight;
  let on = enabled;
  node.classList.add("smooth-height");

  const done = (e: TransitionEvent) => {
    if (e.target !== node || e.propertyName !== "height") return;
    node.style.height = "";
    node.style.overflow = "";
  };
  node.addEventListener("transitionend", done);

  const sizes = new ResizeObserver(() => {
    const next = inner.offsetHeight;
    const from = node.style.height ? node.offsetHeight : height;
    height = next;
    if (!on || reduced() || Math.abs(next - from) < 2) return;
    node.style.overflow = "clip";
    node.style.height = `${from}px`;
    void node.offsetHeight;
    node.style.height = `${next}px`;
  });
  sizes.observe(inner);

  return {
    update(value: boolean) {
      on = value;
    },
    destroy() {
      sizes.disconnect();
      node.removeEventListener("transitionend", done);
    },
  };
}

/** Fields that appear when an option changes: a short fade and lift. */
export function fieldIn(
  _node: Element,
  { delay = 0 }: { delay?: number } = {},
): TransitionConfig {
  if (reduced()) return { duration: 0 };
  return {
    delay,
    duration: 220,
    easing: cubicOut,
    css: (t) => `opacity: ${t}; transform: translateY(${(1 - t) * 4}px)`,
  };
}

/** A view that replaces another (a tab or a page): a quick fade. */
export function viewIn(_node: Element): TransitionConfig {
  if (reduced()) return { duration: 0 };
  return {
    duration: 180,
    easing: cubicOut,
    css: (t) => `opacity: ${t}`,
  };
}

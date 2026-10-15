// Small motion helpers: a pill that slides under the selected option of a
// segmented control, containers that ease to their new height, and the
// fade-ins used when fields or views are swapped. All of them respect the system's
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

/**
 * Eases a box whose height follows its content (a dialog) to its new height,
 * instead of jumping, when the size of any of its `[data-size]` descendants
 * changes. Those must be sized by their content, not by the box.
 */
export function smoothResize(node: HTMLElement) {
  let last = 0;
  let anim: Animation | null = null;

  const measure = () => {
    // Read the height on screen first: an animation may be halfway.
    const from = anim ? node.getBoundingClientRect().height : last;
    anim?.cancel();
    anim = null;
    const to = node.getBoundingClientRect().height;
    last = to;
    // Hidden (a closed dialog) or just shown: nothing to ease from.
    if (!from || !to || reduced() || Math.abs(to - from) < 2) return;
    anim = node.animate([{ height: `${from}px` }, { height: `${to}px` }], {
      duration: 260,
      easing: "cubic-bezier(0.32, 0.72, 0, 1)",
    });
    anim.onfinish = () => (anim = null);
  };

  const sizes = new ResizeObserver(measure);
  for (const el of node.querySelectorAll("[data-size]")) sizes.observe(el);
  // A window resize changes the box without easing; only remember it.
  const remember = () => {
    if (!anim) last = node.getBoundingClientRect().height;
  };
  window.addEventListener("resize", remember);

  return {
    destroy() {
      sizes.disconnect();
      anim?.cancel();
      window.removeEventListener("resize", remember);
    },
  };
}

/**
 * A view that replaces another inside a panel: a fade with a short slide,
 * from the right going forward (`dir` 1), from the left going back (-1).
 */
export function slideIn(
  _node: Element,
  { dir = 0 }: { dir?: number } = {},
): TransitionConfig {
  if (reduced()) return { duration: 0 };
  return {
    duration: 220,
    easing: cubicOut,
    css: (t) => `opacity: ${t}; transform: translateX(${(1 - t) * 16 * dir}px)`,
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

/** A popover (menu, dropdown list, picker) opening: fade, drop and grow from
 * the edge it is attached to. */
export function popIn(
  _node: Element,
  { above = false }: { above?: boolean } = {},
): TransitionConfig {
  if (reduced()) return { duration: 0 };
  const dir = above ? 1 : -1;
  return {
    duration: 160,
    easing: cubicOut,
    css: (t) =>
      `opacity: ${t}; transform: translateY(${(1 - t) * 4 * dir}px) scale(${0.98 + 0.02 * t})`,
  };
}

/** A popover closing: a quick fade. */
export function popOut(_node: Element): TransitionConfig {
  if (reduced()) return { duration: 0 };
  return { duration: 100, css: (t) => `opacity: ${t}` };
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

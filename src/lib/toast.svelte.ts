// Short notes that confirm something done (a map deleted, an image saved) or
// say why it failed, shown in a stack at the bottom right of the window. They
// go away by themselves; a failure stays longer, and none goes while the
// pointer is over the stack. A note can also hold off something until it goes
// (a map deleted), with a button that undoes it before then.

export interface Toast {
  id: number;
  ok: boolean;
  text: string;
  /** Label of the button that undoes what the note is about. */
  undo?: string;
}

const SHOWN_MS = 4000;
const UNDO_MS = 7000;
const FAILED_MS = 9000;
const MAX = 4;

class Toasts {
  list = $state<Toast[]>([]);
  #next = 1;
  #timers = new Map<number, ReturnType<typeof setTimeout>>();
  #paused = false;
  #pending = new Map<number, { commit: () => void; revert?: () => void }>();

  show(text: string, ok = true) {
    this.#add({ id: this.#next++, ok, text });
  }

  /** A note for something not done yet: `commit` does it once the note
   * goes, however it goes, unless its button was pressed first; then
   * `revert` runs instead. */
  undoable(
    text: string,
    undo: string,
    commit: () => void,
    revert?: () => void,
  ) {
    const toast = { id: this.#next++, ok: true, text, undo };
    this.#pending.set(toast.id, { commit, revert });
    this.#add(toast);
  }

  fail(text: string) {
    this.show(text, false);
  }

  dismiss(id: number) {
    const held = this.#pending.get(id);
    this.#pending.delete(id);
    held?.commit();
    this.#remove(id);
  }

  /** The note's button: drop what it held off, then the note. */
  undo(id: number) {
    const held = this.#pending.get(id);
    this.#pending.delete(id);
    held?.revert?.();
    this.#remove(id);
  }

  #add(toast: Toast) {
    for (const old of this.list.slice(MAX - 1)) this.dismiss(old.id);
    this.list = [toast, ...this.list];
    if (!this.#paused) this.#arm(toast);
  }

  #remove(id: number) {
    this.#clear(id);
    this.list = this.list.filter((t) => t.id !== id);
    // The stack is gone, and with it the pointer that paused it.
    if (!this.list.length) this.#paused = false;
  }

  /** The pointer is over the stack: keep every note until it leaves. */
  pause() {
    this.#paused = true;
    for (const id of [...this.#timers.keys()]) this.#clear(id);
  }

  resume() {
    this.#paused = false;
    for (const t of this.list) this.#arm(t);
  }

  #arm(t: Toast) {
    this.#clear(t.id);
    this.#timers.set(
      t.id,
      setTimeout(
        () => this.dismiss(t.id),
        t.undo ? UNDO_MS : t.ok ? SHOWN_MS : FAILED_MS,
      ),
    );
  }

  #clear(id: number) {
    clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
  }
}

export const toasts = new Toasts();

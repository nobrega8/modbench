// The names of the highlight colours and views, in the current language.

import { t } from "./i18n.svelte.ts";
import type { Mark, MarksView } from "./highlights";

export function markName(mark: Mark): string {
  switch (mark) {
    case 1:
      return t("maptable.mark_1");
    case 2:
      return t("maptable.mark_2");
    case 3:
      return t("maptable.mark_3");
    case 4:
      return t("maptable.mark_4");
  }
}

export function viewName(view: MarksView): string {
  switch (view) {
    case "all":
      return t("maptable.marks_all");
    case "first":
      return t("maptable.marks_first");
    case "only":
      return t("maptable.marks_only");
  }
}

// Release notes as the update banner shows them. A release's notes list the
// pull requests merged since the previous one (GitHub writes them, see
// .github/workflows/release.yml), then how to download it.

/** What changed, one entry per pull request, without who merged it. */
export function changes(body: string): string[] {
  return body
    .split(/\r?\n/)
    .map((l) => /^\s*[*-]\s+(.+)$/.exec(l)?.[1] ?? null)
    .filter((l): l is string => l !== null)
    .map((l) => l.replace(/\s+by @\S+ in \S+$/, "").trim())
    .filter((l) => l !== "");
}

/** The changes on one line, or the notes as they are when they list none. */
export function summary(body: string): string {
  const list = changes(body);
  return list.length ? list.join(" · ") : body.trim();
}

// Matching a map against what the user types in a library search.

/** Whether the model name or one of the other models a map applies to
    (`models`, e.g. "EM-210" in a map for EM-200) contains the query, which
    is already trimmed and lower case. */
export function modelMatches(
  query: string,
  model: string,
  models: readonly string[] = [],
): boolean {
  return [model, ...models].some((m) => m.toLowerCase().includes(query));
}

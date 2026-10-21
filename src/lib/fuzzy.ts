/** 4 equal, 3 prefix, 2 substring, 0 none; ignores case. */
function score(title: string, needle: string): number {
  const t = title.toLowerCase();
  const n = needle.toLowerCase();
  if (t === n) return 4;
  if (t.startsWith(n)) return 3;
  if (t.includes(n)) return 2;
  return 0;
}

function inOrder(text: string, needle: string): boolean {
  let at = 0;
  for (const ch of needle) {
    const hit = text.indexOf(ch, at);
    if (hit < 0) return false;
    at = hit + 1;
  }
  return true;
}

/** The best `score` over `fields`, else 1 when one field holds the needle's characters in order. */
function fuzzyScore(fields: string[], needle: string): number {
  const best = Math.max(0, ...fields.map((f) => score(f, needle)));
  if (best > 0) return best;
  const n = needle.toLowerCase();
  return fields.some((f) => inOrder(f.toLowerCase(), n)) ? 1 : 0;
}

/** `items` with a positive score for `needle`, best first; equal scores keep their order. */
export function fuzzyFilter<T>(items: T[], needle: string, fields: (item: T) => string[]): T[] {
  const n = needle.trim();
  if (!n) return items;
  return items
    .map((item, i) => ({ item, i, score: fuzzyScore(fields(item), n) }))
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score || a.i - b.i)
    .map((x) => x.item);
}

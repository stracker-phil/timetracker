export type Prefix = ">" | "p:" | "c:" | "@" | null;

const PREFIXES = [">", "p:", "c:", "@"] as const;

/** The prefix (`>` commands, `p:` projects, `c:` clients, `@` dates) and the text after it. */
export function parsePrefix(input: string): { prefix: Prefix; needle: string } {
  const q = input.trimStart();
  const prefix = PREFIXES.find((p) => q.startsWith(p)) ?? null;
  return { prefix, needle: (prefix ? q.slice(prefix.length) : q).trim() };
}

const isWordChar = (ch: string | undefined) => ch !== undefined && /[\p{L}\p{N}]/u.test(ch);

function startsAWord(text: string, needle: string): boolean {
  for (let at = text.indexOf(needle); at >= 0; at = text.indexOf(needle, at + 1)) {
    if (!isWordChar(text[at - 1])) return true;
  }
  return false;
}

/** 5 equal, 4 prefix, 3 the start of a word, 2 inside a word, 0 none; ignores case. */
function fieldScore(field: string, needle: string): number {
  const t = field.toLowerCase();
  const n = needle.toLowerCase();
  if (t === n) return 5;
  if (t.startsWith(n)) return 4;
  if (startsAWord(t, n)) return 3;
  return t.includes(n) ? 2 : 0;
}

/** The best match over `fields`; an empty needle matches everything with 1. */
export function score(fields: string[], needle: string): number {
  if (!needle) return 1;
  return Math.max(0, ...fields.map((f) => fieldScore(f, needle)));
}

function inOrder(text: string, needle: string): boolean {
  let at = 0;
  for (const ch of needle) {
    at = text.indexOf(ch, at) + 1;
    if (at === 0) return false;
  }
  return true;
}

/** `score`, else 1 when one field holds the needle's characters in order (names, not command titles). */
export function fuzzyScore(fields: string[], needle: string): number {
  const best = score(fields, needle);
  if (best > 0) return best;
  const n = needle.toLowerCase();
  return fields.some((f) => inOrder(f.toLowerCase(), n)) ? 1 : 0;
}

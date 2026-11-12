export type Prefix = ">" | "p:" | "c:" | "r:" | "@" | null;

const PREFIXES = [">", "p:", "c:", "r:", "@"] as const;

/** The prefix (`>` commands, `p:` projects, `c:` clients, `r:` recordings and suggestions, `@` dates) and the text after it. */
export function parsePrefix(input: string): { prefix: Prefix; needle: string } {
  const q = input.trimStart();
  const prefix = PREFIXES.find((p) => q.startsWith(p)) ?? null;
  return { prefix, needle: (prefix ? q.slice(prefix.length) : q).trim() };
}

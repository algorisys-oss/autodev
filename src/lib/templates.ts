import type { PromptTemplate } from "./ipc";

/** Templates whose name matches the slash-command being typed (`/ref` → `refactor`). Empty
 *  unless `text` is a bare `/partialName` with nothing typed after it yet — so the suggestions
 *  disappear once the user moves past naming the command. */
export function templateMatches(text: string, templates: PromptTemplate[]): PromptTemplate[] {
  const m = text.match(/^\/([\w-]*)$/);
  if (!m) return [];
  const q = m[1].toLowerCase();
  return templates.filter((t) => t.name.toLowerCase().startsWith(q));
}

/** Expand a leading `/name` slash-command to its template body, keeping any text typed after
 *  the command (`/refactor the parser` → `<body> the parser`). Returns null when `text` is not
 *  a slash-command for a known template. */
export function expandTemplate(text: string, templates: PromptTemplate[]): string | null {
  const m = text.match(/^\/([\w-]+)(?:\s+([\s\S]*))?$/);
  if (!m) return null;
  const tpl = templates.find((t) => t.name === m[1]);
  if (!tpl) return null;
  const rest = m[2]?.trim();
  return rest ? `${tpl.body} ${rest}` : tpl.body;
}

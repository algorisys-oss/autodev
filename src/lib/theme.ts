/** Light/dark theming (attribute-driven). The CSS keys off `:root[data-theme="dark"|"light"]`;
 *  this module resolves the user's preference to an effective theme, stamps the attribute, and
 *  keeps it in sync with the OS while in "system" mode. The preference is stored in
 *  localStorage so a tiny inline script in index.html can apply it before first paint (no flash).
 */

export type ThemePref = "light" | "dark" | "system";

const KEY = "autodev-theme";

/** The saved preference, or "system" if unset/invalid. */
export function getThemePref(): ThemePref {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" || v === "system" ? v : "system";
}

/** Resolve a preference to the concrete theme to render. */
export function resolveTheme(pref: ThemePref): "light" | "dark" {
  if (pref === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return pref;
}

/** Stamp `data-theme` on the document root so the CSS applies. Returns the effective theme. */
export function applyTheme(pref: ThemePref): "light" | "dark" {
  const effective = resolveTheme(pref);
  document.documentElement.setAttribute("data-theme", effective);
  return effective;
}

/** Persist a preference and apply it. Returns the effective theme. */
export function setThemePref(pref: ThemePref): "light" | "dark" {
  localStorage.setItem(KEY, pref);
  return applyTheme(pref);
}

/** While in "system" mode, follow OS theme changes. Returns an unsubscribe function. */
export function watchSystemTheme(getPref: () => ThemePref, onChange?: (t: "light" | "dark") => void): () => void {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = () => {
    if (getPref() === "system") onChange?.(applyTheme("system"));
  };
  mq.addEventListener("change", handler);
  return () => mq.removeEventListener("change", handler);
}

// Typed wrappers over Tauri commands. This is the single place the frontend and
// Rust core agree on the command contract. Keep these types in sync with the
// structs in src-tauri/src/commands.rs and src-tauri/src/state.rs.
import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
}

export interface AppSettings {
  theme: "system" | "light" | "dark";
  defaultEffort: "high" | "extra-high";
}

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

export function setSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("set_settings", { settings });
}

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

// --- Workspaces & projects (Phase 1) ---

export interface Project {
  name: string;
  path: string;
}

export interface Workspace {
  id: string;
  name: string;
  projects: Project[];
}

export interface ResolvedMention {
  project: Project;
  files: string[];
  truncated: boolean;
}

export function listWorkspaces(): Promise<Workspace[]> {
  return invoke<Workspace[]>("list_workspaces");
}

export function createWorkspace(name: string): Promise<Workspace> {
  return invoke<Workspace>("create_workspace", { name });
}

export function deleteWorkspace(id: string): Promise<void> {
  return invoke<void>("delete_workspace", { id });
}

export function addProject(workspaceId: string, path: string): Promise<Workspace> {
  return invoke<Workspace>("add_project", { workspaceId, path });
}

export function removeProject(workspaceId: string, projectName: string): Promise<Workspace> {
  return invoke<Workspace>("remove_project", { workspaceId, projectName });
}

export function resolveMention(
  workspaceId: string,
  token: string,
): Promise<ResolvedMention | null> {
  return invoke<ResolvedMention | null>("resolve_mention", { workspaceId, token });
}

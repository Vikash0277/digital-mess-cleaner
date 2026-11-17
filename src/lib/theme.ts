// src/lib/theme.ts
// Theme + platform provider for the Tauri app.
//
// main.tsx:
//   import "./styles/theme.css";
//   import { initTheme } from "./lib/theme";
//   initTheme();                       // before createRoot(...).render(...)
//
// In a component:
//   const { mode, resolved, setMode } = useTheme();
//
// Tauri v2: syncing the native window theme needs the permission
//   "core:window:allow-set-theme"   in src-tauri/capabilities/default.json
// Without it the call fails silently and CSS theming still works.

import { useSyncExternalStore } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";
export type Platform = "macos" | "windows" | "linux";
export type Density = "comfortable" | "compact";

const THEME_KEY = "dmc.theme";
const DENSITY_KEY = "dmc.density";

const mq = window.matchMedia("(prefers-color-scheme: dark)");
const listeners = new Set<() => void>();

function read<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return allowed.includes(v as T) ? (v as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* storage unavailable: setting just won't persist */
  }
}

export function detectPlatform(): Platform {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("mac")) return "macos";
  if (ua.includes("win")) return "windows";
  return "linux";
}

let mode: ThemeMode = read<ThemeMode>(THEME_KEY, ["system", "light", "dark"], "system");
let density: Density = read<Density>(DENSITY_KEY, ["comfortable", "compact"], "comfortable");

function resolve(m: ThemeMode): ResolvedTheme {
  return m === "system" ? (mq.matches ? "dark" : "light") : m;
}

let resolved: ResolvedTheme = resolve(mode);

function apply() {
  resolved = resolve(mode);
  const root = document.documentElement;
  root.dataset.theme = resolved;
  root.dataset.platform = detectPlatform();
  root.dataset.density = density;

  // Keep native window chrome (title bar, menus, scrollbars) in step. No-op in a plain browser.
  try {
    void getCurrentWindow()
      .setTheme(mode === "system" ? null : mode)
      .catch(() => {});
  } catch {
    /* not running inside Tauri */
  }

  listeners.forEach((l) => l());
}

export function initTheme() {
  apply();
  mq.addEventListener("change", () => {
    if (mode === "system") apply();
  });
}

export function setThemeMode(next: ThemeMode) {
  mode = next;
  write(THEME_KEY, next);
  apply();
}

export function setDensity(next: Density) {
  density = next;
  write(DENSITY_KEY, next);
  apply();
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

export function useTheme() {
  const m = useSyncExternalStore(subscribe, () => mode);
  const r = useSyncExternalStore(subscribe, () => resolved);
  const d = useSyncExternalStore(subscribe, () => density);
  return { mode: m, resolved: r, density: d, setMode: setThemeMode, setDensity };
}

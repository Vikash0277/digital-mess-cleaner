import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Monitor, Moon, ShieldCheck, Square, Sun, X } from "lucide-react";
import { useTheme, type ThemeMode } from "../lib/theme";

const THEME_CHOICES: { value: ThemeMode; label: string; Icon: typeof Sun }[] = [
  { value: "system", label: "System theme", Icon: Monitor },
  { value: "light", label: "Light theme", Icon: Sun },
  { value: "dark", label: "Dark theme", Icon: Moon },
];

type WindowAction = "minimize" | "toggleMaximize" | "close";

function runWindowAction(action: WindowAction) {
  try {
    const appWindow = getCurrentWindow();
    const result =
      action === "minimize"
        ? appWindow.minimize()
        : action === "toggleMaximize"
          ? appWindow.toggleMaximize()
          : appWindow.close();
    void result.catch(() => {
      /* not running inside Tauri */
    });
  } catch {
    /* not running inside Tauri */
  }
}

export function Titlebar() {
  const { mode, setMode } = useTheme();

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-brand" data-tauri-drag-region>
        <span className="titlebar-logo" aria-hidden="true">
          <ShieldCheck size={16} />
        </span>
        <span className="titlebar-title" data-tauri-drag-region>
          Digital Mess Cleaner
        </span>
      </div>

      <div className="titlebar-spacer" data-tauri-drag-region />

      <div className="titlebar-actions" role="group" aria-label="Theme">
        {THEME_CHOICES.map(({ value, label, Icon }) => (
          <button
            key={value}
            type="button"
            className="titlebar-button"
            onClick={() => setMode(value)}
            title={label}
            aria-label={label}
            aria-pressed={mode === value}
          >
            <Icon size={14} />
          </button>
        ))}
      </div>

      <div className="window-controls" role="group" aria-label="Window controls">
        <button
          type="button"
          className="window-control window-control-minimize"
          onClick={() => runWindowAction("minimize")}
          title="Minimize"
          aria-label="Minimize window"
        >
          <Minus size={14} />
        </button>
        <button
          type="button"
          className="window-control window-control-maximize"
          onClick={() => runWindowAction("toggleMaximize")}
          title="Maximize"
          aria-label="Maximize or restore window"
        >
          <Square size={12} />
        </button>
        <button
          type="button"
          className="window-control window-control-close"
          onClick={() => runWindowAction("close")}
          title="Close"
          aria-label="Close window"
        >
          <X size={14} />
        </button>
      </div>
    </header>
  );
}

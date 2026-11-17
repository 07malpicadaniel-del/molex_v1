import React, { useEffect, useRef, useState } from "react";
import { CornerDownLeft, Loader2, Terminal, Trash2 } from "lucide-react";
import type { DeviceProfile, TerminalEntry } from "../types/molex";

interface SshScreenProps {
  activeDevice: DeviceProfile | null;
  activeHost: string | null;
  cwd: string;
  logs: TerminalEntry[];
  onExecuteCommand: (cmd: string) => Promise<void>;
  onClearLogs: () => void;
  onNavigateDevices: () => void;
}

const QUICK_COMMANDS = [
  "pwd",
  "ls -la",
  "hyprctl monitors",
  "niri msg outputs",
  "nvidia-smi",
  "systemctl --user status molex-daemon",
];

export const SshScreen: React.FC<SshScreenProps> = ({
  activeDevice,
  activeHost,
  cwd,
  logs,
  onExecuteCommand,
  onClearLogs,
  onNavigateDevices,
}) => {
  const [command, setCommand] = useState("");
  const [running, setRunning] = useState(false);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIdx, setHistoryIdx] = useState<number>(-1);
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  const submitCommand = async (rawCmd?: string) => {
    const cmd = (rawCmd ?? command).trim();
    if (!cmd || running) return;

    if (cmd === "clear") {
      onClearLogs();
      setCommand("");
      return;
    }

    setHistory((prev) => [cmd, ...prev.filter((h) => h !== cmd)].slice(0, 100));
    setHistoryIdx(-1);
    if (!rawCmd) setCommand("");
    setRunning(true);
    try {
      await onExecuteCommand(cmd);
    } finally {
      setRunning(false);
      inputRef.current?.focus();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      if (history.length === 0) return;
      const next = Math.min(historyIdx + 1, history.length - 1);
      setHistoryIdx(next);
      setCommand(history[next]);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (historyIdx <= 0) {
        setHistoryIdx(-1);
        setCommand("");
      } else {
        const next = historyIdx - 1;
        setHistoryIdx(next);
        setCommand(history[next]);
      }
    } else if (e.ctrlKey && e.key.toLowerCase() === "l") {
      e.preventDefault();
      onClearLogs();
    }
  };

  if (!activeDevice || !activeHost) {
    return (
      <div className="flex h-full flex-col items-center justify-center bg-molex-bg p-8 text-center">
        <Terminal className="h-12 w-12 text-molex-accent" />
        <h2 className="mt-4 text-xl font-bold text-molex-text">Terminal SSH desconectada</h2>
        <p className="mt-1 max-w-md text-sm text-molex-muted">
          Conéctate primero a un equipo desde Dispositivos para abrir una consola SSH con
          directorio persistente.
        </p>
        <button
          type="button"
          onClick={onNavigateDevices}
          className="mt-6 rounded-lg bg-molex-accent px-5 py-2.5 text-sm font-semibold text-molex-bg hover:bg-molex-light"
        >
          Ir a Dispositivos
        </button>
      </div>
    );
  }

  const promptLabel = `${activeDevice.username}@${activeHost}:${cwd || "~"} $`;

  return (
    <div className="flex h-full flex-col bg-[#0B0B0C]">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-molex-variant bg-molex-surface px-5 py-3">
        <div className="flex items-center gap-2.5">
          <Terminal className="h-4 w-4 text-molex-accent" />
          <span className="text-sm font-semibold text-molex-text">Consola SSH Persistente</span>
          <span className="rounded bg-molex-bg px-2 py-0.5 font-mono text-xs text-molex-light">
            {promptLabel}
          </span>
        </div>

        <button
          type="button"
          onClick={onClearLogs}
          className="inline-flex items-center gap-1.5 rounded-lg border border-molex-variant bg-molex-bg px-3 py-1.5 text-xs text-molex-muted hover:text-molex-text"
        >
          <Trash2 className="h-3.5 w-3.5" />
          Limpiar
        </button>
      </div>

      {/* Quick command chips */}
      <div className="flex flex-wrap items-center gap-1.5 border-b border-molex-variant/60 bg-molex-surface/50 px-5 py-2">
        <span className="mr-1 text-[11px] font-medium text-molex-muted">Rápidos:</span>
        {QUICK_COMMANDS.map((q) => (
          <button
            key={q}
            type="button"
            disabled={running}
            onClick={() => submitCommand(q)}
            className="rounded-md border border-molex-variant bg-molex-bg px-2 py-1 font-mono text-[11px] text-molex-light hover:border-molex-accent hover:bg-molex-variant disabled:opacity-50"
          >
            {q}
          </button>
        ))}
      </div>

      {/* Output Scroll Area */}
      <div
        onClick={() => inputRef.current?.focus()}
        className="flex-1 overflow-y-auto px-5 py-4 font-mono text-[13px] leading-relaxed"
      >
        {logs.length === 0 && (
          <p className="text-xs text-molex-muted">
            Sesión SSH lista. Los comandos <span className="text-molex-light">cd</span> persisten
            su directorio de trabajo entre ejecuciones.
          </p>
        )}

        <div className="space-y-2.5">
          {logs.map((entry) => {
            if (entry.kind === "command") {
              return (
                <div key={entry.id} className="flex flex-wrap items-baseline gap-2 pt-1">
                  <span className="text-emerald-400 font-semibold">
                    {entry.prompt || promptLabel}
                  </span>
                  <span className="text-molex-text font-semibold">{entry.text}</span>
                </div>
              );
            }
            if (entry.kind === "error") {
              return (
                <pre
                  key={entry.id}
                  className="whitespace-pre-wrap break-words rounded border-l-2 border-red-500 bg-red-950/20 px-3 py-1.5 text-red-300"
                >
                  {entry.text}
                </pre>
              );
            }
            if (entry.kind === "info") {
              return (
                <div key={entry.id} className="text-xs text-molex-accent">
                  {entry.text}
                </div>
              );
            }
            return (
              <pre
                key={entry.id}
                className="whitespace-pre-wrap break-words text-zinc-200"
              >
                {entry.text}
              </pre>
            );
          })}
        </div>
        <div ref={bottomRef} />
      </div>

      {/* Command Input Row */}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submitCommand();
        }}
        className="flex items-center gap-2.5 border-t border-molex-variant bg-molex-surface px-4 py-3"
      >
        <span className="hidden font-mono text-xs font-semibold text-molex-accent sm:inline">
          {cwd || "~"} $
        </span>
        <input
          ref={inputRef}
          type="text"
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Escribe un comando (ej. cd ~/proyectos, htop, ls -la)..."
          autoFocus
          className="flex-1 rounded-lg border border-molex-variant bg-molex-bg px-3.5 py-2 font-mono text-sm text-molex-text placeholder:text-zinc-500 focus:border-molex-accent focus:outline-none"
        />
        <button
          type="submit"
          disabled={running || !command.trim()}
          className="inline-flex items-center gap-1.5 rounded-lg bg-molex-accent px-4 py-2 text-xs font-semibold text-molex-bg hover:bg-molex-light disabled:opacity-40"
        >
          {running ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <CornerDownLeft className="h-4 w-4" />
          )}
          Ejecutar
        </button>
      </form>
    </div>
  );
};

import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { bumpRecent, getRecentIds } from "./lib/recents";
import { getBlurToHide } from "./lib/settings";
import {
  cancelRun,
  capabilities,
  createRunId,
  hidePalette,
  hotkeyStatus,
  listTools,
  onOpenSettings,
  onSelectTool,
  runTool,
  type CapabilitiesReport,
  type HotkeyState,
  type Progress,
  type RunId,
  type ToolOutcome,
  type ToolSummary,
} from "./lib/wield";
import { ArgForm } from "./palette/ArgForm";
import { ResultView } from "./palette/ResultView";
import { RunningView } from "./palette/RunningView";
import { SearchView } from "./palette/SearchView";
import { SettingsView } from "./palette/SettingsView";
import { useKeyboardNav } from "./useKeyboardNav";

type CompletedOutcome = Exclude<ToolOutcome, "Cancelled">;

type View =
  | { kind: "search" }
  | { kind: "settings" }
  | { kind: "form"; tool: ToolSummary; values: Record<string, unknown>; viaRunAgain: boolean }
  | {
      kind: "running";
      tool: ToolSummary;
      values: Record<string, unknown>;
      runId: RunId | null;
      message: string | null;
      percent: number | null;
      viaRunAgain: boolean;
    }
  | {
      kind: "result";
      tool: ToolSummary;
      outcome: CompletedOutcome;
      lastValues: Record<string, unknown>;
      viaRunAgain: boolean;
    };

interface State {
  view: View;
  query: string;
  tools: ToolSummary[];
  showingRecents: boolean;
  selectedIndex: number;
}

type Action =
  | { type: "SET_QUERY"; query: string }
  | { type: "SET_TOOLS"; tools: ToolSummary[]; showingRecents: boolean }
  | { type: "SELECT_INDEX"; index: number }
  | { type: "OPEN_FORM"; tool: ToolSummary }
  | { type: "OPEN_SETTINGS" }
  | {
      type: "RUN";
      tool: ToolSummary;
      values: Record<string, unknown>;
      runId: RunId;
      viaRunAgain: boolean;
    }
  | { type: "PROGRESS"; progress: Progress }
  | {
      type: "FINISHED";
      outcome: ToolOutcome;
      tool: ToolSummary;
      values: Record<string, unknown>;
      viaRunAgain: boolean;
    }
  | { type: "ESCAPE" }
  | { type: "RUN_AGAIN" };

const initialState: State = {
  view: { kind: "search" },
  query: "",
  tools: [],
  showingRecents: true,
  selectedIndex: 0,
};

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "SET_QUERY":
      return { ...state, query: action.query, selectedIndex: 0 };
    case "SET_TOOLS":
      return { ...state, tools: action.tools, showingRecents: action.showingRecents, selectedIndex: 0 };
    case "SELECT_INDEX":
      return { ...state, selectedIndex: action.index };
    case "OPEN_FORM":
      return { ...state, view: { kind: "form", tool: action.tool, values: {}, viaRunAgain: false } };
    case "OPEN_SETTINGS":
      return { ...state, view: { kind: "settings" } };
    case "RUN":
      return {
        ...state,
        view: {
          kind: "running",
          tool: action.tool,
          values: action.values,
          runId: action.runId,
          message: null,
          percent: null,
          viaRunAgain: action.viaRunAgain,
        },
      };
    case "PROGRESS": {
      if (state.view.kind !== "running") return state;
      const { progress } = action;
      if (progress === "Started") {
        // A new file (or the run's only file) is starting - clear the
        // previous file's percent so its bar doesn't linger. Keep
        // `message`: a batch's "Converting N of M" label was just set by
        // the Message event that preceded this Started event.
        return { ...state, view: { ...state.view, percent: null } };
      }
      if (typeof progress === "object" && "Message" in progress) {
        return { ...state, view: { ...state.view, message: progress.Message, percent: null } };
      }
      if (typeof progress === "object" && "Percent" in progress) {
        return { ...state, view: { ...state.view, percent: progress.Percent } };
      }
      // "Finished" - no visible change; FINISHED (a separate action) drives
      // the transition to the result view.
      return state;
    }
    case "FINISHED":
      if (action.outcome === "Cancelled") return { ...state, view: { kind: "search" } };
      return {
        ...state,
        view: {
          kind: "result",
          tool: action.tool,
          outcome: action.outcome,
          lastValues: action.values,
          viaRunAgain: action.viaRunAgain,
        },
      };
    case "ESCAPE":
      if (state.view.kind === "form" || state.view.kind === "settings") {
        return { ...state, view: { kind: "search" } };
      }
      if (state.view.kind === "result") {
        return state.view.viaRunAgain
          ? {
              ...state,
              view: {
                kind: "form",
                tool: state.view.tool,
                values: state.view.lastValues,
                viaRunAgain: true,
              },
            }
          : { ...state, view: { kind: "search" } };
      }
      return state;
    case "RUN_AGAIN":
      return state.view.kind === "result"
        ? {
            ...state,
            view: {
              kind: "form",
              tool: state.view.tool,
              values: state.view.lastValues,
              viaRunAgain: true,
            },
          }
        : state;
  }
}

interface SearchControllerProps {
  state: State;
  onQueryChange: (query: string) => void;
  onSelectIndex: (index: number) => void;
  onActivate: (tool: ToolSummary) => void;
  onTopLevelEscape: () => void;
  onOpenSettings: () => void;
}

function SearchController({
  state,
  onQueryChange,
  onSelectIndex,
  onActivate,
  onTopLevelEscape,
  onOpenSettings,
}: SearchControllerProps) {
  const selected = state.tools[state.selectedIndex];
  useKeyboardNav({
    itemCount: state.tools.length,
    selectedIndex: state.selectedIndex,
    onSelectIndex,
    onActivate: () => {
      if (selected !== undefined) onActivate(selected);
    },
    onEscape: onTopLevelEscape,
  });
  return (
    <SearchView
      query={state.query}
      onQueryChange={onQueryChange}
      tools={state.tools}
      showingRecents={state.showingRecents}
      onOpenSettings={onOpenSettings}
      selectedIndex={state.selectedIndex}
      onSelectIndex={onSelectIndex}
      onActivate={onActivate}
    />
  );
}

function recentTools(tools: ToolSummary[]): ToolSummary[] {
  const ids = getRecentIds();
  if (ids.length === 0) return tools;
  const byId = new Map(tools.map((tool) => [tool.id, tool]));
  return ids.flatMap((id) => {
    const tool = byId.get(id);
    return tool === undefined ? [] : [tool];
  });
}

export default function App() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const attemptRef = useRef(0);
  const contentRef = useRef<HTMLDivElement>(null);
  const [cardHeight, setCardHeight] = useState<number | undefined>(undefined);
  const [hotkey, setHotkey] = useState<HotkeyState | null>(null);
  const [report, setReport] = useState<CapabilitiesReport | null>(null);

  // Fetched once up front (not lazily when settings opens) so the view has
  // its data ready the instant it's shown, matching how tool results are
  // never blocked on a fresh network round trip either.
  useEffect(() => {
    void hotkeyStatus().then(setHotkey);
  }, []);

  useEffect(() => {
    void capabilities().then(setReport);
  }, []);

  // Width is fixed; height follows the card's actual rendered content —
  // search results, an arg form, progress, or a result card each have a
  // different natural height. The palette's own OS window is always
  // full-screen now (see layer_shell::configure) so there's no window to
  // resize any more; only the visible card animates, entirely via CSS
  // (`.app-shell`'s `transition: height`) driven by this measured value.
  // The observer watches the INNER content wrapper (its natural,
  // unconstrained height) rather than `.app-shell` itself, which now has
  // its height explicitly set below - observing the same element you're
  // setting would just re-trigger on your own write.
  useEffect(() => {
    const element = contentRef.current;
    if (element === null) return;
    const observer = new ResizeObserver((entries) => {
      const height = entries[0]?.contentRect.height;
      if (height !== undefined) setCardHeight(height);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const query = state.query.trim();
    let active = true;
    const timer = window.setTimeout(() => {
      void listTools(query || undefined)
        .then((tools) => {
          if (!active) return;
          dispatch({
            type: "SET_TOOLS",
            tools: query === "" ? recentTools(tools) : tools,
            showingRecents: query === "",
          });
        })
        .catch(() => {
          if (!active) {
            return;
          }
          dispatch({
            type: "SET_TOOLS",
            tools: [],
            showingRecents: query === "",
          });
        });
    }, 120);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [state.query]);

  useEffect(() => {
    const onBlur = () => {
      if (getBlurToHide()) void hidePalette();
    };
    window.addEventListener("blur", onBlur);
    return () => window.removeEventListener("blur", onBlur);
  }, []);

  const startRun = useCallback(
    (tool: ToolSummary, values: Record<string, unknown>, viaRunAgain: boolean) => {
      bumpRecent(tool.id);
      const runId = createRunId();
      dispatch({ type: "RUN", tool, values, viaRunAgain, runId });
      const attempt = ++attemptRef.current;
      void runTool(tool.id, values, (progress) => {
        if (attempt === attemptRef.current) dispatch({ type: "PROGRESS", progress });
      }, runId)
        .then(({ outcome }) => {
          if (attempt === attemptRef.current) {
            dispatch({ type: "FINISHED", outcome, tool, values, viaRunAgain });
          }
        })
        .catch((error: unknown) => {
          if (attempt !== attemptRef.current) return;
          dispatch({
            type: "FINISHED",
            tool,
            values,
            viaRunAgain,
            outcome: {
              Failed: {
                stage: "Native",
                detail: error instanceof Error ? error.message : String(error),
                hint: null,
              },
            },
          });
        });
    },
    [],
  );

  const activate = useCallback(
    (tool: ToolSummary) => {
      if (!tool.available) return;
      if (tool.args.length === 0) startRun(tool, {}, false);
      else dispatch({ type: "OPEN_FORM", tool });
    },
    [startRun],
  );

  useEffect(() => {
    let unsubscribe: (() => void) | undefined;
    void onSelectTool((toolId) => {
      void listTools().then((tools) => {
        const tool = tools.find((t) => t.id === toolId);
        if (tool) activate(tool);
      });
    }).then((fn) => {
      unsubscribe = fn;
    });
    return () => unsubscribe?.();
  }, [activate]);

  useEffect(() => {
    let unsubscribe: (() => void) | undefined;
    void onOpenSettings(() => dispatch({ type: "OPEN_SETTINGS" })).then((fn) => {
      unsubscribe = fn;
    });
    return () => unsubscribe?.();
  }, []);

  useEffect(() => {
    if (
      state.view.kind !== "running" &&
      state.view.kind !== "result" &&
      state.view.kind !== "settings"
    ) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      if (state.view.kind === "running") {
        if (state.view.runId !== null) void cancelRun(state.view.runId);
      } else {
        dispatch({ type: "ESCAPE" });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [state.view]);

  let content;
  if (state.view.kind === "search") {
    content = (
      <SearchController
        state={state}
        onQueryChange={(query) => dispatch({ type: "SET_QUERY", query })}
        onSelectIndex={(index) => dispatch({ type: "SELECT_INDEX", index })}
        onActivate={activate}
        onTopLevelEscape={() => {
          if (state.query === "") void hidePalette();
          else dispatch({ type: "SET_QUERY", query: "" });
        }}
        onOpenSettings={() => dispatch({ type: "OPEN_SETTINGS" })}
      />
    );
  } else if (state.view.kind === "settings") {
    content = (
      <SettingsView hotkey={hotkey} report={report} onClose={() => dispatch({ type: "ESCAPE" })} />
    );
  } else if (state.view.kind === "form") {
    const { tool, values, viaRunAgain } = state.view;
    content = (
      <ArgForm
        tool={tool}
        initialValues={values}
        onSubmit={(nextValues) => startRun(tool, nextValues, viaRunAgain)}
        onEscape={() => dispatch({ type: "ESCAPE" })}
      />
    );
  } else if (state.view.kind === "running") {
    content = (
      <RunningView
        toolTitle={state.view.tool.title}
        message={state.view.message}
        percent={state.view.percent}
      />
    );
  } else {
    const result = state.view;
    content = (
      <ResultView
        outcome={result.outcome}
        onRunAgain={() => dispatch({ type: "RUN_AGAIN" })}
        onRetry={() => startRun(result.tool, result.lastValues, result.viaRunAgain)}
      />
    );
  }

  return (
    <div
      className="palette-backdrop"
      onMouseDown={(event) => {
        // Only a genuine click on the backdrop itself - not one bubbled up
        // from a child, e.g. a result row - counts as "outside the card".
        // This replaces a separate click-catcher surface: Hyprland never
        // delivers a pointer event to any surface other than the one
        // holding KeyboardMode::Exclusive (hyprwm/Hyprland#14136), so a
        // second surface can never see an outside click. The palette's own
        // surface now covers the whole output instead (see
        // layer_shell::configure), so every click - wherever it lands -
        // reaches this same webview, and ordinary DOM hit-testing (already
        // proven reliable: this is exactly how clicking a result row
        // already worked) tells this handler whether it landed on the
        // backdrop or bubbled up from the card.
        if (event.target === event.currentTarget) void hidePalette();
      }}
    >
      <main className="app-shell" style={{ height: cardHeight }}>
        <div ref={contentRef} className="app-shell-content">
          {content}
        </div>
      </main>
    </div>
  );
}

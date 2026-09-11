import { useCallback, useEffect, useReducer, useRef } from "react";
import { bumpRecent, getRecentIds } from "./lib/recents";
import {
  cancelRun,
  createRunId,
  hidePalette,
  listTools,
  runTool,
  type Progress,
  type RunId,
  type ToolOutcome,
  type ToolSummary,
} from "./lib/wield";
import { ArgForm } from "./palette/ArgForm";
import { ResultView } from "./palette/ResultView";
import { RunningView } from "./palette/RunningView";
import { SearchView } from "./palette/SearchView";
import { useKeyboardNav } from "./useKeyboardNav";

type CompletedOutcome = Exclude<ToolOutcome, "Cancelled">;

type View =
  | { kind: "search" }
  | { kind: "form"; tool: ToolSummary; values: Record<string, unknown>; viaRunAgain: boolean }
  | {
      kind: "running";
      tool: ToolSummary;
      values: Record<string, unknown>;
      runId: RunId | null;
      progress: Progress | null;
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
    case "RUN":
      return {
        ...state,
        view: {
          kind: "running",
          tool: action.tool,
          values: action.values,
          runId: action.runId,
          progress: null,
          viaRunAgain: action.viaRunAgain,
        },
      };
    case "PROGRESS":
      return state.view.kind === "running"
        ? { ...state, view: { ...state.view, progress: action.progress } }
        : state;
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
      if (state.view.kind === "form") return { ...state, view: { kind: "search" } };
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
}

function SearchController({
  state,
  onQueryChange,
  onSelectIndex,
  onActivate,
  onTopLevelEscape,
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
    const onBlur = () => void hidePalette();
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
    if (state.view.kind !== "running" && state.view.kind !== "result") return;
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
      />
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
    content = <RunningView toolTitle={state.view.tool.title} progress={state.view.progress} />;
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

  return <main className="app-shell">{content}</main>;
}

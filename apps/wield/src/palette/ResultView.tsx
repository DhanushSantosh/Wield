import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { ToolOutcome, ValueKind } from "../lib/wield";
import { FailedResult } from "./outcomes/FailedResult";
import { FileResult } from "./outcomes/FileResult";
import { ReportResult } from "./outcomes/ReportResult";
import { UnavailableResult } from "./outcomes/UnavailableResult";
import { ValueResult } from "./outcomes/ValueResult";
import "./ResultView.css";

export interface ResultViewProps {
  outcome: ToolOutcome;
  onRunAgain: () => void;
  onRetry: () => void;
}

function primaryValue(kind: ValueKind, data: string): string {
  if (kind !== "Color") return data;
  try {
    const parsed: unknown = JSON.parse(data);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "hex" in parsed &&
      typeof parsed.hex === "string"
    ) {
      return parsed.hex;
    }
  } catch {
    // Copy the raw payload when a backend value cannot be decoded.
  }
  return data;
}

export function ResultView({ outcome, onRunAgain, onRetry }: ResultViewProps) {
  if (typeof outcome === "string") {
    return null;
  }

  let content;
  if ("Value" in outcome) {
    const { kind, data } = outcome.Value;
    content = (
      <ValueResult
        kind={kind}
        data={data}
        onCopy={() => void navigator.clipboard.writeText(primaryValue(kind, data))}
      />
    );
  } else if ("File" in outcome) {
    content = (
      <FileResult
        path={outcome.File.path}
        onOpenFolder={() => void revealItemInDir(outcome.File.path)}
        onRunAgain={onRunAgain}
      />
    );
  } else if ("Report" in outcome) {
    content = <ReportResult title={outcome.Report.title} lines={outcome.Report.lines} />;
  } else if ("Unavailable" in outcome) {
    content = (
      <UnavailableResult reason={outcome.Unavailable.reason} fix={outcome.Unavailable.fix} />
    );
  } else {
    content = (
      <FailedResult
        stage={outcome.Failed.stage}
        detail={outcome.Failed.detail}
        hint={outcome.Failed.hint}
        onRetry={onRetry}
        onCopyDetails={() => void navigator.clipboard.writeText(outcome.Failed.detail)}
      />
    );
  }

  return (
    <section className="result-view" aria-live="polite" aria-label="Tool result">
      {content}
    </section>
  );
}

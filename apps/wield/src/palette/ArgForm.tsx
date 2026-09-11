import { useState, type FormEvent, type KeyboardEvent } from "react";
import type { ToolSummary } from "../lib/wield";
import { isVisible, literalValue } from "./argVisibility";
import { FormField } from "./FormField";
import "./ArgForm.css";

export interface ArgFormProps {
  tool: ToolSummary;
  initialValues: Record<string, unknown>;
  onSubmit: (values: Record<string, unknown>) => void;
  onEscape: () => void;
}

function initialFormValues(tool: ToolSummary, initialValues: Record<string, unknown>) {
  const defaults = Object.fromEntries(
    tool.args.flatMap((spec) =>
      spec.default === null ? [] : [[spec.name, literalValue(spec.default)]],
    ),
  );
  return { ...defaults, ...initialValues };
}

export function ArgForm({ tool, initialValues, onSubmit, onEscape }: ArgFormProps) {
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    initialFormValues(tool, initialValues),
  );
  const visible = tool.args.filter((spec) => isVisible(spec, values, tool.args));

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    onSubmit(values);
  };
  const handleKeyDown = (event: KeyboardEvent<HTMLFormElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onEscape();
    }
  };

  return (
    <form className="arg-form" onSubmit={submit} onKeyDown={handleKeyDown}>
      <header className="arg-form__header">
        <button type="button" className="arg-form__back" onClick={onEscape} aria-label="Back">
          ←
        </button>
        <h1>{tool.title}</h1>
      </header>
      <div className="arg-form__fields">
        {visible.map((spec) => (
          <div className="arg-form__field" key={spec.name}>
            {spec.arg_type === "Bool" ? null : (
              <label className="field-name" htmlFor={`field-${spec.name}`}>
                {spec.label}
              </label>
            )}
            <FormField
              spec={spec}
              value={values[spec.name]}
              onChange={(value) =>
                setValues((current) => ({ ...current, [spec.name]: value }))
              }
            />
            {spec.help ? <div className="field-help">{spec.help}</div> : null}
          </div>
        ))}
      </div>
      <footer className="arg-form__footer">
        <span>esc to go back</span>
        <button className="primary-action" type="submit">
          {tool.title}
        </button>
      </footer>
    </form>
  );
}

import { open } from "@tauri-apps/plugin-dialog";
import type { ArgSpec } from "../lib/wield";
import "./FormField.css";

export interface FormFieldProps {
  spec: ArgSpec;
  value: unknown;
  onChange: (value: unknown) => void;
}

function selectedFileLabel(value: unknown, multiple: boolean): string {
  if (multiple && Array.isArray(value)) {
    return value.length === 1 ? "1 file selected" : `${value.length} files selected`;
  }
  if (typeof value === "string" && value.length > 0) {
    return value.split(/[\\/]/).at(-1) ?? value;
  }
  return multiple ? "Choose files…" : "Choose a file…";
}

export function FormField({ spec, value, onChange }: FormFieldProps) {
  if (typeof spec.arg_type === "object" && "File" in spec.arg_type) {
    const { filters, multiple } = spec.arg_type.File;
    const chooseFile = async () => {
      const selection = await open({
        multiple,
        filters: filters.map((filter) => ({
          name: filter.label,
          extensions: filter.extensions,
        })),
      });
      if (selection !== null) {
        onChange(selection);
      }
    };
    return (
      <button
        id={`field-${spec.name}`}
        type="button"
        className="field-picker"
        onClick={chooseFile}
      >
        {selectedFileLabel(value, multiple)}
      </button>
    );
  }

  if (spec.arg_type === "Dir") {
    const chooseFolder = async () => {
      const selection = await open({ directory: true });
      if (selection !== null) {
        onChange(selection);
      }
    };
    return (
      <button
        id={`field-${spec.name}`}
        type="button"
        className="field-picker"
        onClick={chooseFolder}
      >
        {typeof value === "string" && value.length > 0 ? value : "Choose a folder…"}
      </button>
    );
  }

  if (spec.arg_type === "Str") {
    return (
      <input
        id={`field-${spec.name}`}
        className="field-control"
        type="text"
        value={typeof value === "string" ? value : ""}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }

  if (spec.arg_type === "Text") {
    return (
      <textarea
        id={`field-${spec.name}`}
        className="field-control field-control--multiline"
        value={typeof value === "string" ? value : ""}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }

  if (typeof spec.arg_type === "object" && "Int" in spec.arg_type) {
    const { range, step } = spec.arg_type.Int;
    return (
      <input
        id={`field-${spec.name}`}
        className="field-control"
        type="number"
        min={range?.[0]}
        max={range?.[1]}
        step={step ?? 1}
        value={typeof value === "number" ? value : ""}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    );
  }

  if (typeof spec.arg_type === "object" && "Float" in spec.arg_type) {
    const { range } = spec.arg_type.Float;
    return (
      <input
        id={`field-${spec.name}`}
        className="field-control"
        type="number"
        min={range?.[0]}
        max={range?.[1]}
        step="any"
        value={typeof value === "number" ? value : ""}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    );
  }

  if (spec.arg_type === "Bool") {
    return (
      <label className="field-checkbox">
        <input
          id={`field-${spec.name}`}
          type="checkbox"
          checked={value === true}
          onChange={(event) => onChange(event.target.checked)}
        />
        <span>{spec.label}</span>
      </label>
    );
  }

  if (typeof spec.arg_type === "object" && "Enum" in spec.arg_type) {
    return (
      <select
        id={`field-${spec.name}`}
        className="field-control"
        value={typeof value === "string" ? value : ""}
        onChange={(event) => onChange(event.target.value)}
      >
        {spec.arg_type.Enum.options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    );
  }

  return null;
}

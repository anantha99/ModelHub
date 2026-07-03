import { useId } from "react";

type SwitchFieldProps = {
  checked: boolean;
  description: string;
  disabled?: boolean;
  label: string;
  name: string;
  onChange: (checked: boolean) => void;
};

export function SwitchField({
  checked,
  description,
  disabled = false,
  label,
  name,
  onChange,
}: SwitchFieldProps) {
  const fieldId = useId();
  const labelId = `${fieldId}-label`;
  const descriptionId = `${fieldId}-description`;

  return (
    <label className="switch-field" data-disabled={disabled || undefined}>
      <span className="switch-field-copy">
        <span className="switch-field-label" id={labelId}>
          {label}
        </span>
        <span className="switch-field-description" id={descriptionId}>
          {description}
        </span>
      </span>
      <input
        aria-describedby={descriptionId}
        aria-labelledby={labelId}
        autoComplete="off"
        checked={checked}
        className="switch-field-control"
        disabled={disabled}
        name={name}
        onChange={(event) => onChange(event.target.checked)}
        role="switch"
        type="checkbox"
      />
    </label>
  );
}

import { positionIconUrl } from "../lib/riotAssets";
import "./RolePicker.css";

export function RoleIcon({ role }: { role: string }) {
  const url = positionIconUrl(role);
  if (!url) {
    return <span className="role-icon-fallback">ALL</span>;
  }
  return <img src={url} alt={role} className="role-icon-img" />;
}

export function RolePicker({
  options,
  value,
  onChange,
}: {
  options: string[];
  value: string;
  onChange: (role: string) => void;
}) {
  return (
    <div className="role-picker">
      {options.map((p) => (
        <button
          key={p}
          type="button"
          title={p.toUpperCase()}
          className={`role-icon-btn${value === p ? " active" : ""}`}
          onClick={() => onChange(p)}
        >
          <RoleIcon role={p} />
        </button>
      ))}
    </div>
  );
}

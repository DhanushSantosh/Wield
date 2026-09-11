import type { CapabilitiesReport } from "../lib/wield";
import "./preferences.css";

export interface SystemStatusSectionProps {
  report: CapabilitiesReport | null;
}

export function SystemStatusSection({ report }: SystemStatusSectionProps) {
  if (report === null) {
    return (
      <section className="prefs-section">
        <h2>System status</h2>
        <p className="prefs-muted">Checking…</p>
      </section>
    );
  }

  const binaries = Object.entries(report.binaries);
  const portals = Object.entries(report.portals);

  return (
    <section className="prefs-section">
      <h2>System status</h2>

      <div className="prefs-subsection">
        <h3>Binaries</h3>
        {binaries.length === 0 ? (
          <p className="prefs-muted">No binaries checked yet.</p>
        ) : (
          <table className="prefs-table">
            <tbody>
              {binaries.map(([name, found]) => (
                <tr key={name}>
                  <td className="prefs-mono">{name}</td>
                  <td className={found ? "prefs-status-ok" : "prefs-status-bad"}>
                    {found ? "Found" : "Not found"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="prefs-subsection">
        <h3>Portals</h3>
        {portals.length === 0 ? (
          <p className="prefs-muted">No portals detected.</p>
        ) : (
          <table className="prefs-table">
            <tbody>
              {portals.map(([name, version]) => (
                <tr key={name}>
                  <td>{name}</td>
                  <td className="prefs-mono">v{version}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="prefs-subsection">
        <h3>Tools</h3>
        <table className="prefs-table">
          <tbody>
            {report.tools.map((tool) => (
              <tr key={tool.id}>
                <td>{tool.title}</td>
                <td className={tool.available ? "prefs-status-ok" : "prefs-status-bad"}>
                  {tool.available ? "Available" : tool.reason}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

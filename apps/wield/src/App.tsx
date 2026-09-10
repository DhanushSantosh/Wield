import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AppInfo {
  name: string;
  version: string;
}

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info")
      .then(setInfo)
      .catch(() => setInfo(null));
  }, []);

  return (
    <main className="app-shell">
      <h1>{info ? info.name : "Wield"}</h1>
      <p>Portal-native Linux utility hub.</p>
      {info && <p className="version">v{info.version}</p>}
    </main>
  );
}

# Wield

Wield is a portal-native Linux utility hub — a global-hotkey command palette and
tray that front a suite of quick actions (screen capture tools) and file/data
converters. It is built on XDG Desktop Portals so it works across desktop
environments without per-DE code.

- `apps/wield`: Tauri + Vite + React desktop shell.
- `crates/wield-core`: descriptor model, executor, and tool registry.
- `crates/wield-portal`: XDG Desktop Portal access and capability probe.
- `crates/wield-tools`: built-in tool descriptors and native tools.
- `crates/wield-cli`: the `wield` command-line surface.

## Development

    npm install
    npm run check          # lint + typecheck + tests (app + cargo)
    npm run dev:app        # run the Tauri shell

## License

MIT License. See [LICENSE](LICENSE).

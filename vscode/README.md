# qualirs for VS Code

Runs the bundled `qualirs` analyzer for Rust files and publishes results as VS Code diagnostics in the Problems panel, with a status bar summary and a `qualirs` output channel.

## Usage

Open a Rust file and run `qualirs: Check Active Rust File`, or save the file when `qualirs.runOnSave` is enabled. Findings appear in the Problems panel.

Open a Rust repository with a `Cargo.toml` and qualirs checks the workspace automatically when `qualirs.runOnWorkspaceOpen` is enabled.

Create a project config with `qualirs: Create qualirs.toml`. The extension resolves `qualirs.toml` from the workspace root by default and passes it to the analyzer for single-file checks.

Pause analysis with `qualirs: Pause`, resume it with `qualirs: Resume`, or switch state with `qualirs: Toggle Pause`.

## Packaging

From this `vscode` directory:

```bash
npm install
npm run package:vsix
```

The `package:vsix` script builds `qualirs` in release mode and embeds the current platform binary under `bin/<platform>-<arch>/`, so the VSIX can be installed without requiring a separate qualirs install on the same platform.

## Settings

- `qualirs.executablePath`: use a custom `qualirs` binary instead of the bundled one.
- `qualirs.configPath`: use a specific `qualirs.toml`.
- `qualirs.runOnOpen`: check Rust files when opened.
- `qualirs.runOnSave`: check Rust files when saved.
- `qualirs.runOnWorkspaceOpen`: check the Rust workspace when opened.
- `qualirs.paused`: pause automatic and manual checks.
- `qualirs.minSeverity`: override the configured minimum severity.
- `qualirs.precision`: choose `conservative`, `balanced`, or `exploratory` finding precision.
- `qualirs.category`: check only one qualirs category.
- `qualirs.threads`: set analyzer threads.

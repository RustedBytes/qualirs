import * as cp from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

type Severity = "critical" | "warning" | "info";
type StatusState = "checking" | "error" | "paused" | undefined;

interface QualirsConfig {
    executablePath: string;
    configPath: string;
    runOnOpen: boolean;
    runOnSave: boolean;
    runOnWorkspaceOpen: boolean;
    paused: boolean;
    minSeverity: string;
    precision: string;
    category: string;
    threads: number;
    statusBarItem: boolean;
}

interface QualirsReport {
    summary?: {
        files_analyzed?: number;
        findings?: number;
        severity_counts?: {
            critical?: number;
            warning?: number;
            info?: number;
        };
    };
    smells?: QualirsFinding[];
    parse_errors?: Array<{ message?: string }>;
}

interface QualirsFinding {
    code?: string;
    severity?: Severity;
    confidence?: "low" | "medium" | "high";
    category?: string;
    name?: string;
    location?: {
        file?: string;
        line_start?: number;
        line_end?: number;
        column?: number | null;
    };
    message?: string;
    suggestion?: string;
}

interface RunResult {
    report: QualirsReport;
    stdout: string;
    stderr: string;
    exitCode: number | null;
}

const diagnosticSource = "qualirs";
const debounceDelayMs = 250;
const diagnosticCollection = vscode.languages.createDiagnosticCollection("qualirs");
let statusBarItem: vscode.StatusBarItem;
let outputChannel: vscode.OutputChannel;
const timers = new Map<string, NodeJS.Timeout>();
let runSequence = 0;

export function activate(context: vscode.ExtensionContext): void {
    outputChannel = vscode.window.createOutputChannel("qualirs", "log");
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 20);
    statusBarItem.command = "qualirs.showOutput";
    statusBarItem.name = "qualirs";
    context.subscriptions.push(outputChannel, statusBarItem, diagnosticCollection);

    context.subscriptions.push(
        vscode.commands.registerCommand("qualirs.checkActiveFile", () => checkActiveFile(context, true)),
        vscode.commands.registerCommand("qualirs.checkWorkspace", () => checkWorkspace(context, true)),
        vscode.commands.registerCommand("qualirs.initConfig", () => initConfig(context)),
        vscode.commands.registerCommand("qualirs.showOutput", () => outputChannel.show()),
        vscode.commands.registerCommand("qualirs.pause", () => setPaused(true)),
        vscode.commands.registerCommand("qualirs.resume", async () => {
            await setPaused(false);
            await checkActiveFile(context, false);
        }),
        vscode.commands.registerCommand("qualirs.togglePause", async () => {
            const shouldPause = !readConfig().paused;
            await setPaused(shouldPause);
            if (!shouldPause) {
                await checkActiveFile(context, false);
            }
        }),
        vscode.workspace.onDidChangeWorkspaceFolders(() => {
            runOnOpenedRustRepository(context);
        }),
        vscode.workspace.onDidSaveTextDocument((document) => {
            const config = readConfig();
            if (!config.paused && isRustDocument(document) && config.runOnSave) {
                scheduleCheck(context, document);
            }
        }),
        vscode.workspace.onDidOpenTextDocument((document) => {
            const config = readConfig();
            if (!config.paused && isRustDocument(document) && config.runOnOpen) {
                scheduleCheck(context, document);
            }
        }),
        vscode.window.onDidChangeActiveTextEditor((editor) => {
            if (readConfig().paused) {
                updateStatusText("paused");
                return;
            }
            if (editor && isRustDocument(editor.document)) {
                if (isWorkspaceDocument(editor.document)) {
                    updateStatusFromDiagnostics(editor.document.uri);
                } else {
                    updateStatusText(undefined);
                }
            } else {
                updateStatusText(undefined);
            }
        }),
        vscode.workspace.onDidChangeConfiguration((event) => {
            if (event.affectsConfiguration("qualirs")) {
                updateStatusVisibility();
                if (readConfig().paused) {
                    clearScheduledChecks();
                    diagnosticCollection.clear();
                    updateStatusText("paused");
                    return;
                }
                void checkActiveFile(context, false);
            }
        })
    );

    updateStatusVisibility();
    if (readConfig().paused) {
        updateStatusText("paused");
    } else if (runOnOpenedRustRepository(context)) {
        return;
    } else if (vscode.window.activeTextEditor && isRustDocument(vscode.window.activeTextEditor.document)) {
        void checkActiveFile(context, false);
    } else {
        updateStatusText(undefined);
    }
}

export function deactivate(): void {
    clearScheduledChecks();
    diagnosticCollection.dispose();
}

async function checkActiveFile(context: vscode.ExtensionContext, revealOutputOnError: boolean): Promise<void> {
    if (readConfig().paused) {
        updateStatusText("paused");
        if (revealOutputOnError) {
            void vscode.window.showInformationMessage("qualirs is paused. Run `qualirs: Resume` to enable analysis.");
        }
        return;
    }

    const editor = vscode.window.activeTextEditor;
    if (!editor || !isRustDocument(editor.document)) {
        updateStatusText(undefined);
        return;
    }

    if (!isWorkspaceDocument(editor.document)) {
        diagnosticCollection.delete(editor.document.uri);
        updateStatusText(undefined);
        if (revealOutputOnError) {
            void vscode.window.showWarningMessage("qualirs only checks Rust files inside the opened workspace.");
        }
        return;
    }

    await checkDocument(context, editor.document, revealOutputOnError);
}

async function checkWorkspace(context: vscode.ExtensionContext, revealOutputOnError: boolean): Promise<void> {
    if (readConfig().paused) {
        updateStatusText("paused");
        if (revealOutputOnError) {
            void vscode.window.showInformationMessage("qualirs is paused. Run `qualirs: Resume` to enable analysis.");
        }
        return;
    }

    const workspaceFolder = findRustWorkspaceFolder() ?? vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) {
        void vscode.window.showWarningMessage("qualirs needs an open workspace to check the workspace.");
        return;
    }

    await runQualirs(context, workspaceFolder.uri.fsPath, workspaceFolder, revealOutputOnError);
}

function runOnOpenedRustRepository(context: vscode.ExtensionContext): boolean {
    const config = readConfig();
    if (config.paused || !config.runOnWorkspaceOpen) {
        return false;
    }

    const workspaceFolder = findRustWorkspaceFolder();
    if (!workspaceFolder) {
        return false;
    }

    void runQualirs(context, workspaceFolder.uri.fsPath, workspaceFolder, false);
    return true;
}

async function checkDocument(
    context: vscode.ExtensionContext,
    document: vscode.TextDocument,
    revealOutputOnError: boolean
): Promise<void> {
    if (readConfig().paused) {
        updateStatusText("paused");
        return;
    }

    if (document.isUntitled) {
        return;
    }

    const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
    if (!workspaceFolder) {
        diagnosticCollection.delete(document.uri);
        updateStatusText(undefined);
        return;
    }

    if (document.isDirty) {
        await document.save();
    }

    await runQualirs(context, document.uri.fsPath, workspaceFolder, revealOutputOnError, document.uri);
}

function scheduleCheck(context: vscode.ExtensionContext, document: vscode.TextDocument): void {
    if (readConfig().paused) {
        updateStatusText("paused");
        return;
    }

    if (!isWorkspaceDocument(document)) {
        diagnosticCollection.delete(document.uri);
        return;
    }

    const key = document.uri.toString();
    const existing = timers.get(key);
    if (existing) {
        clearTimeout(existing);
    }

    timers.set(
        key,
        setTimeout(() => {
            timers.delete(key);
            void checkDocument(context, document, false);
        }, debounceDelayMs)
    );
}

async function runQualirs(
    context: vscode.ExtensionContext,
    targetPath: string,
    workspaceFolder: vscode.WorkspaceFolder | undefined,
    revealOutputOnError: boolean,
    focusedDocument?: vscode.Uri
): Promise<void> {
    if (readConfig().paused) {
        updateStatusText("paused");
        return;
    }

    const runId = ++runSequence;
    updateStatusText("checking");
    const executable = resolveExecutable(context);
    if (!executable) {
        const message = "qualirs executable was not found. Set qualirs.executablePath or package the extension with npm run package:vsix.";
        outputChannel.appendLine(message);
        updateStatusText("error");
        if (revealOutputOnError) {
            outputChannel.show(true);
        }
        void vscode.window.showErrorMessage(message);
        return;
    }

    const cwd = workspaceFolder?.uri.fsPath ?? path.dirname(targetPath);
    const args = buildArgs(targetPath, workspaceFolder);
    outputChannel.appendLine(`Running: ${quote(executable)} ${args.map(quote).join(" ")}`);

    try {
        const result = await execute(executable, args, cwd);
        if (runId !== runSequence) {
            outputChannel.appendLine("Ignored stale qualirs result.");
            return;
        }

        outputChannel.appendLine(`Exit code: ${result.exitCode ?? "signal"}`);
        if (result.stderr.trim()) {
            outputChannel.appendLine(result.stderr.trimEnd());
        }
        appendParseErrors(result.report);

        if (readConfig().paused) {
            diagnosticCollection.clear();
            updateStatusText("paused");
            return;
        }

        applyReport(result.report, focusedDocument, workspaceFolder);
        updateStatusFromReport(result.report);
    } catch (error) {
        if (runId !== runSequence) {
            return;
        }

        const message = error instanceof Error ? error.message : String(error);
        outputChannel.appendLine(message);
        updateStatusText("error");
        if (focusedDocument) {
            diagnosticCollection.delete(focusedDocument);
        }
        if (revealOutputOnError) {
            outputChannel.show(true);
        }
        void vscode.window.showErrorMessage(`qualirs failed: ${message}`);
    }
}

function buildArgs(targetPath: string, workspaceFolder: vscode.WorkspaceFolder | undefined): string[] {
    const config = readConfig();
    const args = ["--format", "json"];
    const configPath = resolveConfigPath(config.configPath, targetPath, workspaceFolder);

    if (configPath) {
        args.push("--config", configPath);
    }
    if (config.minSeverity) {
        args.push("--min-severity", config.minSeverity);
    }
    if (config.precision) {
        args.push("--precision", config.precision);
    }
    if (config.category) {
        args.push("--category", config.category);
    }
    if (config.threads > 0) {
        args.push("--threads", String(config.threads));
    }

    args.push(targetPath);
    return args;
}

function resolveExecutable(context: vscode.ExtensionContext): string | undefined {
    const configured = readConfig().executablePath.trim();
    if (configured) {
        const expanded = expandHome(configured);
        return fs.existsSync(expanded) ? expanded : configured;
    }

    const executableName = process.platform === "win32" ? "qualirs.exe" : "qualirs";
    const bundled = path.join(context.extensionPath, "bin", `${process.platform}-${process.arch}`, executableName);
    if (fs.existsSync(bundled)) {
        return bundled;
    }

    return undefined;
}

function resolveConfigPath(
    configuredPath: string,
    targetPath: string,
    workspaceFolder: vscode.WorkspaceFolder | undefined
): string | undefined {
    const configured = configuredPath.trim();
    if (configured) {
        const expanded = expandHome(configured);
        const absolute = path.isAbsolute(expanded)
            ? expanded
            : path.join(workspaceFolder?.uri.fsPath ?? path.dirname(targetPath), expanded);
        return fs.existsSync(absolute) ? absolute : absolute;
    }

    const found = findUp("qualirs.toml", path.dirname(targetPath), workspaceFolder?.uri.fsPath);
    if (found) {
        return found;
    }

    const workspaceConfig = workspaceFolder ? path.join(workspaceFolder.uri.fsPath, "qualirs.toml") : undefined;
    return workspaceConfig && fs.existsSync(workspaceConfig) ? workspaceConfig : undefined;
}

function findUp(fileName: string, startDir: string, stopDir: string | undefined): string | undefined {
    let current = path.resolve(startDir);
    const stop = stopDir ? path.resolve(stopDir) : path.parse(current).root;

    while (true) {
        const candidate = path.join(current, fileName);
        if (fs.existsSync(candidate)) {
            return candidate;
        }
        if (current === stop || current === path.dirname(current)) {
            return undefined;
        }
        current = path.dirname(current);
    }
}

function execute(executable: string, args: string[], cwd: string): Promise<RunResult> {
    return new Promise((resolve, reject) => {
        const child = cp.spawn(executable, args, {
            cwd,
            windowsHide: true
        });

        let stdout = "";
        let stderr = "";

        child.stdout.setEncoding("utf8");
        child.stderr.setEncoding("utf8");
        child.stdout.on("data", (chunk: string) => {
            stdout += chunk;
        });
        child.stderr.on("data", (chunk: string) => {
            stderr += chunk;
        });
        child.on("error", reject);
        child.on("close", (exitCode) => {
            const jsonText = stdout.trim();
            if (!jsonText) {
                reject(new Error(stderr.trim() || `qualirs exited with code ${exitCode}`));
                return;
            }

            try {
                const report = parseReport(jsonText);
                resolve({ report, stdout, stderr, exitCode });
            } catch (error) {
                const reason = error instanceof Error ? error.message : String(error);
                reject(new Error(`could not parse qualirs JSON output: ${reason}\n${stderr.trim()}`));
            }
        });
    });
}

function parseReport(jsonText: string): QualirsReport {
    const value: unknown = JSON.parse(jsonText);
    if (!isObject(value)) {
        throw new Error("top-level JSON value is not an object");
    }

    return value as QualirsReport;
}

function appendParseErrors(report: QualirsReport): void {
    const parseErrors = report.parse_errors ?? [];
    if (parseErrors.length === 0) {
        return;
    }

    outputChannel.appendLine(`qualirs reported ${parseErrors.length} parse error(s):`);
    for (const parseError of parseErrors) {
        outputChannel.appendLine(`- ${parseError.message ?? "unknown parse error"}`);
    }
}

function applyReport(
    report: QualirsReport,
    focusedDocument: vscode.Uri | undefined,
    workspaceFolder: vscode.WorkspaceFolder | undefined
): void {
    const diagnosticsByFile = new Map<string, vscode.Diagnostic[]>();

    for (const finding of report.smells ?? []) {
        const uri = uriForFinding(finding, workspaceFolder);
        if (!uri) {
            continue;
        }

        const diagnostic = diagnosticForFinding(finding);
        const key = uri.toString();
        diagnosticsByFile.set(key, [...(diagnosticsByFile.get(key) ?? []), diagnostic]);
    }

    if (focusedDocument) {
        diagnosticCollection.set(focusedDocument, diagnosticsByFile.get(focusedDocument.toString()) ?? []);
        return;
    }

    diagnosticCollection.clear();
    for (const [uriString, diagnostics] of diagnosticsByFile) {
        diagnosticCollection.set(vscode.Uri.parse(uriString), diagnostics);
    }
}

function diagnosticForFinding(finding: QualirsFinding): vscode.Diagnostic {
    const lineStart = Math.max((finding.location?.line_start ?? 1) - 1, 0);
    const lineEnd = Math.max((finding.location?.line_end ?? finding.location?.line_start ?? 1) - 1, lineStart);
    const column = Math.max((finding.location?.column ?? 1) - 1, 0);
    const range = new vscode.Range(lineStart, column, lineEnd, column + 1);
    const code = finding.code ?? "Q0000";
    const name = finding.name ?? "qualirs finding";
    const confidence = confidenceSuffix(finding);
    const message = finding.suggestion
        ? `${name}${confidence}: ${finding.message ?? ""}\n${finding.suggestion}`
        : `${name}${confidence}: ${finding.message ?? ""}`;
    const diagnostic = new vscode.Diagnostic(range, message.trim(), vscodeSeverity(finding.severity));
    diagnostic.source = diagnosticSource;
    diagnostic.code = code;
    return diagnostic;
}

function confidenceSuffix(finding: QualirsFinding): string {
    const precision = readConfig().precision;
    if (!precision || precision === "conservative" || !finding.confidence) {
        return "";
    }

    return ` [${finding.confidence} confidence]`;
}

function uriForFinding(finding: QualirsFinding, workspaceFolder: vscode.WorkspaceFolder | undefined): vscode.Uri | undefined {
    const file = finding.location?.file;
    if (!file || !workspaceFolder) {
        return undefined;
    }

    const absolute = path.isAbsolute(file)
        ? path.resolve(file)
        : path.resolve(workspaceFolder.uri.fsPath, file);
    if (!isPathInsideOrEqual(absolute, workspaceFolder.uri.fsPath)) {
        return undefined;
    }

    return vscode.Uri.file(absolute);
}

function vscodeSeverity(severity: Severity | undefined): vscode.DiagnosticSeverity {
    switch (severity) {
        case "critical":
            return vscode.DiagnosticSeverity.Error;
        case "warning":
            return vscode.DiagnosticSeverity.Warning;
        case "info":
        default:
            return vscode.DiagnosticSeverity.Information;
    }
}

async function initConfig(context: vscode.ExtensionContext): Promise<void> {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) {
        void vscode.window.showWarningMessage("Open a workspace before creating qualirs.toml.");
        return;
    }

    const target = path.join(workspaceFolder.uri.fsPath, "qualirs.toml");
    if (fs.existsSync(target)) {
        void vscode.window.showInformationMessage("qualirs.toml already exists in this workspace.");
        return;
    }

    const executable = resolveExecutable(context);
    if (executable) {
        try {
            await executeConfigInit(executable, target, workspaceFolder.uri.fsPath);
            void vscode.window.showInformationMessage("Created qualirs.toml.");
            const document = await vscode.workspace.openTextDocument(vscode.Uri.file(target));
            await vscode.window.showTextDocument(document);
            return;
        } catch (error) {
            outputChannel.appendLine(`qualirs init-config failed, using bundled template: ${String(error)}`);
        }
    }

    const template = path.join(context.extensionPath, "qualirs.toml");
    fs.copyFileSync(template, target);
    void vscode.window.showInformationMessage("Created qualirs.toml.");
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(target));
    await vscode.window.showTextDocument(document);
}

function executeConfigInit(executable: string, target: string, cwd: string): Promise<void> {
    return new Promise((resolve, reject) => {
        const child = cp.spawn(executable, ["init-config", "--output", target], {
            cwd,
            windowsHide: true
        });
        let stderr = "";
        child.stderr.setEncoding("utf8");
        child.stderr.on("data", (chunk: string) => {
            stderr += chunk;
        });
        child.on("error", reject);
        child.on("close", (exitCode) => {
            if (exitCode === 0) {
                resolve();
            } else {
                reject(new Error(stderr.trim() || `qualirs init-config exited with code ${exitCode}`));
            }
        });
    });
}

function updateStatusFromReport(report: QualirsReport): void {
    if (readConfig().paused) {
        updateStatusText("paused");
        return;
    }

    const counts = report.summary?.severity_counts;
    const critical = counts?.critical ?? 0;
    const warning = counts?.warning ?? 0;
    const info = counts?.info ?? 0;
    const total = report.summary?.findings ?? critical + warning + info;

    statusBarItem.text = total === 0 ? "$(check) qualirs" : `$(warning) qualirs ${total}`;
    statusBarItem.tooltip = `qualirs: ${critical} critical, ${warning} warning, ${info} info`;
}

function updateStatusFromDiagnostics(uri: vscode.Uri): void {
    if (readConfig().paused) {
        updateStatusText("paused");
        return;
    }

    const diagnostics = diagnosticCollection.get(uri) ?? [];
    const critical = diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error).length;
    const warning = diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Warning).length;
    const info = diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Information).length;
    updateStatusFromReport({
        summary: {
            findings: diagnostics.length,
            severity_counts: { critical, warning, info }
        }
    });
}

function updateStatusText(state: StatusState): void {
    if (state === "checking") {
        statusBarItem.text = "$(sync~spin) qualirs";
        statusBarItem.tooltip = "qualirs is checking this Rust file.";
    } else if (state === "error") {
        statusBarItem.text = "$(error) qualirs";
        statusBarItem.tooltip = "qualirs failed. Open the qualirs output channel for details.";
    } else if (state === "paused") {
        statusBarItem.text = "$(debug-pause) qualirs";
        statusBarItem.tooltip = "qualirs is paused.";
    } else {
        statusBarItem.text = "$(search) qualirs";
        statusBarItem.tooltip = "qualirs is ready.";
    }
}

function updateStatusVisibility(): void {
    if (readConfig().statusBarItem) {
        statusBarItem.show();
    } else {
        statusBarItem.hide();
    }
}

function isRustDocument(document: vscode.TextDocument): boolean {
    return document.languageId === "rust" && document.uri.scheme === "file";
}

function findRustWorkspaceFolder(): vscode.WorkspaceFolder | undefined {
    return vscode.workspace.workspaceFolders?.find(isRustWorkspaceFolder);
}

function isRustWorkspaceFolder(workspaceFolder: vscode.WorkspaceFolder): boolean {
    return fs.existsSync(path.join(workspaceFolder.uri.fsPath, "Cargo.toml"));
}

async function setPaused(paused: boolean): Promise<void> {
    await getConfig().update("paused", paused, configurationTargetForPause());
    if (paused) {
        clearScheduledChecks();
        diagnosticCollection.clear();
        updateStatusText("paused");
        outputChannel.appendLine("qualirs paused.");
        void vscode.window.showInformationMessage("qualirs paused.");
    } else {
        updateStatusText(undefined);
        outputChannel.appendLine("qualirs resumed.");
        void vscode.window.showInformationMessage("qualirs resumed.");
    }
}

function configurationTargetForPause(): vscode.ConfigurationTarget {
    return vscode.workspace.workspaceFolders?.length
        ? vscode.ConfigurationTarget.Workspace
        : vscode.ConfigurationTarget.Global;
}

function clearScheduledChecks(): void {
    for (const timer of timers.values()) {
        clearTimeout(timer);
    }
    timers.clear();
}

function isWorkspaceDocument(document: vscode.TextDocument): boolean {
    return vscode.workspace.getWorkspaceFolder(document.uri) !== undefined;
}

function isPathInsideOrEqual(candidatePath: string, rootPath: string): boolean {
    const candidate = normalizePathForComparison(candidatePath);
    const root = normalizePathForComparison(rootPath);
    const relative = path.relative(root, candidate);
    return relative === "" || (!!relative && !relative.startsWith("..") && !path.isAbsolute(relative));
}

function normalizePathForComparison(value: string): string {
    const normalized = path.resolve(value);
    return process.platform === "win32" ? normalized.toLowerCase() : normalized;
}

function getConfig(): vscode.WorkspaceConfiguration {
    return vscode.workspace.getConfiguration("qualirs");
}

function readConfig(): QualirsConfig {
    const config = getConfig();
    return {
        executablePath: config.get<string>("executablePath", ""),
        configPath: config.get<string>("configPath", ""),
        runOnOpen: config.get<boolean>("runOnOpen", true),
        runOnSave: config.get<boolean>("runOnSave", true),
        runOnWorkspaceOpen: config.get<boolean>("runOnWorkspaceOpen", true),
        paused: config.get<boolean>("paused", false),
        minSeverity: config.get<string>("minSeverity", ""),
        precision: config.get<string>("precision", "conservative"),
        category: config.get<string>("category", ""),
        threads: Math.max(config.get<number>("threads", 0), 0),
        statusBarItem: config.get<boolean>("statusBarItem", true)
    };
}

function expandHome(value: string): string {
    if (value === "~") {
        return os.homedir();
    }
    if (value.startsWith(`~${path.sep}`)) {
        return path.join(os.homedir(), value.slice(2));
    }
    return value;
}

function quote(value: string): string {
    return /\s/.test(value) ? `"${value.replace(/"/g, '\\"')}"` : value;
}

function isObject(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}

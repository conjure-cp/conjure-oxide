"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
const fs = require("fs");
const path = require("path");
const vscode_1 = require("vscode");
const node_1 = require("vscode-languageclient/node");
const SERVER_CONFIG_SECTION = 'conjureOxide';
const SERVER_PATH_CONFIG_KEY = 'serverPath';
const SERVER_OUTPUT_CHANNEL = 'Conjure-Oxide Language Server';
function activate(context) {
    tryStartLanguageServer(context);
}
function tryStartLanguageServer(context) {
    //for future, possibly may want version checking
    const output = vscode_1.window.createOutputChannel(SERVER_OUTPUT_CHANNEL);
    output.appendLine('Starting language server...');
    const command = resolveServerCommand(context);
    const cwd = resolveServerCwd(context);
    let serveroptions = {
        command,
        args: ["server-lsp"],
        options: cwd ? { cwd } : undefined,
    };
    let clientoptions = {
        // Do not restrict to scheme 'file': remote workspaces use schemes like
        // 'vscode-remote', and filtering them out prevents didOpen/didChange.
        // Include both language-id and filename-pattern selectors to survive
        // differing language associations in user/release environments.
        documentSelector: [
            { language: 'essence' },
            { language: 'eprime' },
            { pattern: '**/*.essence' },
            { pattern: '**/*.eprime' },
        ]
    };
    output.appendLine(`Server command: ${command}`);
    if (cwd) {
        output.appendLine(`Server cwd: ${cwd}`);
    }
    let client = new node_1.LanguageClient("Conjure-Oxide Language Server", serveroptions, clientoptions, true);
    client.start();
    output.appendLine("Language client setup complete.");
    context.subscriptions.push(client, output);
}
function resolveServerCommand(context) {
    var _a;
    const configuredPath = vscode_1.workspace
        .getConfiguration(SERVER_CONFIG_SECTION)
        .get(SERVER_PATH_CONFIG_KEY);
    if (configuredPath && fs.existsSync(configuredPath)) {
        return configuredPath;
    }
    const binaryName = process.platform === 'win32' ? 'conjure-oxide.exe' : 'conjure-oxide';
    const workspaceCandidates = ((_a = vscode_1.workspace.workspaceFolders) !== null && _a !== void 0 ? _a : [])
        .filter((folder) => folder.uri.scheme === 'file')
        .map((folder) => path.join(folder.uri.fsPath, 'target', 'release', binaryName));
    for (const candidate of workspaceCandidates) {
        if (fs.existsSync(candidate)) {
            return candidate;
        }
    }
    const extensionCandidate = context.asAbsolutePath(path.join('target', 'release', binaryName));
    if (fs.existsSync(extensionCandidate)) {
        return extensionCandidate;
    }
    const devCandidate = path.join(__dirname, '../../../target/release', binaryName);
    if (fs.existsSync(devCandidate)) {
        return devCandidate;
    }
    return process.platform === 'win32' ? 'conjure-oxide.exe' : 'conjure-oxide';
}
function resolveServerCwd(context) {
    if (vscode_1.workspace.workspaceFolders) {
        const fileWorkspace = vscode_1.workspace.workspaceFolders.find((folder) => folder.uri.scheme === 'file');
        if (fileWorkspace) {
            return fileWorkspace.uri.fsPath;
        }
    }
    if (context.globalStorageUri.scheme === 'file') {
        const storagePath = context.globalStorageUri.fsPath;
        fs.mkdirSync(storagePath, { recursive: true });
        return storagePath;
    }
    return undefined;
}

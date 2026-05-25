import * as fs from 'fs';
import * as path from 'path';
import { ExtensionContext, workspace, window } from 'vscode';

import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions
} from 'vscode-languageclient/node';

const SERVER_CONFIG_SECTION = 'conjureOxide';
const SERVER_PATH_CONFIG_KEY = 'serverPath';
const SERVER_OUTPUT_CHANNEL = 'Conjure-Oxide Language Server';

export function activate(context: ExtensionContext) {
	tryStartLanguageServer(context);
}
function tryStartLanguageServer(context: ExtensionContext) {

	//for future, possibly may want version checking

	const output = window.createOutputChannel(SERVER_OUTPUT_CHANNEL);
	output.appendLine('Starting language server...');

	const command = resolveServerCommand(context);
	const cwd = resolveServerCwd(context);

	let serveroptions: ServerOptions = {
		command,
		args: ["server-lsp"],
		options: cwd ? { cwd } : undefined,
	}

	let clientoptions: LanguageClientOptions = {
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
	}

	output.appendLine(`Server command: ${command}`);
	if (cwd) {
		output.appendLine(`Server cwd: ${cwd}`);
	}

	let client = new LanguageClient("Conjure-Oxide Language Server", serveroptions, clientoptions, true);
	client.start();
	output.appendLine("Language client setup complete.");

	context.subscriptions.push(client, output)
}

function resolveServerCommand(context: ExtensionContext): string {
	const configuredPath = workspace
		.getConfiguration(SERVER_CONFIG_SECTION)
		.get<string>(SERVER_PATH_CONFIG_KEY);

	if (configuredPath && fs.existsSync(configuredPath)) {
		return configuredPath;
	}

	const binaryName = process.platform === 'win32' ? 'conjure-oxide.exe' : 'conjure-oxide';

	const workspaceCandidates = (workspace.workspaceFolders ?? [])
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

function resolveServerCwd(context: ExtensionContext): string | undefined {
	if (workspace.workspaceFolders) {
		const fileWorkspace = workspace.workspaceFolders.find((folder) => folder.uri.scheme === 'file');
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

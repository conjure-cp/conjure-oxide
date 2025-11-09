import * as path from 'path';
import { window } from 'vscode';
import { workspace, ExtensionContext } from 'vscode';

import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	TransportKind
} from 'vscode-languageclient/node';
import { vscode } from 'vscx'

export function activate(context: ExtensionContext) {
	tryStartLanguageServer(context);
}
function tryStartLanguageServer(context: ExtensionContext) {

	//for future, possibly may want version checking

	console.log("Before setup");
	let serveroptions: ServerOptions = {
		command: "conjure-oxide", args: ["server-lsp"]
	}

	let clientoptions: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'essence' }]
	}

	let client = new LanguageClient("Conjure-Oxide Language Server", serveroptions, clientoptions, true);
	client.start();
	console.log("Setup done");

	context.subscriptions.push(client)
}
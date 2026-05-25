import * as fs from 'fs';
import * as path from 'path';
import { ExtensionContext } from 'vscode';

import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions
} from 'vscode-languageclient/node';

export function activate(context: ExtensionContext) {
	tryStartLanguageServer(context);
}
function tryStartLanguageServer(context: ExtensionContext) {

	//for future, possibly may want version checking

	console.log("Before setup");
	const localServerPath = path.join(__dirname, '../../../target/release/conjure-oxide');
	const command = fs.existsSync(localServerPath) ? localServerPath : "conjure-oxide";

	let serveroptions: ServerOptions = {
		command,
		args: ["server-lsp"]
	}

	let clientoptions: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'essence' }]
	}

	let client = new LanguageClient("Conjure-Oxide Language Server", serveroptions, clientoptions, true);
	client.start();
	console.log("Setup done");

	context.subscriptions.push(client)
}

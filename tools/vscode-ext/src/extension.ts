import * as path from 'path';
import { window } from 'vscode';
import { workspace, ExtensionContext } from 'vscode';

import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	TransportKind
} from 'vscode-languageclient/node';

export function activate(context: ExtensionContext) {
	tryStartLanguageServer(context);
}
function tryStartLanguageServer(context: ExtensionContext) {

	//for future, possibly may want version checking

	console.log("Before setup");
	const serverPath = path.join(__dirname, '../../../target/release/conjure-oxide');
    
    let serveroptions: ServerOptions = {
        command: serverPath, args: ["server-lsp"]
    }

	let clientoptions: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'essence' }]
	}

	let client = new LanguageClient("Conjure-Oxide Language Server", serveroptions, clientoptions, true);
	client.start();
	console.log("Setup done");

	context.subscriptions.push(client)
}
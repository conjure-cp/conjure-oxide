"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
const node_1 = require("vscode-languageclient/node");
function activate(context) {
    tryStartLanguageServer(context);
}
function tryStartLanguageServer(context) {
    //for future, possibly may want version checking
    console.log("Before setup");
    let serveroptions = {
        command: "conjure-oxide", args: ["server-lsp"]
    };
    let clientoptions = {
        documentSelector: [{ scheme: 'file', language: 'essence' }]
    };
    let client = new node_1.LanguageClient("Conjure-Oxide Language Server", serveroptions, clientoptions, true);
    client.start();
    console.log("Setup done");
    context.subscriptions.push(client);
}

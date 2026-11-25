"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
const path = __importStar(require("path"));
const node_1 = require("vscode-languageclient/node");
function activate(context) {
    tryStartLanguageServer(context);
}
function tryStartLanguageServer(context) {
    //for future, possibly may want version checking
    console.log("Before setup");
    const serverPath = path.join(__dirname, '../../../target/release/conjure-oxide');
    let serveroptions = {
        command: serverPath, args: ["server-lsp"]
    };
    let clientoptions = {
        documentSelector: [{ scheme: 'file', language: 'essence' }]
    };
    let client = new node_1.LanguageClient("Conjure-Oxide Language Server", serveroptions, clientoptions, true);
    client.start();
    console.log("Setup done");
    context.subscriptions.push(client);
}

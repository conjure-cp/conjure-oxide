[//]: # (Author: Liz Dempster)
[//]: # (Last Updated: 19/12/2025)

# LSP Documentation
## Overview 
This is the overview documentation for the Languge Server which is developed for use by Conjure Oxide. This is developed following the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) developed by Microsoft Visual Studio Code, and implemented following VSCode guidelines. This means that functionality cannot be guaranteed on other IDE, though this does not mean it will/cannot work. The Language Server and Client provide the basis through which a fully functional Essence extension will be produced. The server and client run as separate processes primarily because this minimises performance cost.

## Current Functionality Goals 
Where implemented or actively in progress, the details of each goal shall be expanded upon. Achieving these goals would be considered to be a fully completed implementation.
- Syntax Highlighting
   - This is highlighting based on grammar structure
- Semantic Highlighting
   - This is highlighting based on meaning
- Error Underlining
   - Underlining errors of varying severity across the characters causing error
- Hover Tooltips
   - When hovering over a word, produce information relating
- Code Autocomplete
   - Produce completion options while user types 

## Files
```bash
crates/conjue-cp-language-server # the server (excluding diagnostics)
├─ src/
├── lib.rs
├── server.rs
├─ Cargo.toml
tools/vscode-ext # the client
├─ node_modules/ #contains installed node_modules
├─ out/ #contains info produced by npm compile
├─ src/
├── extension.ts
├─ syntaxes/
├── essence.tmLanguage.json
├─ language-configuration.json
├─ package-lock.json
package.json
package-lock.json
tsconfig.json
```
## Client
The Client is a VSCode extention, in TypeScript. As such, the client represents the installed extention which is actively being used by a user. Broadly, we consider the client to be the the VSCode IDE in any instance where a `.essence` file is open.

### Key Structure
The Client itself is programmed within a TypeScript file, called `extention.ts`. This simply launches the client, which then launches the server using `ServerOptions` to call `conjure-oxide server-lsp`.

Additionally, there is a `syntaxes/essence.tmLanguage.json` file, which lays out the syntactic grammar of Essence in a TextMate file. This file is what allows for Syntax Highlighting. The reason that this file exists, alongside the highlighting which will be performed by the server, is that this is extremely lightweight to implement. There are often performance costs associated with an LSP, as they can be large and require a lot of computation. As such, TextMate grammars allow for simple highlighting to occur before the LSP loads/while it is performing computation.

The client also requires a number of `package[-lock].json` files. The functionality of these files is to declare to the VSCode IDE what the extention is called, where it is sourced within the codebase, and what it's functionality is. These files also establish the dependencies which are required for the client to function.

### Syntax Highlighting

## Server
The server is a Rust server, created using tower-lsp. The implementation abstracts over the implementation of the Diagnostic API and the parser, both of which are implemented and manipulated further downstream (if we consider the client and server to be the frontmost end of the project). This allows for the server to implement new functionality freely, so long as the information is capable of being provided by the Diagnostic API.

### Key Structure

### Error Underlining
The core functionality of this is expanded upon within the [Diagnostic API Documentation](https://github.com/conjure-cp/conjure-oxide/tree/main/docs/src/documentation/diagnostics_api.md). The server abstracts over most of the complex implementation and simply calls `get_diagnostics` in order to recieve a vector of all diagnostics within a provided file. The server then performs simple transformations in order to convet the ParserDiagnostic into a Diagnostic JSON object which will be accepted by the client.

### Hover Tooltips

## Client-Server Communication
The client and server communicate asynchronously through I/O streams. Communication occurs through requests and responses, which use well-defined JSON objects to communicate. This communication is triggered on events (such as on-open, on-save, on-change), where the client will prompt the server.

A client and server declare their capabilities during the initialisation handshake. These capabilities reflect what the server and client actually support, and so when new features are added these capabilities must be updated. Capabilities essentially allow for server and client to inform the other whether or not they support specific types of requests, ensuring that requests are not made needlessly. For example, `text_document_sync` indicates that the server supports syncing, meaning that the client will communicate on sync events (such as on-change and on-save).

## How to use (for development)
At the current stage of development, the extension is not being released. This means that functionality is being tested using VSCode's Extention Development environment. In order to ensure that this works as intended, compile `extension.ts` using `npm run compile` (or ctrl-shift-b and select this). Then launch the Extention Development environment, and this will cause the client to run. Due to the structure of the client-server, the server will then run and link to the client, and the LSP can then be tested. 

It is worth noting that a copy of conjure-oxide **MUST** be installed in order for the client to work. Testing the server requires for an updated install. 

If wishing to just run the server (though without a client it will be incapable of connecting to anything, and will time out), the user can run `conjure-oxide server-lsp`, which is a custom subcommand specifically to call the server. This does not run any other aspects of conjure-oxide.

## 

[//]: # (Author: Liz Dempster)
[//]: # (Last Updated: 19/12/2025)

# LSP Documentation
## Overview 
This is the overview documentation for the Language Server which is developed for use by Conjure Oxide. This is developed following the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) developed by Microsoft Visual Studio Code, and implemented following VSCode guidelines. This means that functionality cannot be guaranteed on other IDE, though this does not mean it will/cannot work. The Language Server and Client provide the basis through which a fully functional Essence extension will be produced. The server and client run as separate processes primarily because this minimises performance cost. 

At present this addresses the client and server, with abstraction over lower level functions called by the server. 

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
# the server (excluding diagnostics)
crates/conjue-cp-language-server 
├─ src/
├── lib.rs
├── server.rs
├─ Cargo.toml

# the client
tools/vscode-ext 
├─ node_modules/ #contains installed node_modules
├─ out/ #contains files produced by npm compile
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
## Client-Server Communication
The client and server communicate asynchronously through I/O streams. Communication occurs through requests and responses, which use well-defined JSON objects to communicate. This communication is triggered on events (such as on-open, on-save, on-change), where the client will prompt the server.

A client and server declare their capabilities during the initialisation handshake. These capabilities reflect what the server and client actually support, and so when new features are added these capabilities must be updated. Capabilities essentially allow for server and client to inform the other whether or not they support specific types of requests, ensuring that requests are not made needlessly. For example, `text_document_sync` indicates that the server supports syncing, meaning that the client will communicate on sync events (such as on-change and on-save).

## Client
The Client is a VSCode extention, in TypeScript. As such, the client represents the installed extention which is actively being used by a user. Broadly, we consider the client to be the the VSCode IDE in any instance where a `.essence` file is open.

### Key Structure
The Client itself is programmed within a TypeScript file, called `extention.ts`. This simply launches the client, which then launches the server using `ServerOptions` to call `conjure-oxide server-lsp`.

Additionally, there is a `syntaxes/essence.tmLanguage.json` file, which lays out the syntactic grammar of Essence in a TextMate file. This file is what allows for Syntax Highlighting. The reason that this file exists, alongside the highlighting which will be performed by the server, is that this is extremely lightweight to implement. There are often performance costs associated with an LSP, as they can be large and require a lot of computation. As such, TextMate grammars allow for simple highlighting to occur before the LSP loads/while it is performing computation. `language-configuration.json` provides autoclosing of brackets, speech marks, etc.

The client also requires a number of `package[-lock].json` files. The functionality of these files is to declare to the VSCode IDE what the extention is called, where it is sourced within the codebase, and what it's functionality is. These files also establish the dependencies which are required for the client to function.

### Syntax Highlighting
As addressed above, syntax highlighting is implemented through a TextMate grammar. The basis of this grammar is that outlined by Conjure's VSCode Extention[^bignote]
‌

## Server
The server is a Rust server, created using tower-lsp. This library was used as it abstracts over the low-level implementation details. The implementation also abstracts over the implementation of the Diagnostic API and the parser, both of which are implemented and manipulated further downstream (if we consider the client and server to be the frontmost end of the project). This allows for the server to implement new functionality freely, so long as the information is capable of being provided by the Diagnostic API. Essentially, the implementation of the Language Server here is a higher level construct which makes requests to other levels (e.g. Diagnostics) when required. This allows for the code of the server to be fairly simple and easy to follow.

### Key Structure
#### Backend
Backend is the custom struct which represents the state and functionality of the Language Server. In this, it **must** implement towerlsp's LanguageServer trait. This struct is named Backend simply because it represents the basis of the Language Server, and because this is common naming convention.

Backend has a custom function, `handle_diagnostic` which takes in a uri (uniform resource identifier: uniquely identifies a file) and reads this file from disk, should it exist. This is done because the client does not always pass `text_document.text` (for example, `.text` is passed on-open, but not on-save). This file is then passed to the background Diagnostic API, transformed from a vector of `parserDianostic` to a vector of `lspDiagnostic` (our name for towerlsp's `Diagnostic` type), and published to the client. When a diagnostic is published to a client, it replaces any previous published diagnostics. 

The LanguageServer trait defined by towerlsp is the interface through which our language server is capable of adhering to the Language Server Protocol. In this, the capabilities of our server, and the implementation of these capabilities on given events, are specified. The current capabilities of the server are limited to syncing (as is required for error underlining).

As established above, during the initialise handshake, the capabilities of the server are laid out. At present, the server only has it's default capabilities and syncing capabilities (as required for error underlining), as it currently is in an in-development state. As more functionality is added to the server, more capabilities will be communicated across this handshake.

There are currently five events handled, of which one is after the initialize handshake (simply communicates for log purposes) and one is shutdown. The other three events are syncing events, which have been mentioned throughout this documentation. The three events implemented at current are `did_open`, `did_save` and `did_change`. The functionality of `did_open` and `did_save` is identical. They get the document uri from the parameters (where parameters to the function represents the JSON object passed by the client, though modified through the towerlsp library), and call `handle_diagnostic`. This is because for an opened or newly saved file, the most recent version will be on disk. This is therefore the easiest way to read the whole text, though not necessarily the most efficient. `did_change` is functionally different from these, as the current file is not necessarily saved. 

#### main
Main is responsible for launching the server using the tokio library, as this is the async library towerlsp is designed to be used with. This is the function which is called from outwith the `conjure-cp-lsp` crate: for example, this is what is called in order to run the server, from conjure-oxide's subcommand `server-lsp`.

#### convert_diagnostics, parser_to_lsp_range, parser_to_lsp_position
These are helper functions which allow for the transformation of a vector of `parseDiagnostics` to `lspDiagnostics`. This is done through iterating through the vector, and setting values according to `parseDiagnostics`. This is because the JSON communication between the Server and it's Diagnostic API is not defined the same as the communication between the Server and the Client, though all of the correct information is communicated. 

### Error Underlining
The core functionality of this is expanded upon within the [Diagnostic API Documentation](https://github.com/conjure-cp/conjure-oxide/tree/main/docs/src/documentation/diagnostics_api.md). Most implementation details of Error Underlining are covered above. Error underlining is done through publishing diagnostics, which contain a range, warning level, and associated error text. The client recieves these diagnostics, and then displays the underlining as directed.

Improvements to error underlining currently revolve around performance considerations. Presently, the whole Essence text is passed around. This is because this is the simplest option (and the performance cost is not great due to the fact that current essence test files are fairly short), and because the Diagnostic API requires the full text in order to parse and identify errors within it. However, it would be best if, instead of simply passing a whole text object, the server caches the text and updates it on syncing events. This would remove the overhead of constantly having to recieve, read, and pass entirely new documents, since the majority of the document will likely already exist within the cache. This is not currenly implemented, but will be. Additionally, this will allow for modifications to be made such that the file need not be read from disk, but rather will passed by the client `on_open`, and then simply updated during all other syncing events.

### Hover Tooltips
Not yet implemented, though this will be the next functionality added to the server. This will use Conjure's documentation to provide information about Essence keywords. 

## Development 
### How to use (for development)
At the current stage of development, the extension is not being released. This means that functionality is being tested using VSCode's Extention Development environment. In order to ensure that this works as intended, compile `extension.ts` using `npm run compile` (or ctrl-shift-b and select this). Then launch the Extention Development environment, and this will cause the client to run. Due to the structure of the client-server, the server will then run and link to the client, and the LSP can then be tested. 

It is worth noting that a copy of conjure-oxide **MUST** be installed in order for the client to work. Testing the server requires for an updated install. 

If wishing to just run the server (though without a client it will be incapable of connecting to anything, and will time out), the user can run `conjure-oxide server-lsp`, which is a custom subcommand specifically to call the server. This does not run any other aspects of conjure-oxide.

### Development Directions
When developing, ensure that whatever functionality is being added has it's respective capability communicated to the client during the initialisation handshake, otherwise the triggering events will not be communicated by the client. 

## References
[^bignote]:
   conjure-cp (2025). conjure-vs-code/syntaxes/essence.tmLanguage.json at main · conjure-cp/conjure-vs-code. [online] GitHub. Available at: https://github.com/conjure-cp/conjure-vs-code/blob/main/syntaxes/essence.tmLanguage.json


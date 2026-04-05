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
   - This is highlighting based on meaning rather than grammar 
- Error Underlining
   - Underlining errors of varying severity, providing informative messaging
- Hover Tooltips
   - When hovering over a word, produce information relating to the hovered word
- Code Autocomplete
   - Produce completion options while user types 

## Files
```bash
# the server (excluding diagnostics)
crates/conjue-cp-lsp
├─ src/
├── lib.rs
├── server.rs
├── handlers/
├─── cache.rs
├─── hovering.rs
├─── mod.rs
├─── sync_event.rs
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

A client and server declare their capabilities during the initialisation handshake. These capabilities reflect what the server and client actually support, and so when new features are added these capabilities must be updated. Capabilities essentially allow for server and client to inform the other whether or not they support specific types of requests, ensuring that requests are not made needlessly. For example, `text_document_sync` indicates that the server supports syncing, meaning that the client will communicate on sync events (such as on-change and on-save); and `hover_provider` indicates that the server supports some hovering, meaning that the client will communicate when hovering is occurring.

## Client
The Client is a VSCode extension, in TypeScript. As such, the client represents the installed extension which is actively being used by a user. Broadly, we consider the client to be the VSCode IDE in any instance where a `.essence` file is open.

### Key Structure
The Client itself is programmed within a TypeScript file, called `extension.ts`. This simply launches the client, which then launches the server using `ServerOptions` to call `conjure-oxide server-lsp`.

Additionally, there is a `syntaxes/essence.tmLanguage.json` file, which lays out the syntactic grammar of Essence in a TextMate file. This file is what allows for Syntax Highlighting. The reason that this file exists, alongside the highlighting which will be performed by the server, is that this is extremely lightweight to implement. There are often performance costs associated with an LSP, as they can be large and require a lot of computation. As such, TextMate grammars allow for simple highlighting to occur before the LSP loads/while it is performing computation. `language-configuration.json` provides autoclosing of brackets, speech marks, etc.

The client also requires a number of `package[-lock].json` files. The functionality of these files is to declare to the VSCode IDE what the extension is called, where it is sourced within the codebase, and what it's functionality is. These files also establish the dependencies which are required for the client to function.

### Syntax Highlighting
As addressed above, syntax highlighting is implemented through a TextMate grammar. The basis of this grammar is that outlined by Conjure's VSCode extension[^bignote]
‌
## Server
The server is a Rust server, created using tower-lsp. This library was used as it abstracts over the low-level implementation details. The implementation also abstracts over the implementation of the Diagnostic API and the parser, both of which are implemented and manipulated further downstream (if we consider the client and server to be the frontmost end of the project). This allows for the server to implement new functionality freely, so long as the information is capable of being provided by the Diagnostic API. Essentially, the implementation of the Language Server here is a higher level construct which makes requests to other levels (e.g. Diagnostics) when required. This allows for the code of the server to be fairly simple and easy to follow.

### Key Structure
#### Backend
Backend is the custom struct which represents the state and functionality of the Language Server. In this, it **must** implement towerlsp's LanguageServer trait. This struct is named Backend simply because it represents the basis of the Language Server, and because this is common naming convention.

The LanguageServer trait defined by towerlsp is the interface through which our language server is capable of adhering to the Language Server Protocol. In this, the capabilities of our server, and the implementation of these capabilities on given events, are specified. The current capabilities of the server are limited to syncing (as is required for error underlining).

As established above, during the initialise handshake, the capabilities of the server are laid out. At present, the server only has it's default capabilities and syncing capabilities (as required for error underlining), as it currently is in an in-development state. As more functionality is added to the server, more capabilities will be communicated across this handshake. It is worth noting that in the capabilities, the `text_document_sync` is set to `INCREMENTAL` to ensure that each trigger event causes the client to pass only the modified content of the file, and the range (except for the first call, where the file is passed in its entirety). This is not the default case.

There are currently six events handled, of which one is after the initialize handshake (simply communicates for log purposes) and one is shutdown. The four additional events implemented at current are `did_open`, `did_save`, `did_change`, the functionality of which is handled by `handlers/sync_events`; and `hover`, which is managed by `handlers/hover.rs`. 

#### cache
This LSP uses a library called [Moka](https://github.com/moka-rs/moka) in order to implement caching. The reason that caching has been added is to reduce the time required to load longer files, especially if they are being reused, as this means that they can simply be recovered from the cache. The cache is also used alongside the incremental updates. Modifying the cache occurs within `sync_events`, but the cache is defined and instantiated using a method from within caching. The cache itself is made up of a series of `CacheCont`s, which store a given files sourcemap, ast, cst, errors, contents, and versioned index. The cache automatically evicts after reaching a size of 10,000B, and has a time to live of 30 minutes (time something exists, actively being used, in cache before being evicted) and a time to idle of 5 minutes (time something exists and is not being used before eviction). The cache uses the uri of a file as its identifier, as this is unique to a file. This means that if a file changes location in the file tree (and therefore its URI changes), it will have to be re-entered into the cache, and the previous copy will time out of TTL/TTI and will be evicted.

#### sync_events
These are split from the main `server.rs` primarily for readability, and to prevent the main body of the server from becoming bloated. Rust allows a struct to have it's `impl` split over multiple files, so this file also simply implements functionality to the Backend struct. 

**handle_did_open**
This is the handler for the did_open event. On open, the cache is queried by the uri of the opened file to see whether it currently exists in the cache. If not, the file is used to generate the cst, ast, sourcemap, etc., which populate the CacheConts. Diagnostics (`handle_diagnostics`) are then ran to produce the error underlining.

**handle_did_save**
If saving a file, it is not currently being changed (this is triggered by a different event). As such, when saving a file, simply fetch the correct entry from the cache and get its diagnostics using `handle_diagnostics`.

**handle_did_change**
When changing a file, due to the `INCREMENTAL` change setting, passes only the change and the range. As such, the handler must use `replace_range` to update the cache store of the text document, and then update the cst. Treesitter has a function called `treesitter::edit` which allows only modified subtrees to be regenerated, therefore not requiring the whole tree to be regenerated. This is another speed advantage of the method of caching. Once the cst is generated, it can be parsed, and the cache contents can be updated. The diagnostics can then be generated.

Backend has a custom function, `handle_diagnostics` which takes in a uri (uniform resource identifier: uniquely identifies a file) and the text of a file. This file is then passed to the background Diagnostic API, transformed from a vector of `parserDianostic` to a vector of `lspDiagnostic` (our name for towerlsp's `Diagnostic` type), and published to the client. When a diagnostic is published to a client, it replaces any previous published diagnostics. 

#### convert_diagnostics, parser_to_lsp_range, parser_to_lsp_position
These are helper functions which allow for the transformation of a vector of `parseDiagnostics` to `lspDiagnostics`. This is done through iterating through the vector, and setting values according to `parseDiagnostics`. This is because the JSON communication between the Server and it's Diagnostic API is not defined the same as the communication between the Server and the Client, though all of the correct information is communicated. 

#### position_to_byte, position_to_treesitter_point, calculate_new_end_position
These are helper functions to allow for the functionality of INCREMENTAL synchronising. `position_to_byte` converts from an LSP Position into a byte index, to allow for `replace_range` to function `calculate_new_end_position` is used by 

#### main
Main is responsible for launching the server using the tokio library, as this is the async library towerlsp is designed to be used with. This is the function which is called from outwith the `conjure-cp-lsp` crate: for example, this is what is called in order to run the server, from conjure-oxide's subcommand `server-lsp`. This also instantiates the cache, using `create_cache`.

### Error Underlining
The core functionality of this is expanded upon within the [Diagnostic API Documentation](https://github.com/conjure-cp/conjure-oxide/tree/main/docs/src/documentation/LSP/diagnostics_api.md). Most implementation details of Error Underlining are covered above. Error underlining is done through publishing diagnostics, which contain a range, warning level, and associated error text. The client recieves these diagnostics, and then displays the underlining as directed. The IDE will contain the error message on hover. Error underlining is demonstrated is documented in [Error Underlining Video](https://github.com/conjure-cp/conjure-oxide/tree/main/docs/src/documentation/ErrorUnderlineExample.mp4). 

#### hovering

Hovering is implemented by `handle_hovering`. The contents of the file are gathered from the cache using the uri, and then the sourcemap (from cache) is gathered. The LSP Position is converted into a byte, and then the helper method `hover_info_at_byte` is used to gather the information from the sourcemap for this byte. This is then posted to the client, allowing for the information to be seen on hover.

It is of note that the hovering does not currently work if there is an error anywhere in the tree. If there is an underlined error, then there is no access to hovering, as the AST and sourcemap cannot be generated when there is an error in the CST. This is currently one of the priorities for modification.

## Development 
### How to use (for development)
At the current stage of development, the extension is not released into the VSCode Marketplace. This means that functionality must be tested using VSCode's extension Development environment. In order to ensure that this works as intended, compile `extension.ts` using `npm run compile` (or ctrl-shift-b and select this). Then launch the extension Development environment, and this will cause the client to run. Due to the structure of the client-server, the server will then run and link to the client, and the LSP can then be tested. Please ensure that `npm install` has been run in advance/all required node modules are installed, otherwise the client will not be able to launch. The client can be seen running when opening a .essence file. In the VSCode panel (bar at bottom)'s output, it is possible to select Conjure-Oxide Language Server as the channel to listen to. This will display the logs communicated to the client.

It is worth noting that a copy of conjure-oxide **MUST** be installed in order for the client to work. Testing the server requires for an updated install. 

If wishing to just run the server (though without a client it will be incapable of connecting to anything, and will time out), the user can run `conjure-oxide server-lsp`, which is a custom subcommand specifically to call the server. This does not run any other aspects of conjure-oxide.

### Development Directions
When developing, ensure that whatever functionality is being added has it's respective capability communicated to the client during the initialisation handshake, otherwise the triggering events will not be communicated by the client. 

Furthermore, it is of note that the server and client communicate over standard input and output. As such, nothing which will be called by the server, including the server itself, should ever write onto or read from the stdin and stdout streams. This should always be handled by the tower-lsp library, to ensure that the communication occurring is JSONrpc as expected by the client. Any incorrectly formed writing to the stream will cause the client to close the server and disconnect.

## References
[^bignote]:
   conjure-cp (2025). conjure-vs-code/syntaxes/essence.tmLanguage.json at main · conjure-cp/conjure-vs-code. [online] GitHub. Available at: https://github.com/conjure-cp/conjure-vs-code/blob/main/syntaxes/essence.tmLanguage.json

[//]: # (Author: Anastasia Martinson)
[//]: # (Last Updated: 18/12/2025)

# Overview
The Language Server Protocol (LSP) for Essence is designed to provide error detection and syntax highlighting for the Essence language. It consists of a server component and a diagnostics API. When fully implemented, the server will receive document change events from clients (e.g., VS Code), call the diagnostics API to detect syntactic and semantic errors, and publish these diagnostics back to the client for. The diagnostics API provides error detection and document symbol information for syntax highlighting. Its main purpose is to improve the user experience when formulating the problems in Essence.

![LSP Structure Diagram](assets/lsp_structure.svg)

Key features supported:
- **Diagnostics**: Syntax and semantic error reporting with precise source ranges.
- **Hover**: Information on symbols and expressions.
- **Completion**: Auto-completion for keywords, functions, and variables.
- **Document Symbols**: Outline view for navigating code structure.
- **Semantic Highlighting**: Color-coded tokens based on their roles in the grammar.

# Structure

The LSP architecture follows a client-server model where the client (editor extension) and server (language server) communicate asynchronously via JSON-RPC 2.0 protocol. The server runs as a separate process, typically spawned by the client, and exchanges messages over standard I/O streams.

## Client-Server Communication

- **Transport**: JSON-RPC over stdin/stdout (or TCP/WebSocket in advanced setups).
- **Message Types**:
  - **Requests**: Client-initiated (e.g., `textDocument/didOpen`, `textDocument/completion`).
  - **Responses**: Server replies with results or errors.
  - **Notifications**: One-way messages (e.g., `textDocument/didChange` for incremental updates).
- **Initialization**: Handshake via `initialize` request where capabilities are negotiated.

## Server Architecture

The Conjure-Oxide language server is built on top of the parser and diagnostics modules:
- **Parser Integration**: Uses `conjure-cp-essence-parser` for syntax trees and semantic analysis.
- **Diagnostics API**: Leverages `diagnostics_api.rs` to generate LSP-compliant diagnostic messages.
- **Capabilities**: Supports core LSP features; extensible for additional ones like goto definition or refactoring.

## API Communication Flow

1. **Startup**: Client launches server with `conjure-oxide server-lsp`.
2. **Initialization**: Client sends `initialize` with supported capabilities; server responds with its own.
3. **Document Handling**: Client notifies server of file opens/changes; server parses and caches state.
4. **Feature Requests**: Client requests completions, hovers, etc.; server computes and responds.
5. **Diagnostics**: Server pushes diagnostics asynchronously via `textDocument/publishDiagnostics`.


This diagram illustrates the flow of messages between the VS Code client and the Conjure-Oxide server, highlighting the key components and communication channels.

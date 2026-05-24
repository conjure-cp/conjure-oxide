[//]: # (Author: Anastasia Martinson)
[//]: # (Last Updated: 24/05/2026)

# Overview
The Language Server Protocol (LSP) for Essence is designed to provide error detection, hover support, and semantic highlighting for the Essence language. It consists of a server component and a diagnostics API. The server receives document sync events from clients (e.g., VS Code), calls parser and diagnostics components to detect syntactic and semantic errors, and publishes these diagnostics back to the client. The diagnostics API provides shared diagnostic data structures and conversion helpers, alongside symbol kinds used for semantic highlighting. Its main purpose is to improve the user experience when formulating the problems in Essence.

# Structure
The LSP architecture follows a client-server model where the client (editor extension) and server (language server) communicate asynchronously via JSON-RPC 2.0 protocol. The server runs as a separate process, and exchanges messages over standard I/O streams. More on that in [LSP structure docs](server-client-model.md).

On `did_open` and `did_change`, the server parses and collects diagnostics (see [Diagnostics API](diagnostics-api.md)), converts them to `tower-lsp` diagnostics, and publishes them to the client. Hover and semantic-token requests use cached source-map data from the same parse pipeline.

![LSP Structure Diagram](lsp-structure.svg)

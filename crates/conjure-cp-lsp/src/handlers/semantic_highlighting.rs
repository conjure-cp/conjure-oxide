use crate::server::Backend;
use conjure_cp_essence_parser::diagnostics::semantic_tokens::encode_semantic_tokens;
use tower_lsp::{jsonrpc::Error, lsp_types::*};

impl Backend {
    pub async fn handle_semantic_highlighting(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>, Error> {
        self.client
            .log_message(MessageType::INFO, "semantic highlighting")
            .await;
        let uri = params.text_document.uri.clone();

        let lsp_cache = &self.lsp_cache;

        let cache_conts = match lsp_cache.get(&uri).await {
            Some(conts) => conts,
            None => {
                self.client
                    .log_message(MessageType::WARNING, "Document not found in cache")
                    .await;
                return Ok(None);
            }
        };

        let source_map = match &cache_conts.sourcemap {
            Some(map) => map,
            None => {
                self.client
                    .log_message(MessageType::WARNING, "No source map found in cache")
                    .await;
                return Ok(None);
            }
        };

        // if let Some(ref ast) = cache_conts.ast {
        //     if let Some(ref cst) = cache_conts.cst {
        //         resolve_references(
        //             &cst.root_node(),
        //             &cache_conts.contents,
        //             &mut source_map,
        //             &ast.symbols.read(),
        //         );
        //     }
        // }

        let data = encode_semantic_tokens(source_map)
            .chunks(5)
            .map(|c| SemanticToken {
                delta_line: c[0],
                delta_start: c[1],
                length: c[2],
                token_type: c[3],
                token_modifiers_bitset: c[4],
            })
            .collect();

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }
}

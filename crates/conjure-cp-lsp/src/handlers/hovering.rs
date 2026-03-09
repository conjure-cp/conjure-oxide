use crate::handlers::cache;
use crate::handlers::{cache::CacheCont, sync_event::position_to_byte};
use crate::server::Backend;
use tower_lsp::{lsp_types::*, jsonrpc::Error};


impl Backend {
    pub async fn handle_hovering(&self, params: HoverParams) -> Result<Option<Hover>, Error> {
        
        self.client.log_message(MessageType::INFO, "hovering").await;

        let uri = params.text_document_position_params.text_document.uri.clone();
        let position = params.text_document_position_params.position.clone();
        
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

        let hover_byte = position_to_byte(&cache_conts.contents, position);

        let info = match source_map.hover_info_at_byte(hover_byte) {
            Some(info) => info.clone(),
            None => {
                return Ok(None);
            }
        };

        Ok(Some(Hover {
            contents: HoverContents::Array(vec![
                    MarkedString::String(info.description),
                    MarkedString::String(info.ty.unwrap())
                ]),
            range: None,
        }))
    }
}
use crate::handlers::{reference, sync_event::position_to_byte};
use crate::server::Backend;
use tower_lsp::{jsonrpc::Error, lsp_types::*};

impl Backend{
    pub async fn handle_references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>, Error>{
        let uri = params
            .text_document_position
            .text_document
            .uri
            .clone();
        
        let position = params
            .text_document_position
            .position;

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

        let mut references = Vec::new();
        
        


        Ok(())
    }
}
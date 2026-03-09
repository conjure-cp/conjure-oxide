use crate::handlers::{cache::CacheCont, sync_event::position_to_byte};
use crate::server::Backend;
use tower_lsp::{lsp_types::*, jsonrpc::Error};


impl Backend {
    pub async fn handle_hovering(&self, params: HoverParams) -> Result<Option<Hover>, Error> {
        
        self.client.log_message(MessageType::INFO, "hovering").await;

        let uri = params.text_document_position_params.text_document.uri.clone();
        let position = params.text_document_position_params.position.clone();
        
        let lsp_cache = &self.lsp_cache;

        if let Some(cache_conts) = lsp_cache.get(&uri).await {
            let source_map = cache_conts.sourcemap.unwrap();
            //check this 
            let hover_byte = position_to_byte(&cache_conts.contents, position);

            let info = source_map.hover_info_at_byte(hover_byte).unwrap().clone();

            return Ok(Some(Hover {
                // contents: HoverContents::Array(vec![
                //     MarkedString::String(info.description),
                //     MarkedString::String(info.ty.unwrap())
                // ]),
                contents: HoverContents::Scalar((MarkedString::String(("some hovering shit".to_string())))),
                range: None
            }));
        }

        Ok(None)
    }
}
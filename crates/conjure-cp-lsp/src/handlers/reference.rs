use crate::handlers::{sync_event::position_to_byte};
use crate::server::Backend;
use tower_lsp::{jsonrpc::Error, lsp_types::*};

impl Backend{
    pub async fn handle_references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>, Error>{
        let uri = params
            .text_document_position
            .text_document
            .uri
            .clone();

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

        let byte = position_to_byte(&cache_conts.contents, params.text_document_position.position);

        let Some((word_start, word_end)) = word_at_byte(&cache_conts.contents, byte) else {
            return Ok(Some(Vec::new()));
        };

        let word = &cache_conts.contents[word_start..word_end];
        let bytes = cache_conts.contents.as_bytes();

        let highlights: Vec<DocumentHighlight> = cache_conts.contents.match_indices(word).filter_map(|(start, _)| {
            let end = start + word.len();
            let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
            let after_ok = end == bytes.len() || !is_word_byte(bytes[end]);

            if !before_ok || !after_ok {
                return None;
            }

            Some(DocumentHighlight {
                range: Range {
                    start: byte_to_lsp_position(&cache_conts.contents, start),
                    end: byte_to_lsp_position(&cache_conts.contents, end),
                },
                kind: Some(DocumentHighlightKind::TEXT),
            })
        }).collect();

        Ok(Some(highlights.into_iter().map(|h| Location {
            uri: uri.clone(),
            range: h.range,
        }).collect()))
    }
}

/// helper to check if a byte is part of a word (alphanumeric or underscore)
fn is_word_byte(b: u8) -> bool {
   b.is_ascii_alphanumeric() || b == b'_'
}

/// helper to find the byte range of the word at a given byte offset in the text
fn word_at_byte(text: &str, byte: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    if byte >= bytes.len() ||bytes.is_empty() || !is_word_byte(bytes[byte]) {
        return None;
    }
    let mut idx = byte.min(bytes.len() - 1);

    // if cursor is just after a word, still treat it as that same word
    // that's how rist does it ayway
    if !is_word_byte(bytes[idx]) && idx > 0 && is_word_byte(bytes[idx - 1]) {
        idx -= 1;
    }

    if !is_word_byte(bytes[idx]) {
        return None;
    }

    let mut start = idx;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = idx + 1;
    while end < bytes.len() && is_word_byte(bytes[end]) {
        end += 1;
    }

    Some((start, end))
}

/// helper to convert byte offset to LSP position (line and character)
fn byte_to_lsp_position(text: &str, byte: usize) -> Position {
    let mut line = 0;
    let mut character = 0;

    for ch in text[..byte].chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }

    Position { line, character }
}

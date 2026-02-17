/**
 * Source map for mapping span IDs to source code locations and related metadata.
 * This is used for error reporting and diagnostics.
 */

use crate::diagnostics::diagnostics_api::{Position};
pub type SpanId = u32;

pub struct HoverInfo {
    pub description: String, // keyword description, type info...
    pub kind: Option<SymbolKind>, // var, domain, function...
    pub ty: Option<String>, // type info like int(0..10)
    pub decl_span: Option<SpanId>, // where declared (not sure that's doable)
}
// source span with start and end positions
// in the essence source code
pub struct SourceSpan {
    pub start_byte: usize, // byte offset in the source code
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
    pub hover_info: Option<HoverInfo>,
}

// can add more metadata for hovering and stuff
#[derive(Debug, Default)]
pub struct SourceMap {
    pub spans: Vec<SourceSpan>,
    pub by_byte: Vec<Option<SpanId>>,
}

// allocate a new span and return span id
// put the position of the span in the source map
pub fn alloc_span(range: tree_sitter::Range, source_map: &mut SourceMap, hover_info: Option<HoverInfo>) -> SpanId {
    let span_id = source_map.spans.len() as SpanId;
    source_map.spans.push(SourceSpan {
        start_byte: range.start_byte,
        end_byte: range.end_byte,
        start_point: Position {
            line: range.start_point.row as u32,
            character: range.start_point.column as u32,
        },
        end_point: Position {
            line: range.end_point.row as u32,
            character: range.end_point.column as u32,
        },
        hover_info,
    });
    // map byte offsets to span id
    for i in range.start_byte..range.end_byte {
        if i < source_map.by_byte.len() {
            source_map.by_byte[i] = Some(span_id);
        } else {
            // extend the by_byte
            source_map.by_byte.push(Some(span_id));
        }
    }
    span_id
}
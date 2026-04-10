/**
 * Source map for mapping span IDs to source code locations and related metadata.
 * This is used for error reporting and diagnostics.
 */
use crate::diagnostics::diagnostics_api::{Position, SymbolKind};
use rangemap::RangeMap;
pub type SpanId = u32;

#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub description: String,       // keyword description, type info...
    pub kind: Option<SymbolKind>,  // var, domain, function...
    pub ty: Option<String>,        // type info like int(0..10)
    pub decl_span: Option<SpanId>, // where declared (not sure that's doable)
}
// source span with start and end positions
// in the essence source code
#[derive(Debug, Clone)]
pub struct SourceSpan {
    pub start_byte: usize, // byte offset in the source code
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
    pub hover_info: Option<HoverInfo>,
}

// can add more metadata for hovering and stuff
#[derive(Debug, Default, Clone)]
pub struct SourceMap {
    pub spans: Vec<SourceSpan>,
    pub by_byte: RangeMap<usize, SpanId>,
}

// allocate a new span and return span id
// put the position of the span in the source map
pub fn alloc_span(
    range: tree_sitter::Range,
    source_map: &mut SourceMap,
    hover_info: Option<HoverInfo>,
) -> SpanId {
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
    // map byte offsets to span id (RangeMap handles lookup)
    source_map
        .by_byte
        .insert(range.start_byte..range.end_byte, span_id);
    span_id
}

impl SourceMap {
    // helper to get hover info for a given byte offset (e.g. cursor position)
    pub fn span_id_at_byte(&self, byte: usize) -> Option<SpanId> {
        self.by_byte.get(&byte).copied()
    }

    // helper to get hover info for a given byte offset (e.g. cursor position)
    pub fn hover_info_at_byte(&self, byte: usize) -> Option<&HoverInfo> {
        self.span_id_at_byte(byte)
            .and_then(|span_id| self.spans.get(span_id as usize))
            .and_then(|span| span.hover_info.as_ref())
    }
}

// helper to allocate a span with hover info directly from a tree-sitter node
// source is not used yet but could be for more complex hover info (e.g. showing the actual code snippet)
pub fn span_with_hover(
    node: &tree_sitter::Node,
    _source: &str,
    map: &mut SourceMap,
    info: HoverInfo,
) -> SpanId {
    alloc_span(node.range(), map, Some(info))
}

/// Fetch Essence syntax documentation from Conjure's `docs/bits/` folder on GitHub.
///
/// `name` should typically be something like `"bool" or "L_bool"
pub fn get_documentation(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut base = trimmed.to_string();
    if let Some(stripped) = base.strip_suffix(".md") {
        base = stripped.to_string();
    }

    // This url is for raw Markdown bytes
    let url =
        format!("https://raw.githubusercontent.com/conjure-cp/conjure/main/docs/bits/{base}.md");

    let output = std::process::Command::new("curl")
        .args(["-fsSL", &url])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

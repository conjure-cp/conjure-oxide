use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::SourceMap;
use crate::parser::syntax_errors::line_start_byte;

pub const TOKEN_TYPE_NUMBER: u32 = 0;
pub const TOKEN_TYPE_FUNCTION: u32 = 1;
pub const TOKEN_TYPE_VARIABLE: u32 = 2;
pub const TOKEN_TYPE_LETTING: u32 = 3;
pub const TOKEN_TYPE_FIND: u32 = 4;
pub const TOKEN_TYPE_DOMAIN: u32 = 5;
pub const TOKEN_TYPE_LETTINGVAR: u32 = 6;
pub const TOKEN_TYPE_FINDVAR: u32 = 7;
pub const TOKEN_TYPE_GIVEN: u32 = 8;
pub const TOKEN_TYPE_GIVENVAR: u32 = 9;

pub const MODIFIER_DECLARATION: u32 = 0;
pub const MODIFIER_READONLY: u32 = 1;

pub struct TokenEncoding {
    pub ty: u32,
    pub modifiers: u32,
}

fn utf16_units(bytes: &[u8]) -> u32 {
    String::from_utf8_lossy(bytes).encode_utf16().count() as u32
}

fn line_start_offsets(source: &[u8]) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (idx, b) in source.iter().enumerate() {
        if *b == b'\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

fn line_index_at_byte(line_starts: &[usize], byte: usize) -> usize {
    // index of last line start <= byte
    line_starts.partition_point(|&start| start <= byte).saturating_sub(1)
}

// maps kind in SourceMap into a TokenEncoding
pub fn token_encoding(kind: &SymbolKind) -> Option<TokenEncoding> {
    match kind {
        SymbolKind::Integer => Some(TokenEncoding {
            ty: TOKEN_TYPE_NUMBER,
            modifiers: 0,
        }),
        SymbolKind::Decimal => Some(TokenEncoding {
            ty: TOKEN_TYPE_NUMBER,
            modifiers: 0,
        }),
        SymbolKind::Function => Some(TokenEncoding {
            ty: TOKEN_TYPE_FUNCTION,
            modifiers: 0,
        }),
        SymbolKind::Variable => Some(TokenEncoding {
            ty: TOKEN_TYPE_VARIABLE,
            modifiers: 0,
        }),
        SymbolKind::Constant => Some(TokenEncoding {
            ty: TOKEN_TYPE_VARIABLE,
            modifiers: (1 << MODIFIER_READONLY),
        }),
        SymbolKind::Letting => Some(TokenEncoding {
            ty: TOKEN_TYPE_LETTING,
            modifiers: 0,
        }),
        SymbolKind::Find => Some(TokenEncoding {
            ty: TOKEN_TYPE_FIND,
            modifiers: 0,
        }),
        SymbolKind::Domain => Some(TokenEncoding {
            ty: TOKEN_TYPE_DOMAIN,
            modifiers: 0,
        }),
        SymbolKind::FindVar => Some(TokenEncoding {
            ty: TOKEN_TYPE_FINDVAR,
            modifiers: (1 << MODIFIER_DECLARATION),
        }),
        SymbolKind::LettingVar => Some(TokenEncoding {
            ty: TOKEN_TYPE_LETTINGVAR,
            modifiers: (1 << MODIFIER_DECLARATION),
        }),
        SymbolKind::Given => Some(TokenEncoding {
            ty: TOKEN_TYPE_GIVEN,
            modifiers: 0,
        }),
        SymbolKind::GivenVar => Some(TokenEncoding {
            ty: TOKEN_TYPE_GIVENVAR,
            modifiers: (1 << MODIFIER_DECLARATION),
        }),
    }
}

// translate span in SourceMap into the VSCode semantic token format
// NOTE: LSP semantic token positions and lengths are UTF-16 code units.
pub fn encode_semantic_tokens(source_map: &SourceMap, source: &str) -> Vec<u32> {
    let source_bytes = source.as_bytes();
    let line_starts = line_start_offsets(source_bytes);
    let mut entries: Vec<(u32, u32, u32, u32, u32)> = source_map
        .spans
        .iter()
        .filter_map(|span| {
            let kind = span.hover_info.as_ref()?.kind.as_ref()?;
            // if (kind == )
            // let ty = span.hover_info.as_ref()?.ty.as_ref()?
            let enc = token_encoding(kind)?;

            let start_byte = span.start_byte;
            let end_byte = span.end_byte;
            if end_byte <= start_byte || end_byte > source_bytes.len() || start_byte > source_bytes.len() {
                return None;
            }

            let start_line = line_index_at_byte(&line_starts, start_byte);
            let end_line = line_index_at_byte(&line_starts, end_byte.saturating_sub(1));
            if start_line != end_line {
                // LSP semantic token entries should not span lines.
                return None;
            }

            let line_start = line_start_byte(source_bytes, start_line);
            if start_byte < line_start {
                return None;
            }

            let col = utf16_units(source_bytes.get(line_start..start_byte)?);
            let len = utf16_units(source_bytes.get(start_byte..end_byte)?);
            if len == 0 {
                return None;
            }
            Some((
                start_line as u32,
                col,
                len,
                enc.ty,
                enc.modifiers,
            ))
        })
        .collect();

    // filter out spans with nested spans
    // let mut filtered: Vec<(u32, u32, u32, u32, u32)> = Vec::new();
    // for entry in entries {
    //     let (line, col, len, _, _) = entry;
    //     let end = col + len;

    //     let overlaps = filtered.iter().any(|&(fl, fc, fl_len, _, _)| {
    //         fl == line && fc <= col && fc + fl_len >= end
    //     });

    //     if !overlaps {
    //         filtered.push(entry);
    //     }
    // }

    entries.sort_by_key(|&(line, col, _, _, _)| (line, col));

    let mut data = Vec::with_capacity(entries.len() * 5);
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;

    for (line, col, len, ty, modifiers) in entries {
        let delta_line = line - prev_line;
        let delta_col = if delta_line == 0 { col - prev_col } else { col };
        data.extend_from_slice(&[delta_line, delta_col, len, ty, modifiers]);
        prev_line = line;
        prev_col = col;
    }

    data
}

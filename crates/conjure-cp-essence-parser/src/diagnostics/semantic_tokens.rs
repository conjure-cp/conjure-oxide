use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::SourceMap;

pub const TOKEN_TYPE_NUMBER: u32 = 0;
pub const TOKEN_TYPE_FUNCTION: u32 = 1;
pub const TOKEN_TYPE_VARIABLE: u32 = 2;
pub const TOKEN_TYPE_LETTING: u32 = 3;
pub const TOKEN_TYPE_FIND: u32 = 4;
pub const TOKEN_TYPE_DOMAIN: u32 = 5;
pub const TOKEN_TYPE_LETTINGVAR: u32 = 6;
pub const TOKEN_TYPE_FINDVAR: u32 = 7;

pub const MODIFIER_NONE: u32 = 0;
pub const MODIFIER_DECLARATION: u32 = 1;
pub const MODIFIER_READONLY: u32 = 2;

pub struct TokenEncoding {
    pub ty: u32,
    pub modifiers: u32,
}

// maps kind in SourceMap into a TokenEncoding
pub fn token_encoding(kind: &SymbolKind) -> Option<TokenEncoding> {
    match kind {
        SymbolKind::Integer => Some(TokenEncoding {
            ty: TOKEN_TYPE_NUMBER,
            modifiers: MODIFIER_NONE,
        }),
        SymbolKind::Decimal => Some(TokenEncoding {
            ty: TOKEN_TYPE_NUMBER,
            modifiers: MODIFIER_NONE,
        }),
        SymbolKind::Function => Some(TokenEncoding {
            ty: TOKEN_TYPE_FUNCTION,
            modifiers: MODIFIER_NONE,
        }),
        SymbolKind::Variable => Some(TokenEncoding {
            ty: TOKEN_TYPE_VARIABLE,
            modifiers: MODIFIER_NONE,
        }),
        SymbolKind::Constant => Some(TokenEncoding {
            ty: TOKEN_TYPE_VARIABLE,
            modifiers: MODIFIER_READONLY,
        }),
        SymbolKind::Letting => Some(TokenEncoding {
            ty: TOKEN_TYPE_LETTING,
            modifiers: MODIFIER_NONE,
        }),
        SymbolKind::Find => Some(TokenEncoding {
            ty: TOKEN_TYPE_FIND,
            modifiers: MODIFIER_NONE,
        }),
        SymbolKind::Domain => Some(TokenEncoding {
            ty: TOKEN_TYPE_DOMAIN,
            modifiers: MODIFIER_NONE,
        }),
        SymbolKind::FindVar => Some(TokenEncoding {
            ty: TOKEN_TYPE_FINDVAR,
            modifiers: MODIFIER_DECLARATION,
        }),
        SymbolKind::LettingVar => Some(TokenEncoding {
            ty: TOKEN_TYPE_LETTINGVAR,
            modifiers: MODIFIER_DECLARATION,
        }),
    }
}

// translate span in SourceMap into the VSCode semantic token format
pub fn encode_semantic_tokens(source_map: &SourceMap) -> Vec<u32> {
    let mut entries: Vec<(u32, u32, u32, u32, u32)> = source_map
        .spans
        .iter()
        .filter_map(|span| {
            let kind = span.hover_info.as_ref()?.kind.as_ref()?;
            // if (kind == )
            // let ty = span.hover_info.as_ref()?.ty.as_ref()?
            let enc = token_encoding(kind)?;
            Some((
                span.start_point.line,
                span.start_point.character,
                (span.end_byte - span.start_byte) as u32,
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

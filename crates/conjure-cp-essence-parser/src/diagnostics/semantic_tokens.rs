use crate::diagnostics::diagnostic_api::SymbolKind;
use crate::source_map::SourceMap;

pub const TOKEN_TYPE_NUMBER:   u32 = 0;
pub const TOKEN_TYPE_FUNCTION: u32 = 1;
pub const TOKEN_TYPE_VARIABLE: u32 = 2;
pub const TOKEN_TYPE_LETTING:  u32 = 3;
pub const TOKEN_TYPE_FIND:     u32 = 4;
pub const TOKEN_TYPE_DOMAIN:   u32 = 5;

pub const MODIFIER_NONE:     u32 = 0;
pub const MODIFIER_READONLY: u32 = 1; 

pub struct TokenEncoding {
    pub ty: u32,
    pub modifiers: u32,
}

pub fn token_encoding(kind: &SymbolKind) -> Option<TokenEncoding> {
    match kind {
        SymbolKind::Integer  => Some(TokenEncoding { ty: TOKEN_TYPE_NUMBER,   modifiers: MODIFIER_NONE }),
        SymbolKind::Decimal  => Some(TokenEncoding { ty: TOKEN_TYPE_NUMBER,   modifiers: MODIFIER_NONE }),
        SymbolKind::Function => Some(TokenEncoding { ty: TOKEN_TYPE_FUNCTION, modifiers: MODIFIER_NONE }),
        SymbolKind::Variable => Some(TokenEncoding { ty: TOKEN_TYPE_VARIABLE, modifiers: MODIFIER_NONE }),
        SymbolKind::Constant => Some(TokenEncoding { ty: TOKEN_TYPE_VARIABLE, modifiers: MODIFIER_READONLY }),
        SymbolKind::Letting  => Some(TokenEncoding { ty: TOKEN_TYPE_LETTING,  modifiers: MODIFIER_NONE }),
        SymbolKind::Find     => Some(TokenEncoding { ty: TOKEN_TYPE_FIND,     modifiers: MODIFIER_NONE }),
        SymbolKind::Domain   => Some(TokenEncoding { ty: TOKEN_TYPE_DOMAIN,   modifiers: MODIFIER_NONE }),
    }
}

pub fn encode_semantic_tokens(source_map: &SourceMap) -> Vec<u32> {

}
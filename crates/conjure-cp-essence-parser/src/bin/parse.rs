use conjure_cp_core::ast::SymbolTablePtr;
use conjure_cp_essence_parser::{parse_exprs, parse_program};

fn main() {
    let symbols = SymbolTablePtr::new();
    dbg!(parse_program(
        include_str!("../../../../new.essence"),
        symbols
    ));
}

use std::sync::{Arc, RwLock};

use conjure_cp_core::ast::{DeclarationKind, Name};
use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::parse_essence_file_native;

#[test]
fn parses_find_aux_as_find_auxiliary() {
    let dir = std::env::temp_dir().join(format!("conjure-oxide-findaux-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("model.essence");
    std::fs::write(
        &path,
        "language Essence 1.0\nfindAux a: bool\nfind x: int(1..3)\nsuch that a -> (x = 1)\n",
    )
    .expect("write model");

    let context = Arc::new(RwLock::new(Context::default()));
    let model =
        parse_essence_file_native(path.to_str().unwrap(), context).expect("parse findAux model");

    let _ = std::fs::remove_dir_all(&dir);

    let a = model
        .symbols()
        .lookup(&Name::user("a"))
        .expect("a should be declared");
    let x = model
        .symbols()
        .lookup(&Name::user("x"))
        .expect("x should be declared");

    assert!(
        matches!(
            &a.kind() as &DeclarationKind,
            DeclarationKind::FindAuxiliary(_)
        ),
        "findAux should parse as FindAuxiliary, got {:?}",
        a.kind()
    );
    assert!(
        matches!(&x.kind() as &DeclarationKind, DeclarationKind::Find(_)),
        "find should parse as Find, got {:?}",
        x.kind()
    );
    assert!(a.is_find_auxiliary());
    assert!(!x.is_find_auxiliary());
}

//! Registration must work across crate boundaries without explicit initialisation.
use conjure_cp::representation::{ReprRule, get_repr_by_name, get_repr_rules};
use conjure_cp::rule_engine::{get_all_rules, get_rule_by_name, get_rule_set_by_name};
use conjure_cp_rules::representation::IntDirect;

mod downstream {
    use conjure_cp::ast::{Expression, SymbolTable};
    use conjure_cp::rule_engine::{
        ApplicationResult, RuleEffect, register_rule, register_rule_set,
    };

    register_rule_set!("RegistrationTest");

    #[register_rule("RegistrationTest", 7)]
    fn downstream_identity(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
        Ok(RuleEffect::pure(expr.clone()))
    }
}

#[test]
fn discovers_downstream_rules_and_rule_sets() {
    let rule = get_rule_by_name("downstream_identity").unwrap();
    let set = get_rule_set_by_name("RegistrationTest").unwrap();
    assert_eq!(set.get_rules().get(rule), Some(&7));
    assert_eq!(
        get_all_rules()
            .iter()
            .filter(|r| r.name == rule.name)
            .count(),
        1
    );
}

#[test]
fn discovers_representations_from_the_rules_crate() {
    let repr = get_repr_by_name(IntDirect::NAME).unwrap();
    assert_eq!(repr.short_name(), IntDirect::SHORT_NAME);
    assert_eq!(
        get_repr_rules()
            .filter(|r| r.name() == IntDirect::NAME)
            .count(),
        1
    );
}

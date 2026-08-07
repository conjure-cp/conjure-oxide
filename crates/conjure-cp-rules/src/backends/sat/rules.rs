use conjure_cp::rule_engine::register_rule_set;
use conjure_cp::settings::SolverFamily;

// BOOLEAN SAT ENCODING RULES:

register_rule_set!("SAT", ("Base"), |f: &SolverFamily| matches!(
    f,
    SolverFamily::Sat(_)
));

register_rule_set!("SAT_Direct", ("SAT"));

register_rule_set!("SAT_Order", ("SAT"));

register_rule_set!("SAT_Log", ("SAT"));

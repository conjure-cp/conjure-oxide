use conjure_cp::rule_engine::register_rule_set;
use conjure_cp::solver::SolverFamily;

// BOOLEAN SAT ENCODING RULES:

register_rule_set!("SAT", ("Base"), |f: &SolverFamily| matches!(
    f,
    SolverFamily::Sat
));

register_rule_set!("SAT_Direct", ("SAT"));

register_rule_set!("SAT_Log", ("SAT"));

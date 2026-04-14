use conjure_cp::rule_engine::register_rule_set;
use conjure_cp::settings::{SatEncoding, SolverFamily};

// BOOLEAN SAT ENCODING RULES:

register_rule_set!("SAT", ("Base"), |f: &SolverFamily| matches!(
    f,
    SolverFamily::Sat(_)
));

register_rule_set!("SAT_Direct", ("SAT"), |f: &SolverFamily| matches!(
    f,
    SolverFamily::Sat(SatEncoding::Direct)
));

register_rule_set!("SAT_Order", ("SAT"));

register_rule_set!("SatIntLog", ("SAT"));

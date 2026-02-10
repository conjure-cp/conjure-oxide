use conjure_cp::rule_engine::register_rule_set;
use conjure_cp::solver::SolverFamily;

// BOOLEAN SAT ENCODING RULES:

register_rule_set!("SAT", ("Base"), |f: &SolverFamily| matches!(
    f,
    SolverFamily::Sat
));

register_rule_set!("SAT_Direct", ("SAT"));

register_rule_set!("SAT_Order", ("SAT"));

register_rule_set!("SAT_Log", ("SAT"));

// Encoding rules should have the following priorities:

// | Rule Type                           | Priority |
// |-------------------------------------|----------|
// | Integer Decision Variable -> SATInt | 4800     |
// | SATInt -> SATInt                    | 4700     |
// | SATInt -> Boolean                   | 4600     |
// | Literal -> SATInt                   | 4500     |
// | Boolean -> Boolean                  | 4400     |
// | Boolean -> Nothing                  | 4300     |

/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

use conjure_core::rule_engine::register_rule_set;
use conjure_core::solver::SolverFamily;

register_rule_set!("CNF", ("Base"), (SolverFamily::SAT));

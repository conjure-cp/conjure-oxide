//! Compatibility rule set for evaluator normalisation.
//!
//! Constant and partial evaluation used to be ordinary rewrite rules in this module. They are now
//! driven by the rewriter's evaluator normalisation hook so they can run before ordinary rules and
//! immediately after each successful rewrite without paying universal rule-attempt costs. Keep the
//! `Constant` rule set registered so existing rule-set selections and tests that name it continue
//! to resolve.

use conjure_cp::rule_engine::register_rule_set;

register_rule_set!("Constant", ());

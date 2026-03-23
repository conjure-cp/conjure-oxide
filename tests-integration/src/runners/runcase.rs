use conjure_cp::settings::{
    Parser, QuantifiedExpander, Rewriter, SolverFamily, set_comprehension_expander,
    set_current_parser, set_current_rewriter, set_current_solver_family,
    set_minion_discrete_threshold,
};

#[derive(Clone, Copy, Debug)]
pub struct RunCase<'a> {
    pub parser: Parser,
    pub rewriter: Rewriter,
    pub comprehension_expander: QuantifiedExpander,
    pub solver: SolverFamily,
    pub case_name: &'a str,
}

pub fn run_case_name(
    parser: Parser,
    rewriter: Rewriter,
    comprehension_expander: QuantifiedExpander,
) -> String {
    format!("{parser}-{rewriter}-{comprehension_expander}")
}

pub fn run_case_label(
    path: &str,
    essence_base: &str,
    extension: &str,
    run_case: RunCase<'_>,
) -> String {
    format!(
        "test_dir={path}, model={essence_base}.{extension}, parser={}, rewriter={}, comprehension_expander={}, solver={}",
        run_case.parser,
        run_case.rewriter,
        run_case.comprehension_expander,
        run_case.solver.as_str()
    )
}

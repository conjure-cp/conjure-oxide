//! Keep assertion context and reuse RustSAT's atomic implication encodings.
use super::*;
use std::collections::HashSet;

impl Compiler<'_> {
    /// Collect source literals whose truth is fixed by the assertion.
    pub(super) fn collect_asserted_literals(
        &mut self,
        expression: &Expression,
        truth: bool,
        asserted: &mut HashSet<Lit>,
    ) -> Result<(), SolverError> {
        match expression {
            Expression::Atomic(_, Atom::Reference(_)) => {
                let term = self.encode(expression)?;
                if let Term::Literal(literal) = term {
                    asserted.insert(if truth { literal } else { !literal });
                }
            }
            Expression::Not(_, input) => {
                self.collect_asserted_literals(input, !truth, asserted)?;
            }
            Expression::And(_, inputs) if truth => {
                for input in boolean_children(inputs)? {
                    self.collect_asserted_literals(input, truth, asserted)?;
                }
            }
            Expression::Or(_, inputs) if !truth => {
                for input in boolean_children(inputs)? {
                    self.collect_asserted_literals(input, truth, asserted)?;
                }
            }
            Expression::Imply(_, left, right) if !truth => {
                self.collect_asserted_literals(left, true, asserted)?;
                self.collect_asserted_literals(right, false, asserted)?;
            }
            _ => (),
        }
        Ok(())
    }

    /// Compile a Boolean assertion in the requested polarity.
    pub(super) fn assert_expression(
        &mut self,
        expression: &Expression,
        truth: bool,
    ) -> Result<(), SolverError> {
        match expression {
            Expression::Not(_, input) => self.assert_expression(input, !truth)?,
            Expression::And(_, inputs) if truth => {
                for input in boolean_children(inputs)? {
                    self.assert_expression(input, truth)?;
                }
            }
            Expression::Or(_, inputs) if !truth => {
                for input in boolean_children(inputs)? {
                    self.assert_expression(input, truth)?;
                }
            }
            Expression::Imply(_, left, right) if !truth => {
                self.assert_expression(left, true)?;
                self.assert_expression(right, false)?;
            }
            Expression::Iff(_, left, right)
            | Expression::Eq(_, left, right)
            | Expression::Neq(_, left, right) => {
                let left = self.encode(left)?;
                let right = self.encode(right)?;
                let equal = truth != matches!(expression, Expression::Neq(..));
                self.equate(left, if equal { right } else { right.negated() });
            }
            _ => {
                let mut terms = Vec::new();
                self.collect_disjunction(expression, truth, &mut terms)?;
                match self.boolean_literals(false, terms) {
                    Ok(literals) => {
                        self.instance
                            .add_clause(atomics::cube_impl_clause(&[], &literals));
                    }
                    Err(value) => self.assert(value),
                }
            }
        }
        Ok(())
    }

    fn collect_disjunction(
        &mut self,
        expression: &Expression,
        truth: bool,
        terms: &mut Vec<Term>,
    ) -> Result<(), SolverError> {
        match expression {
            Expression::Not(_, input) => self.collect_disjunction(input, !truth, terms)?,
            Expression::And(_, inputs) | Expression::Or(_, inputs)
                if matches!(expression, Expression::Or(..)) == truth =>
            {
                for input in boolean_children(inputs)? {
                    self.collect_disjunction(input, truth, terms)?;
                }
            }
            Expression::Imply(_, left, right) if truth => {
                self.collect_disjunction(left, false, terms)?;
                self.collect_disjunction(right, true, terms)?;
            }
            _ => terms.push(self.encode_polarity(expression, truth)?),
        }
        Ok(())
    }

    // A fresh witness needs only witness -> requested truth. Source Boolean values
    // and reified/PB operands continue through the fully equivalent encoder.
    fn encode_polarity(
        &mut self,
        expression: &Expression,
        truth: bool,
    ) -> Result<Term, SolverError> {
        Ok(match expression {
            Expression::Not(_, input) => self.encode_polarity(input, !truth)?,
            Expression::And(_, inputs) | Expression::Or(_, inputs) => {
                let terms = boolean_children(inputs)?
                    .iter()
                    .map(|input| self.encode_polarity(input, truth))
                    .collect::<Result<Vec<_>, _>>()?;
                let and = matches!(expression, Expression::And(..)) == truth;
                self.combine_implied(and, terms)
            }
            Expression::Imply(_, left, right) => {
                let left = self.encode_polarity(left, !truth)?;
                let right = self.encode_polarity(right, truth)?;
                self.combine_implied(!truth, vec![left, right])
            }
            _ => {
                let term = self.encode(expression)?;
                if truth { term } else { term.negated() }
            }
        })
    }

    /// Normalise constants, aliases, duplicates and complementary inputs.
    pub(super) fn boolean_literals(&self, and: bool, terms: Vec<Term>) -> Result<Vec<Lit>, Term> {
        let mut literals = Vec::new();
        for term in terms {
            match self.resolve_alias(term) {
                Term::Constant(value) if value != and => return Err(Term::Constant(value)),
                Term::Constant(_) => (),
                Term::Literal(literal) => literals.push(literal),
            }
        }
        literals.sort_unstable();
        literals.dedup();
        if literals
            .iter()
            .any(|literal| literals.binary_search(&!(*literal)).is_ok())
        {
            return Err(Term::Constant(!and));
        }
        match literals.as_slice() {
            [] => Err(Term::Constant(and)),
            [literal] => Err(Term::Literal(*literal)),
            _ => Ok(literals),
        }
    }

    fn combine_implied(&mut self, and: bool, terms: Vec<Term>) -> Term {
        let literals = match self.boolean_literals(and, terms) {
            Ok(literals) => literals,
            Err(value) => return value,
        };
        let key = (and, literals.clone());
        let existing = self
            .counters
            .as_ref()
            .and_then(|cache| {
                cache
                    .gates
                    .get(&key)
                    .or_else(|| cache.implied_gates.get(&key))
            })
            .copied();
        if let Some(output) = existing {
            return self.resolve_alias(Term::Literal(output));
        }
        let output = self.instance.new_lit();
        self.emit_gate_implication(and, output, &literals);
        if let Some(cache) = self.counters.as_mut() {
            cache.implied_gates.insert(key, output);
        }
        Term::Literal(output)
    }

    /// Emit the library clauses for output implying its gate value.
    pub(super) fn emit_gate_implication(&mut self, and: bool, output: Lit, inputs: &[Lit]) {
        if and {
            for clause in atomics::lit_impl_cube(output, inputs) {
                self.instance.add_clause(clause);
            }
        } else {
            self.instance
                .add_clause(atomics::lit_impl_clause(output, inputs));
        }
    }

    /// Complete a gate with the reverse implication.
    pub(super) fn emit_gate_reverse(&mut self, and: bool, output: Lit, inputs: &[Lit]) {
        if and {
            self.instance
                .add_clause(atomics::cube_impl_lit(inputs, output));
        } else {
            for clause in atomics::clause_impl_lit(inputs, output) {
                self.instance.add_clause(clause);
            }
        }
    }
}

fn boolean_children(expression: &Expression) -> Result<&[Expression], SolverError> {
    match expression {
        Expression::AbstractLiteral(_, AbstractLiteral::Matrix(inputs, _)) => Ok(inputs),
        _ => Err(SolverError::ModelInvalid(
            "SAT Boolean operands must be an explicit matrix".into(),
        )),
    }
}

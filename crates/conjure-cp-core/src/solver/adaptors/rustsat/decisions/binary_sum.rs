//! Binary arithmetic with storage proportional to input digits, independent of bound values.
use super::*;

impl Compiler<'_> {
    /// Retain the sum digits across threshold checks and objective improvements.
    fn binary_sum(&mut self, positive: &[(Lit, i128)]) -> Vec<Term> {
        if let Some(bits) = self
            .cache
            .as_ref()
            .and_then(|cache| cache.binary_sums.get(positive))
        {
            return bits.clone();
        }
        let mut buckets: Vec<Vec<Term>> = Vec::new();
        for &(literal, weight) in positive {
            let mut weight = weight as u128;
            while weight != 0 {
                let bit = weight.trailing_zeros() as usize;
                buckets.resize_with(buckets.len().max(bit + 1), Vec::new);
                buckets[bit].push(Term::Literal(literal));
                weight &= weight - 1;
            }
        }
        let mut bit = 0;
        while bit < buckets.len() {
            // Compress three digits into a sum digit and a carry; two use a half adder.
            while buckets[bit].len() >= 2 {
                let a = buckets[bit].pop().unwrap();
                let b = buckets[bit].pop().unwrap();
                let xor = self.binary_xor(a, b);
                let ab = self.combine(true, vec![a, b]);
                let (sum, carry) = if let Some(c) = buckets[bit].pop() {
                    let sum = self.binary_xor(xor, c);
                    let xc = self.combine(true, vec![xor, c]);
                    (sum, self.combine(false, vec![ab, xc]))
                } else {
                    (xor, ab)
                };
                buckets[bit].push(sum);
                if carry != Term::Constant(false) {
                    if bit + 1 == buckets.len() {
                        buckets.push(Vec::new());
                    }
                    buckets[bit + 1].push(carry);
                }
            }
            bit += 1;
        }
        let bits: Vec<_> = buckets
            .into_iter()
            .map(|mut bucket| bucket.pop().unwrap_or(Term::Constant(false)))
            .collect();
        if let Some(cache) = self.cache.as_mut() {
            cache.binary_sums.insert(positive.to_vec(), bits.clone());
        }
        bits
    }

    fn binary_xor(&mut self, a: Term, b: Term) -> Term {
        let a = self.resolve_alias(a);
        let b = self.resolve_alias(b);
        match (a, b) {
            (Term::Constant(value), other) | (other, Term::Constant(value)) => {
                if value {
                    other.negated()
                } else {
                    other
                }
            }
            _ if a == b => Term::Constant(false),
            _ if a == b.negated() => Term::Constant(true),
            (Term::Literal(a), Term::Literal(b)) => {
                let output = self.instance.new_lit();
                for clause in [
                    [a, b, !output],
                    [!a, !b, !output],
                    [!a, b, output],
                    [a, !b, output],
                ] {
                    self.instance.add_clause(clause.into_iter().collect());
                }
                Term::Literal(output)
            }
        }
    }

    pub(super) fn word_upper(&mut self, bound: i128, bits: &[Term]) -> Term {
        let mut upper = Term::Constant(true);
        for (bit, &input) in bits.iter().enumerate() {
            // Equality at this digit defers to the lower digits.
            let and = (bound as u128) & (1u128 << bit) == 0;
            upper = self.combine(and, vec![input.negated(), upper]);
        }
        upper
    }

    /// Use a digit comparator for existing words and binary-adder weighted sums.
    pub(super) fn weighted_upper(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        bound: i128,
        positive: &[(Lit, i128)],
        total: i128,
    ) -> Option<Term> {
        if let Some(value) = self.binary_word_upper(bound, positive, total) {
            return Some(value);
        }
        if algorithm != crate::ast::sat_decision::PbEncoding::RustsatBinaryAdder {
            return None;
        }
        if bound < 0 || bound >= total {
            return Some(Term::Constant(bound >= total));
        }
        let bits = self.binary_sum(positive);
        Some(self.word_upper(bound, &bits))
    }
}

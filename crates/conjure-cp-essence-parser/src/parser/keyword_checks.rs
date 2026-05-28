const KEYWORDS: [&str; 24] = [
    "forall",
    "exists",
    "such",
    "that",
    "letting",
    "find",
    "minimise",
    "maximise",
    "subject",
    "to",
    "where",
    "and",
    "or",
    "not",
    "if",
    "then",
    "else",
    "in",
    "sum",
    "product",
    "bool",
    "pareto",
    "minimising",
    "maximising",
];

pub fn is_keyword_identifier(identifier: &str) -> bool {
    KEYWORDS.contains(&identifier)
}

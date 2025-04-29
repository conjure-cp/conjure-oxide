module.exports = grammar ({
  name: 'essence',

  extras: $ => [
    $.single_line_comment,
    /\s/,
    $.language_label

  ],

  rules: {
    // Top-level statements
    program: $ => repeat(choice(
      seq("find", commaSep1(field("find_statement", $.find_statement))),
      seq(
        "such that", 
        commaSep1(choice(field("bool_expr", $.bool_expr), field("atom", $.atom), field("comparison_expr", $.comparison_expr))), 
      ),
      seq("letting", commaSep1(field("letting_statement", $.letting_statement))),
      field("dominance_relation", $.dominance_relation),
      field("find", $.FIND),
      field("letting", $.LETTING),
      field("such_that", $.SUCH_THAT),
    )),

    SUCH_THAT: $ => "such that",
    FIND: $ => "find",
    LETTING: $ => "letting",
    COLON: $ => ":",

    single_line_comment: $ => token(seq('$', /.*/)),

    language_label: $ => token(seq("language", /.*/)),

    //general
    constant: $ => choice(
      field("integer", $.integer),
      field("true", $.TRUE),
      field("false", $.FALSE)
    ),

    // integer: $ => choice(/[0-9]+/, /-[0-9]+/),
    integer: $ => token(/[0-9]+/),

    TRUE: $ => choice("true", "TRUE"),

    FALSE: $ => choice("false", "FALSE"),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    //meta-variable (aka template argument)
    metavar: $ => seq("&", field("identifier", $.identifier)),

    //find statements
    find_statement: $ => seq(
      field("variables", $.variable_list),
      field("colon", $.COLON),
      field("domain", $.domain),
    ),
    variable_list: $ => commaSep1($.identifier),

    domain: $ => choice(
      field("bool_domain", $.bool_domain),
      field("int_domain", $.int_domain),
      field("variable_domain", $.identifier),
      field("tuple_domain", $.tuple_domain),
      field("matrix_domain", $.matrix_domain),
    ),
    bool_domain: $ => "bool",

    int_domain: $ => seq(
      "int",
      optional(seq(
        "(",
        field("ranges", $.range_list),
        ")"
      ))
    ),

    range_list: $ => prec(2, commaSep1(choice($.int_range, $.integer))),

    int_range: $ => seq(
      optional(field("lower", $.arithmetic_expr)), 
      "..", 
      optional(field("upper", $.arithmetic_expr))
    ),

    tuple_domain: $ => seq(
      optional("tuple"),
      "(",
      commaSep1($.domain),
      ")"
    ),

    matrix_domain: $ => seq(
      "matrix",
      "indexed",
      "by",
      "[",
      field("index_domain_list", $.index_domain_list),
      "]",
      "of",
      field("value_domain", $.domain)
    ),

    index_domain_list: $ => commaSep1(choice($.int_domain, $.bool_domain)),

    //letting statements
    letting_statement: $ => seq(
      field("variable_list", $.variable_list), 
      field("be", "be"), 
      optional(field ("domain", "domain")),
      field("expr_or_domain", choice($.bool_expr, $.arithmetic_expr, $.domain))
    ),

    // Constraints 
    bool_expr: $ => choice(
      field("not_expression", $.not_expr),
      field("and_expression", $.and_expr),
      field("or_expression", $.or_expr),
      field("implication", $.implication),
      field("iff_expr", $.iff_expr),
      field("quantifier_expression_bool", $.quantifier_expr_bool),
      field("from_solution", $.from_solution),
      field("sub_bool_expression", $.sub_bool_expr),
    ),

    not_expr: $ => prec(20, seq("!", field("expression", choice($.bool_expr, $.comparison_expr, $.atom)))),
    
    and_expr: $ => prec(-1, prec.left(seq(
      field("left", choice($.bool_expr, $.comparison_expr, $.atom)), 
      field("operator", "/\\"),
      field("right", choice($.bool_expr, $.comparison_expr, $.atom))
    ))),
    
    or_expr: $ => prec(-2, prec.left(seq(
      field("left", choice($.bool_expr, $.comparison_expr, $.atom)), 
      field("operator", "\\/"),
      field("right", choice($.bool_expr, $.comparison_expr, $.atom))
    ))),
    
    implication: $ => prec(-4, prec.left(seq(
      field("left", choice($.bool_expr, $.comparison_expr, $.atom)), 
      field("operator", "->"), 
      field("right", choice($.bool_expr, $.comparison_expr, $.atom))
    ))),

    iff_expr: $ => prec(-4, prec.left(seq(
      field("left", choice($.bool_expr, $.comparison_expr, $.atom)), 
      field("operator", "<->"), 
      field("right", choice($.bool_expr, $.comparison_expr, $.atom))
    ))),

    quantifier_expr_bool: $ => prec(-10, seq(
      field("quantifier", choice("and", "or", "allDiff")),
      "(",
      field("arg", choice($.matrix, $.tuple_matrix_index_or_slice, $.identifier)),
      ")"
    )),

    from_solution: $ => seq(
      "fromSolution",
      "(",
      field("variable", $.identifier),
      ")"
    ),

    comparison_expr: $ => prec(0, prec.left(seq(
      field("left", choice($.bool_expr, $.arithmetic_expr)), 
      field("operator", choice("=", "!=", "<=", ">=", "<", ">")),
      field("right", choice($.bool_expr, $.arithmetic_expr))
    ))),

    sub_bool_expr: $ => prec(1, seq("(", choice($.bool_expr, $.comparison_expr, $.atom), ")")),
    
    arithmetic_expr: $ => choice(
      field("atom", $.atom),
      field("negative_expression", $.negative_expr),
      field("absolute_value", $.abs_value),
      field("exponentiation", $.exponent),
      field("product_expression", $.product_expr),
      field("sum_expression", $.sum_expr),
      field("sub_arith_expression", $.sub_arith_expr),
      field("quantifier_expression_arith", $.quantifier_expr_arith),
    ),

    atom: $ => prec(-1, choice(
      field("constant", $.constant),
      field("variable", $.identifier),
      field("metavar", $.metavar),
      field("tuple", $.tuple),
      field("matrix", $.matrix),
      field("tuple_matrix_index_or_slice", $.tuple_matrix_index_or_slice),
    )),

    tuple: $ => prec(-5, seq(
      "(",
      field("element", $.arithmetic_expr),
      ",",
      field("element", commaSep1($.arithmetic_expr)),
      ")"
    )),

    matrix: $ => seq(
      "[",
      field("elements", commaSep1($.arithmetic_expr)),
      optional(seq(
        ";",
        choice($.int_domain, $.bool_domain) 
      )),
      "]"
    ),

    tuple_matrix_index_or_slice: $ => seq(
      field("tuple_or_matrix", choice($.identifier, $.tuple, $.matrix)),
      "[",
      field("indices", $.indices),
      "]"
    ),

    indices: $ => commaSep1(choice(field("index", $.arithmetic_expr), field("null_index", $.null_index))),

    null_index: $ => "..",

    sub_arith_expr: $ => seq("(", field("expression", $.arithmetic_expr), ")"),

    negative_expr: $ => prec(15, prec.left(seq("-", field("expression", $.arithmetic_expr)))),
    
    abs_value: $ => prec(20, seq("|", field("expression", $.arithmetic_expr), "|")),
    
    exponent: $ => prec(18, prec.right(seq(
      field("left", $.arithmetic_expr), 
      field("operator", "**"),
      field("right", $.arithmetic_expr)
    ))),

    product_expr: $ => prec(10, prec.left(seq(
      field("left", $.arithmetic_expr), 
      field("operator", $.mulitcative_op), 
      field("right", $.arithmetic_expr)
    ))),
    
    mulitcative_op: $ => choice("*", "/", "%"),
    
    sum_expr: $ => prec(1, prec.left(seq(
      field("left", $.arithmetic_expr), 
      field("operator", $.additive_op), 
      field("right", $.arithmetic_expr)
    ))),

    additive_op: $ => choice("+", "-"),

    quantifier_expr_arith: $ => prec(-10, seq(
      field("quantifier", choice("min", "max", "sum")),
      "(",
      field("arg", choice($.matrix, $.tuple_matrix_index_or_slice, $.identifier)),
      ")"
    )),

    dominance_relation: $ => seq(
      "dominanceRelation",
      field("expression", choice($.bool_expr, $.comparison_expr, $.arithmetic_expr)),
    )
  }
});

function commaSep1(rule) {
  return seq(rule, optional(repeat(seq(",", rule))), optional(","));
}

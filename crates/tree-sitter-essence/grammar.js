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
      field("find_statement_list", $.find_statement_list),
      field("constraint_list", $.constraint_list),
      field("letting_statement_list", $.letting_statement_list),
      field("dominance_relation", $.dominance_relation)
    )),

    single_line_comment: $ => token(seq('$', /.*/)),

    language_label: $ => token(seq("language", /.*/)),

    //general
    constant: $ => choice(
      $.integer,
      $.TRUE,
      $.FALSE
    ),

    // integer: $ => choice(/[0-9]+/, /-[0-9]+/),
    integer: $ => token(/[0-9]+/),

    TRUE: $ => choice("true", "TRUE"),

    FALSE: $ => choice("false", "FALSE"),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    //meta-variable (aka template argument)
    metavar: $ => seq("&", $.identifier),

    //find statements
    find_statement_list: $ => prec.right(seq("find", commaSep1($.find_statement))),

    find_statement: $ => seq(
      field("variables", $.variable_list),
      ":",
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

    int_domain: $ => prec.left(seq(
      "int",
      optional(seq(
        "(",
        field("ranges", $.range_list),
        ")"
      ))
    )),

    range_list: $ => prec(2, commaSep1(choice($.int_range, $.integer))),

    int_range: $ => seq(
      optional(field("start", $.arith_expression)), 
      "..", 
      optional(field("end", $.arith_expression))
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
    letting_statement_list: $ => prec.right(seq("letting", commaSep1($.letting_statement))),
    letting_statement: $ => seq(
      field("variable_list", $.variable_list), 
      "be", 
      optional("domain"),
      field("expr_or_domain", choice($.bool_expression, $.arith_expression, $.domain))
    ),

    //constraints
    constraint_list: $ => prec.right(seq(
      "such that", 
      commaSep1(choice($.bool_expression, $.atom)), 
    )),

    // Expressions
    bool_expression: $ => choice(
      field("sub_bool_expression", $.sub_bool_expr),
      field("not_expression", $.not_expr),
      field("and_expression", $.and_expr),
      field("or_expression", $.or_expr),
      field("implication", $.implication),
      field("iff_expr", $.iff_expr),
      field("quantifier_expression_bool", $.quantifier_expr_bool),
      field("from_solution", $.from_solution),
      field("comparison_expression", $.comparison_expr), 
    ),

    not_expr: $ => prec(20, seq("!", field("expression", choice($.bool_expression, $.atom)))),
    
    and_expr: $ => prec(-1, prec.left(seq(
      field("left", choice($.bool_expression, $.atom)), 
      field("operator", "/\\"),
      field("right", choice($.bool_expression, $.atom))
    ))),

    or_expr: $ => prec(-2, prec.left(seq(
      field("left", choice($.bool_expression, $.atom)),
      field("operator", "\\/"),
      field("right", choice($.bool_expression, $.atom))
    ))),
    
    implication: $ => prec(-4, prec.left(seq(
      field("left", choice($.bool_expression, $.atom)),
      field("operator", "->"), 
      field("right", choice($.bool_expression, $.atom)),
    ))),

    iff_expr: $ => prec(-4, prec.left(seq(
      field("left", choice($.bool_expression, $.atom)),
      field("operator", "<->"), 
      field("right", choice($.bool_expression, $.atom)),
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
      field("left", choice($.bool_expression, $.arith_expression)), 
      field("operator", choice("=", "!=", "<=", ">=", "<", ">")),
      field("right", choice($.bool_expression, $.arith_expression))
    ))),

    sub_bool_expr: $ => prec(1, seq("(", choice($.bool_expression, $.atom), ")")),

    atom: $ => prec(-1, choice(
      field("constant", $.constant),
      field("variable", $.identifier),
      field("metavar", $.metavar),
      field("tuple", $.tuple),
      field("matrix", $.matrix),
      field("tuple_matrix_index_or_slice", $.tuple_matrix_index_or_slice),
    )),

    // for now, tuples must be of arity 2+
    tuple: $ => prec(-5, seq(
      "(",
      field("element", $.arith_expression),
      ",",
      field("element", commaSep1($.arith_expression)),
      ")"
    )),

    matrix: $ => seq(
      "[",
      field("elements", commaSep1($.arith_expression)),
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

    indices: $ => commaSep1(choice($.arith_expression, field("null_index", $.null_index))),

    null_index: $ => "..",

    arith_expression: $ => choice(
      field("negative_expression", $.negative_expr),
      field("absolute_value", $.abs_value),
      field("exponentiation", $.exponent),
      field("product_expression", $.product_expr),
      field("sum_expression", $.sum_expr),
      field("sub_arith_expression", $.sub_arith_expr),
      field("atom", $.atom),
      field("quantifier_expression_num", $.quantifier_expr_num),
    ),

    negative_expr: $ => prec(15, prec.left(seq("-", field("expression", $.arith_expression)))),
    
    abs_value: $ => prec(20, seq("|", field("expression", $.arith_expression), "|")),
    
    exponent: $ => prec(18, prec.right(seq(
      field("left", $.arith_expression), 
      field("operator", "**"),
      field("right", $.arith_expression)
    ))),

    product_expr: $ => prec(10, prec.left(seq(
      field("left", $.arith_expression), 
      field("operator", $.mulitcative_op), 
      field("right", $.arith_expression)
    ))),
    
    mulitcative_op: $ => choice("*", "/", "%"),
    
    sum_expr: $ => prec(1, prec.left(seq(
      field("left", $.arith_expression), 
      field("operator", $.additive_op), 
      field("right", $.arith_expression)
    ))),

    additive_op: $ => choice("+", "-"),

    quantifier_expr_num: $ => prec(-10, seq(
      field("quantifier", choice("min", "max", "sum")),
      "(",
      field("arg", choice($.matrix, $.tuple_matrix_index_or_slice, $.identifier)),
      ")"
    )),

    sub_arith_expr: $ => seq(
      "(",
      field("expression", $.arith_expression), 
      ")"
    ),

    dominance_relation: $ => seq(
      "dominanceRelation",
      field("expression", $.bool_expression)
    )
  }
});

function commaSep1(rule) {
  return seq(rule, optional(repeat(seq(",", rule))), optional(","));
}

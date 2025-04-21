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

    integer: $ => choice(/[0-9]+/, /-[0-9]+/),

    TRUE: $ => "true",

    FALSE: $ => "false",

    variable: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    //meta-variable (aka template argument)
    metavar: $ => seq("&", $.variable),

    //find statements
    find_statement_list: $ => prec.right(seq("find", repeat1(field("find_statement", $.find_statement)))),

    find_statement: $ => seq(
      field("variables", $.variable_list),
      ":",
      field("domain", $.domain),
      optional(",")
    ),
    variable_list: $ => seq(
      field("variable", $.variable), 
      optional(repeat(seq(",", field("variable", $.variable))))
    ),
    domain: $ => choice(
      field("bool_domain", $.bool_domain),
      field("int_domain", $.int_domain),
      field("variable_domain", $.variable),
      field("tuple_domain", $.tuple_domain),
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

    range_list: $ => prec(2, seq(
      field("range_or_value", choice($.int_range, $.integer)),
      optional(repeat(seq(
        ",",
        field("range_or_value", choice($.int_range, $.integer))
      )))
    )),

    int_range: $ => seq(
      optional(field("start", $.expression)), 
      "..", 
      optional(field("end", $.expression))
    ),

    tuple_domain: $ => seq(
      optional("tuple"),
      "(",
      field("domain", $.domain),
      optional(repeat(seq(
        ",",
        field("domain", $.domain)
      ))),
      ")"
    ),

    //letting statements
    letting_statement_list: $ => prec.right(seq("letting", repeat1(field("letting_statement", $.letting_statement)))),
    letting_statement: $ => seq(
      field("variable_list", $.variable_list), 
      "be", 
      field("expr_or_domain", choice($.expression, seq("domain", $.domain)))
    ),

    //constraints
    constraint_list: $ => prec.right(seq(
      "such that", 
      field("expression", $.expression), 
      optional(repeat1(seq(",", field("expression", $.expression)))), 
      optional(",")
    )),

    // Expressions
    expression: $ => choice(
      field("not_expression", $.not_expr),
      field("and_expression", $.and_expr),
      field("or_expression", $.or_expr),
      field("implication", $.implication),
      field("quantifier_expression_bool", $.quantifier_expr_bool),
      field("from_solution", $.from_solution),
      field("comparison_expression", $.comparison_expr), 
      field("primary_expression", $.primary_expr),
      field("sub_expression", $.sub_expr),
    ),

    not_expr: $ => prec(20, seq("!", field("expression", $.expression))),
    
    and_expr: $ => prec(-1, prec.left(seq(
      field("left", $.expression), 
      field("operator", "/\\"),
      field("right", $.expression)
    ))),

    or_expr: $ => prec(-2, prec.left(seq(
      field("left", $.expression),
      field("operator", "\\/"),
      field("right", $.expression)
    ))),
    
    implication: $ => prec(-4, prec.left(seq(
      field("left", $.expression),
      field("operator", "->"), 
      field("left", $.expression),
    ))),

    quantifier_expr_bool: $ => prec(-10, seq(
      field("quantifier", choice("and", "or", "allDiff")),
      "([",
      repeat1(seq(field("expression", $.expression, optional(",")))),
      "])"
    )),

    from_solution: $ => seq(
      "fromSolution",
      "(",
      field("variable", $.variable),
      ")"
    ),

    comparison_expr: $ => prec(0, prec.left(seq(
      field("left", $.arithmetic_expr), 
      field("operator", $.comparative_op),
      field("right", $.arithmetic_expr)
    ))),

    comparative_op: $ => choice("=", "!=", "<=", ">=", "<", ">"),

    sub_expr: $ => seq("(", field("expression", $.expression), ")"),

    primary_expr: $ => choice(
      field("constant", $.constant),
      field("variable", $.variable),
      field("metavar", $.metavar),
      field("tuple_element", $.tuple_element),
      field("tuple", $.tuple),
      // field("matrix_element", $.matrix_element),
    ),

    tuple_element: $ => seq(
      field("tuple", choice($.variable, $.tuple)),
      "[",
      field("index", $.integer),
      "]"
    ),

    tuple: $ => seq(
      "(",
      field("element", $.arithmetic_expr),
      repeat1(seq(
        ",",
        field("element", $.arithmetic_expr)
      )),
      ")"
    ),

    // matrix_element: $ => seq(
    //   field("matrix", $.variable),
    //   "[",
    //   field("row_index", $.integer),
    // ),

    arithmetic_expr: $ => prec(-1, choice(
      field("negative_expression", $.negative_expr),
      field("absolute_value", $.abs_value),
      field("exponentiation", $.exponent),
      field("product_expression", $.product_expr),
      field("sum_expression", $.sum_expr),
      field("sub_arithmetic_expression", $.sub_arithmetic_expr),
      field("primary_expression", $.primary_expr),
      field("quantifier_expression_num", $.quantifier_expr_num),
    )),

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

    quantifier_expr_num: $ => prec(-10, seq(
      field("quantifier", choice("min", "max", "sum", "allDiff")),
      "([",
      repeat1(seq(field("expression", $.arithmetic_expr), optional(","))),
      "])"
    )),

    sub_arithmetic_expr: $ => seq(
      "(",
      field("expression", $.arithmetic_expr), 
      ")"
    ),
    
    additive_op: $ => choice("+", "-"),

    dominance_relation: $ => seq(
      "dominanceRelation",
      field("expression", $.expression)
    )
  }
});
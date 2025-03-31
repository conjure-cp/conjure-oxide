module.exports = grammar({
  name: 'essence',

  extras: $ => [
    $.single_line_comment,
    /\s/,
    $.language_label
  ],

  rules: {
    // Top-level statements
    program: $ => repeat(choice(
      field("find_statements", $.find_statement_list),
      field("constraints", $.constraint_list),
      field("letting_statements", $.letting_statement_list),
      field("dominance_relation", $.dominance_relation)
    )),

    single_line_comment: $ => token(seq('$', /.*/)),
    language_label: $ => token(seq("language", /.*/)),

    // Basic components
    constant: $ => choice(field("integer", $.integer), field("true", $.TRUE), field("false", $.FALSE)),
    integer: $ => /[0-9]+/,
    TRUE: $ => "true",
    FALSE: $ => "false",
    variable: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,
    // variable: $ => choice(
    //   /[a-zA-Z_][a-zA-Z0-9_]*/, $.reserved_keyword
    // ),
  
    // reserved_keyword: $ => choice(
    //   $.SUCH_THAT, $.FIND, $.LETTING
    // ),
    SUCH_THAT: $ => "such that",
    FIND: $ => "find",
    LETTING: $ => "letting",

    // Find statements
    // find_statement_list: $ => prec.right(seq(field("find", $.FIND), repeat(field("find_statement", $.find_statement)))),
    find_statement_list: $ => seq("find", repeat(field("find_statement", $.find_statement))),
    find_statement: $ => seq(
      field("variables", $.variable_list),
      field("colon", $.COLON),
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
      field("variable_domain", $.variable)
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

    // Letting statements
    // letting_statement_list: $ => prec.right(seq(field("letting", $.LETTING), repeat(field("letting_statement", $.letting_statement)))),
    letting_statement_list: $ => prec.right(seq(field("letting", $.LETTING), repeat(field("letting_statement", $.letting_statement)))),
    letting_statement: $ => seq(
      field("variable_list", $.variable_list), 
      "be", 
      choice(field("expression", $.expression), seq("domain", field("domain", $.domain)))
    ),

    // Constraints
    // constraint_list: $ => prec.right(seq(
      // field("such_that", $.SUCH_THAT), 
    // constraint_list: $ => seq(
    //   field("such_that", $.SUCH_THAT), 
    //   field("expression", $.expression), 
    //   optional(repeat(seq(",", field("expression", $.expression)))), 
    //   optional(",")
    // ),

    constraint_list: $ => seq(
      field("such_that", $.SUCH_THAT), 
      repeat(field("constraint", $.constraint))
    ),

    constraint: $ => seq(
      field("expression", $.expression), 
      optional(",")
    ),

    // Expression hierarchy
    expression: $ => choice(
      field("boolean_expression", $.boolean_expr), 
      field("comparison_expression", $.comparison_expr), 
      field("arithmetic_expression", $.arithmetic_expr)
    ),
    
    boolean_expr: $ => choice(
      field("not_expression", $.not_expr),
      field("and_expression", $.and_expr),
      field("or_expression", $.or_expr),
      field("implication", $.implication),
      field("quantifier_expression", $.quantifier_expr),
      field("from_solution", $.from_solution)
    ),

    not_expr: $ => prec(20, seq("!", field("expression", choice($.boolean_expr, $.comparison_expr, $.primary_expr)))),
    
    and_expr: $ => prec(-1, prec.left(seq(
      field("left", choice($.boolean_expr, $.comparison_expr, $.primary_expr)), 
      "/\\", 
      field("right", choice($.boolean_expr, $.comparison_expr, $.primary_expr))
    ))),
    
    or_expr: $ => prec(-2, prec.left(seq(
      field("left", choice($.boolean_expr, $.comparison_expr, $.primary_expr)), 
      "\\/", 
      field("right", choice($.boolean_expr, $.comparison_expr, $.primary_expr))
    ))),
    
    implication: $ => prec(-4, prec.left(seq(
      field("left", choice($.boolean_expr, $.comparison_expr, $.primary_expr)), 
      "->", 
      field("right", choice($.boolean_expr, $.comparison_expr, $.primary_expr))
    ))),

    quantifier_expr: $ => prec(-10, seq(
      field("quantifier", choice("and", "or", "min", "max", "sum", "allDiff")),
      "([",
      repeat(seq(field("expression", $.expression), optional(","))),
      "])"
    )),

    from_solution: $ => seq(
      "fromSolution",
      "(",
      field("variable", $.variable),
      ")"
    ),

    comparison_expr: $ => prec(0, prec.left(seq(
      field("left", choice($.boolean_expr, $.arithmetic_expr)), 
      field("operator", $.comparative_op),
      field("right", choice($.boolean_expr, $.arithmetic_expr))
    ))),

    comparative_op: $ => choice("=", "!=", "<=", ">=", "<", ">"),

    arithmetic_expr: $ => choice(
      field("primary_expression", $.primary_expr),
      field("negative_expression", $.negative_expr),
      field("absolute_value", $.abs_value),
      field("exponentiation", $.exponent),
      field("product_expression", $.product_expr),
      field("sum_expression", $.sum_expr)
    ),

    primary_expr: $ => choice(
      field("sub_expression", $.sub_expr),
      field("constant", $.constant),
      field("variable", $.variable)
    ),

    sub_expr: $ => seq("(", field("expression", $.expression), ")"),

    negative_expr: $ => prec(15, prec.left(seq("-", field("expression", $.arithmetic_expr)))),
    
    abs_value: $ => prec(20, seq("|", field("expression", $.arithmetic_expr), "|")),
    
    exponent: $ => prec(18, prec.right(seq(
      field("base", $.arithmetic_expr), 
      "**", 
      field("exponent", $.arithmetic_expr)
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

    dominance_relation: $ => seq(
      "dominanceRelation",
      field("expression", $.expression)
    )
  }
});
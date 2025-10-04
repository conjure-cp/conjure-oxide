module.exports = grammar ({
  name: 'essence',

  extras: $ => [
    $.single_line_comment,
    /\s/,
    $.language_declaration
  ],

  rules: {
    //top level statements
    program: $ => repeat(choice(
      $.find_statement_list,
      $.constraint_list,
      $.letting_statement_list,
      $.dominance_relation
    )),

    single_line_comment: $ => token(seq('$', /.*/)),

    language_declaration: $ => token(seq("language", /.*/)),

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
    find_statement_list: $ => seq("find", repeat($.find_statement)),

    find_statement: $ => seq(
      $.variable_list,
      ":",
      $.domain,
      optional(",")
    ),

    variable_list: $ => seq(
      $.variable,
      optional(repeat(seq(
        ",",
        $.variable
      )))
    ),

    domain: $ => choice(
      $.bool_domain,
      $.int_domain,
      $.variable
    ),

    bool_domain: $ => "bool",

    int_domain: $ => prec.left(seq(
      "int",
      optional(seq(
        "(",
        $.range_list,
        //TODO: eventually add in expressions here
        ")"
      ))
    )),

    range_list: $ => prec(2, seq(
      choice(
        $.int_range,
        $.integer
      ),
      optional(repeat(seq(
        ",",
        choice(
          $.int_range,
          $.integer
        ),
      )))
    )),

    int_range: $ => seq(optional($.expression), "..", optional($.expression)),

    //letting statements
    letting_statement_list: $ => seq("letting", repeat($.letting_statement)),

    letting_statement: $ => seq(
      $.variable_list,
      "be",
      choice($.expression, seq("domain", $.domain))
    ),

    //constraints
    constraint_list: $ => seq("such that", repeat(seq($.expression, optional(",")))),

    expression: $ => choice(
      seq("(", $.expression, ")"),
      $.metavar,
      $.not_expr,
      $.abs_value,
      $.exponent,
      $.negative_expr,
      $.product_expr,
      $.sum_expr,
      $.comparison,
      $.and_expr,
      $.or_expr,
      $.implication,
      $.quantifier_expr,
      $.constant,
      $.variable,
      $.from_solution
    ),

    not_expr: $ => prec(20, seq("!", $.expression)),

    abs_value: $ => prec(20, seq("|", $.expression, "|")),

    exponent: $ => prec(18, prec.right(seq($.expression, "**", $.expression))),

    negative_expr: $ => prec(15, prec.left(seq("-", $.expression))),

    product_expr: $ => prec(10, prec.left(seq($.expression, $.multiplicative_op, $.expression))),

    multiplicative_op: $ => choice("*", "/", "%"),

    sum_expr: $ => prec(1, prec.left(seq($.expression, $.additive_op, $.expression))),

    additive_op: $ => choice("+", "-"),

    comparison: $ => prec(0, prec.left(seq($.expression, $.comp_op, $.expression))),

    comp_op: $ => choice("=", "!=", "<=", ">=", "<", ">"),

    and_expr: $ => prec(-1, prec.left(seq($.expression, "/\\", $.expression))),

    or_expr: $ => prec(-2, prec.left(seq($.expression, "\\/", $.expression))),

    implication: $ => prec(-4, prec.left(seq($.expression, "->", $.expression))),

    toInt_expr: $ => seq("toInt","(",$.expression,")"),

    quantifier_expr: $ => prec(-10, seq(
      choice("and", "or", "min", "max", "sum", "allDiff"),
      "([",
      repeat(seq(
        $.expression,
        optional(",")
      )),
      "])"
    )),

    from_solution: $ => seq(
      "fromSolution",
      "(",
      $.variable,
      ")"
    ),

    dominance_relation: $ => seq(
      "dominanceRelation",
      $.expression
    )
  }
})

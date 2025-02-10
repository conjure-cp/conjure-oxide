module.exports = grammar ({
  name: 'essence',

  extras: $ => [
    $.single_line_comment,
    /\s/,
    $.e_prime_label
  ],

  rules: {
    //top level statements
    program: $ => repeat(choice(
      $.find_statement_list,
      $.constraint_list,
      $.letting_statement_list,
    )),

    single_line_comment: $ => token(seq('$', /.*/)),

    e_prime_label: $ => token("language ESSENCE' 1.0"),

    //general
    constant: $ => choice(
      $.integer,
      $.TRUE,
      $.FALSE
    ),

    integer: $ => /[0-9]+/,

    TRUE: $ => "true",

    FALSE: $ => "false",

    variable: $ => $.identifier,

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    //find statements
    find_statement_list: $ => seq("find", repeat($.find_statement)),

    find_statement: $ => seq(
      field("variable_list", $.variable_list),
      ":",
      field("domain", $.domain),
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
        field("range_list", $.range_list),
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
    constraint_list: $ => seq("such that", repeat($.constraint)),

    constraint: $ => seq($.expression, optional(",")),

    expression: $ => choice(
      $.unary_minus_expr,
      $.or_expr,
      $.and_expr,
      $.comparison,
      $.math_expr,
      $.not_expr,
      $.sub_expr,
      $.min,
      $.max,
      $.sum,
      $.all_diff,
      $.constant,
      $.variable,
      $.abs_value,
      $.imply_expr
    ),

    unary_minus_expr: $ => prec(3, prec.left(seq("-", $.expression))),
    
    or_expr: $ => prec.left(choice(
      seq($.expression, "\\/", $.expression),
      seq(
        "or([",
        repeat(seq(
          $.expression,
          optional(",")
        )),
        "])"
      )
    )),

    and_expr: $ => prec.left(choice(
      seq($.expression, "/\\", $.expression),
      seq(
        "and[",
        repeat(seq(
          $.expression,
          optional(",")
        )),
        "])"
      )
    )),

    comparison: $ => prec(1, prec.left(seq($.expression, $.comp_op, $.expression))),

    comp_op: $ => choice(
      "=",
      "!=",
      "<=",
      ">=",
      "<",
      ">"
    ),

    math_expr: $ => prec(2, prec.left(seq($.expression, $.math_op, $.expression))),

    math_op: $ => choice(
      "+",
      "-",
      "*",
      "/", 
      "%",
      "**"
    ),

    not_expr: $ => prec(2, prec.left(seq("!", $.expression))),

    sub_expr: $ => seq("(", $.expression, ")"),

    min: $ => seq(
      "min([",
      repeat(seq(
        choice($.variable, $.constant),
        ","
      )),
      "])"
    ),

    max: $ => seq(
      "max([",
      repeat(seq(
        choice($.variable, $.constant),
        ","
      )),
      "])"
    ),

    sum: $ => seq(
      "sum([",
      repeat(seq(
        $.expression,
        optional(",")
      )),
      "])"
    ),

    all_diff: $ => seq(
      "allDiff([",
      repeat(seq(
        $.expression,
        optional(",")
      )),
      "])"
    ),

    abs_value: $ => seq(
      "|",
      $.expression,
      "|"
    ),

    imply_expr: $ => prec.left(seq(
      $.expression,
      "->",
      $.expression
    ))
  }
})
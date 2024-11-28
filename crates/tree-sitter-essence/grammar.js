module.exports = grammar ({
  name: 'essence',

  extras: $ => [
    $.single_line_comment,
    /\s/
  ],

  rules: {
    //top level statements
    program: $ => repeat(choice(
      $.find_statement_list,
      $.constraint_list
    )),

    single_line_comment: $ => token(seq(
      '$',
      /.*/
    )),

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
    find_statement_list: $ => seq(
      "find",
      repeat($.find_statement)
    ),

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
      $.int_domain
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

    int_range: $ => seq(
      optional($.expression),
      "..",
      optional($.expression)
    ),

    //constraints
    constraint_list: $ => seq(
      "such that",
      repeat($.constraint)
    ),

    constraint: $ => seq(
      optional($.not),
      $.expression,
      optional(",")
    ),

    not: $ => "!",

    expression: $ => prec.left(choice(
      field("or_expr", seq($.expression, "\\/", $.expression)),
      field("and_expr", seq($.expression, "/\\", $.expression)),
      prec(1, field("comp_op_expr", seq($.expression, $.comp_op, $.expression))),
      prec(2, field("math_op_expr", seq($.expression, $.math_op, $.expression))),
      field("min_expr", $.min),
      field("max_expr", $.max),
      field("sum_expr", $.sum),
      field("constant", $.constant),
      field("variable", $.variable),
      field("sub_expr", seq("(", $.expression, ")"))
    )),

    

    comp_op: $ => choice(
      "=",
      "!=",
      "<=",
      ">=",
      "<",
      ">"
    ),

    math_op: $ => choice(
      "+",
      "-",
      "*",
      "/", 
      "%"
    ),

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
    )
  }
})
module.exports = grammar ({
  name: 'essence',

  extras: $ => [
    $.single_line_comment,
    /\s/
  ],

  rules: {
    program: $ => repeat(choice(
      $.find_statement_list,
      $.constraint_list
    )),

    single_line_comment: $ => token(seq(
      '$',
      /.*/
    )),

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

    domain: $ => choice(
      $.int_domain,
      $.bool_domain
    ),

    int_domain: $ => prec.left(seq(
      "int",
      optional(seq(
        "(",
        field("range_list", $.range_list),
        //eventually add in expressions here
        ")"
      ))
    )),

    bool_domain: $ => "bool",

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
    
    //should these be expresions not integers?
    lower_bound_range: $ => seq(
      $.integer,
      ".."
    ),

    upper_bound_range: $ => seq(
      "..",
      $.integer
    ),
    
    closed_range: $ => seq(
      $.integer,
      "..",
      $.integer
    ),

    variable: $ => $.identifier,

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    expression: $ => seq(
      $.conjunction,
      optional(repeat(seq(
        "or",
        $.conjunction
      )))
    ),

    conjunction: $ => seq(
      $.comparison,
      optional(repeat(seq(
        "and",
        $.comparison
      )))
    ),

    comparison: $ => seq(
      $.addition,
      optional(repeat(seq(
        $.comp_op,
        $.addition
      )))
    ),

    addition: $ => seq(
      $.term,
      optional(repeat(seq(
        choice(
          "+",
          "-"
        ),
        $.term
      )))
    ),

    term: $ => seq(
      $.factor,
      optional(repeat(seq(
        choice(
          "*",
          "/",
          "%"
        ),
        $.factor
      )))
    ),

    factor: $ => prec.left(choice(
      seq(
        "(",
        $.expression,
        ")"
      ),
      $.min,
      $.max,
      $.constant,
      $.variable,
      $.sum
    )),

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
        $.factor,
        optional(",")
      )),
      "])"
    ),

    TRUE: $ => "true",

    FALSE: $ => "false",

    comp_op: $ => choice(
      "=",
      "!=",
      "<=",
      ">=",
      "<",
      ">"
    ),

    constant: $ => choice(
      $.integer,
      $.TRUE,
      $.FALSE
    ),

    integer: $ => /[0-9]+/
  }
})
module.exports = grammar ({
  name: 'essence',

  //next: look into adding field names 

  extras: $ => [
    $.single_line_comment,
    /\s/
  ],

  rules: {
    program: $ => repeat(choice(
      $.find_statement_list,
      $.constraint_list,
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
        $.range_list,
        //eventually add in expressions here
        ")"
      ))
    )),

    bool_domain: $ => "bool",

    range_list: $ => prec(2, seq(
      choice(
      $.lower_bound_range,
      $.upper_bound_range,
      $.closed_range,
      $.integer
      ),
      optional(repeat(seq(
        ",",
        choice(
          $.lower_bound_range,
          $.upper_bound_range,
          $.closed_range,
          $.integer
        )
      )))
    )),
    
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
          "/"
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
    )),

    min: $ => seq(
      "min([",
      repeat(seq(
        $.variable,
        ","
      )),
      "])"
    ),

    max: $ => seq(
      "max([",
      repeat(seq(
        $.variable,
        ","
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

// /**
//  * @file WIP parser for essence
//  * @author Leia McAlister-Young <l.mcalisteryoung@gmail.com>
//  * @license Mozilla Public License
//  */
// module.exports = grammar({
//   name: 'essence',

//   rules: {
//     program: $ => repeat(choice(
//       $.declaration_statement,
//       $.branching_statement,
//       $.such_that_statement,
//       $.where_statement,
//       $.objective_statement
//     )),

//     // statement: $ => choice(
//     //   $.declaration_statement,
//     //   $.branching_statement,
//     //   $.such_that_statement,
//     //   $.where_statement,
//     //   $.objective_statement
//     // ),

//     declaration_statement: $ => choice(
//       $.find_statement,
//       $.given_statement,
//       $.letting_statement,
//       $.given_enum,
//       $.letting_enum,
//       $.letting_unnamed
//     ),

//     find_statement: $ => seq(
//       "find",
//       repeat(seq(
//           $.identifier,
//           optional(",")
//       )),
//       ":",
//       $.domain_declaration
//     ),

//     given_statement: $ => seq(
//       "given",
//       repeat($.identifier),
//       ":",
//       $.domain_declaration
//     ),

//     letting_statement: $ => seq(
//       "letting",
//       $.identifier,
//       "be",
//       choice(
//         $.expression,
//         seq(
//           "domain",
//           $.domain_declaration
//         )
//       )
//     ),

//     given_enum: $ => seq(
//       "given",
//       $.identifier,
//       "new type enum"
//     ),

//     letting_enum: $ => seq(
//       "letting",
//       $.identifier,
//       "be new type enum",
//       $.list
//     ),

//     list: $ => seq(
//       "list placeholder",
//       repeat($.expression)
//     ),
    
//     letting_unnamed: $ => seq(
//       "letting",
//       $.identifier,
//       "be new type of size",
//       $.expression
//     ),

//     branching_statement: $ => seq(
//       "branching on",
//       $.list
//     ),

//     such_that_statement: $ => seq(
//       "such that",
//       $.expression,
//       optional(seq(
//         ",",
//         $.expression
//       ))
//     ),

//     where_statement: $ => seq(
//       "where",
//       $.list
//     ),

//     objective_statement: $ => choice(
//       seq(
//         "minimising",
//         $.expression
//       ),
//       seq(
//         "maximising",
//         $.expression
//       )
//     ),

//     domain_declaration: $ => choice(
//       "bool",
//       $.int_range,
//       $.enum_range,
//       $.tuple_range,
//       $.record_range,
//       $.variant_range,
//       $.matrix_domain,
//       $.set_domain,
//       $.mset_domain,
//       $.function_domain,
//       $.sequence_domain,
//       //$.relation_domain
//     ),

//     int_range: $ => prec.left(seq(
//       "int",
//       optional(seq(
//           "(",
//           choice(
//             $.expression,
//             $.lower_bound_range,
//             $.upper_bound_range,
//             $.closed_range
//           ),
//           ")"
//       ))
//     )),

//     lower_bound_range: $ => seq(
//       $.expression,
//       ".."
//     ),
//     upper_bound_range: $ => seq(
//       "..",
//       $.expression
//     ),

//     closed_range: $ => seq(
//       $.expression,
//       "..",
//       $.expression
//     ),

//     enum_range: $ => prec.left(seq(
//       $.identifier,
//       optional( seq(
//         "(",
//         choice(
//           $.expression, //enum identifier
//           $.lower_bound_range,
//           $.upper_bound_range,
//           $.closed_range
//         ),
//         ")"
//       ))
//     )),
    
//     tuple_range: $ => seq(
//       optional("tuple"), //only optional if tuple has more than 2 elements i belive?
//       "(",
//       repeat(seq(
//         $.domain_declaration, //or (domain) identifier?
//         optional(",")
//       )),
//       ")"
//     ),

//     record_range: $ => seq(
//       "record",
//       "{",
//       repeat(seq(
//         $.name_domain_pair,
//         optional(",")
//       )),
//       "}"
//     ),

//     name_domain_pair: $ => seq(
//       $.identifier, //name of domain
//       ":",
//       $.domain_declaration //domain identifier possibly
//     ),

//     variant_range: $ => seq(
//       "variant",
//       "{",
//       repeat(seq(
//         $.name_domain_pair,
//         optional(",")
//       )),
//       "}"
//     ),

//     matrix_domain: $ => seq(
//       "matrix indexed by",
//       "[",
//       repeat(seq(
//         $.domain_declaration,
//         optional(",")
//       )),
//       "]",
//       "of",
//       $.domain_declaration
//     ),

//     set_domain: $ => seq(
//       "set",
//       optional(seq(
//           "(",
//           repeat(seq(
//             $.attribute, //set attribute
//             optional(",")
//           )),
//           ")"
//       )),
//       "of",
//       $.domain_declaration //domain for members of the set
//     ),

//     mset_domain: $ => seq( //fix this whole thing
//       "mset",
//       optional(repeat(seq(
//         $.attribute, //mset_attribute
//         optional(",")
//       )))
//     ),

//     function_domain: $ => seq(
//       "function",
//       optional(seq(
//         "(", //check which type of bracket it is
//         repeat(seq(
//           $.attribute, //function attribute
//           optional(",")
//         )),
//         ")"
//       )),
//       $.domain_declaration, //domain identifier
//       "-->",
//       $.domain_declaration //domain identifier
//     ),

//     sequence_domain: $ => seq(
//       "sequence",
//       optional(seq(
//         "(", //check bracket type, if any
//         repeat(seq(
//           $.attribute,
//           optional(",")
//         )),
//         ")"
//       )),
//       "of",
//       $.domain_declaration //domain for members of the sequence
//     ),

//     attribute: $ => choice(
//       $.cardinality_attribute,
//       $.num_occurences_attribute,
//       $.partiality_attribute,
//       $.function_property_attribute
//     ),

//     cardinality_attribute: $ => choice( 
//       "size",
//       "minSize",
//       "maxSixe"
//     ),

//     num_occurences_attribute: $ => choice(
//       "minOccur",
//       "maxOccur" 
//     ),

//     partiality_attribute: $ => "total",

//     function_property_attribute: $ => choice(
//       "injective",
//       "surjective",
//       "bijective"
//     ),

//     expression: $ => prec.left(choice(
//       $.assignment_expression, //move this to be its own thing
//       $.literal,
//       //$.operator,
//       $.identifier,
//       $.arithmetic_expression,
//       $.comparison_expression,
//       $.logical_expression,
//       $.parenthesized_expression
//     )),

//     literal: $ => choice(
//       $.decimal_integer_literal,
//       $.true,
//       $.false,
//       //$.character_literal,
//     ),

//     decimal_integer_literal: $ => /\d+/,

//     true: _ => "true",
//     false: _ => "false",

//     assignment_expression: $ => seq(
//       $.identifier,
//       "=",
//       $.decimal_integer_literal
//     ),

//     arithmetic_expression: $ => prec.left(seq(
//       $.expression,
//       $.arithmetic_operator,
//       $.expression
//     )),

//     comparison_expression: $ => prec.left(seq(
//       $.expression,
//       $.comparison_operator,
//       $.expression
//     )),

//     logical_expression: $ => prec.left(seq(
//       $.expression,
//       $.logical_operator,
//       $.expression
//     )),

//     parenthesized_expression: $ => prec.left(seq(
//       "(",
//       $.expression,
//       ")"
//     )),

//     arithmetic_operator: $ => choice(
//       "+",
//       "-",
//       "*",
//       "/",
//     ),

//     comparison_operator: $ => choice(
//       "==",
//       "!=",
//       "<",
//       ">",
//       "<=",
//       ">=",
//       "="
//     ),

//     logical_operator: $ => choice(
//       "&&",
//       "||"
//     ),

//     identifier: $ => /[\p{XID_Start}_$][\p{XID_Continue}\u00A2_$]*/
//   }
// })

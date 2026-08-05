module.exports = grammar ({
  name: 'essence',

  extras: $ => [
    $.single_line_comment,
    /\s/,
    $.language_declaration
  ],

  rules: {
    // Top-level statements
    program: $ => repeat(choice(
      seq(
        // Single Essence expression.
        // Not part of Essence syntax but used to parse Essence fragments outside a full Essence file.
        "_FRAGMENT_EXPRESSION",
        commaSep1(choice(
          field("atom", $.atom),
          field("bool_expr", $.bool_expr),
          field("comparison_expr", $.comparison_expr),
          field("arithmetic_expr", $.arithmetic_expr),
          field("annotation_expr", $.annotation_expr)
        ))
      ),
      field("find_statement", $.find_statement),
      field("given_statement", $.given_statement),
      seq(
        field("such_that_keyword", "such that"),
        commaSep1($._constraint_expression),
      ),
      field("where_statement", $.where_statement),
      field("letting_statement", $.letting_statement),
      field("objective_statement", $.objective_statement),
      field("dominance_relation", $.dominance_relation),
    )),

    single_line_comment: $ => token(seq('$', /.*/)),

    language_declaration: $ => token(seq("language", /.*/)),

    _constraint_expression: $ => choice(
      field("bool_expr", $.bool_expr),
      field("atom", $.atom),
      field("comparison_expr", $.comparison_expr)
    ),

    where_statement: $ => seq(
      field("where_keyword", "where"),
      commaSep1($._constraint_expression)
    ),

    //general
    constant: $ => choice(
      field("integer", $.integer),
      field("true", $.TRUE),
      field("false", $.FALSE)
    ),

    integer: $ => /[0-9]+/,

    TRUE: $ => choice("true", "TRUE"),

    FALSE: $ => choice("false", "FALSE"),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    //meta-variable (aka template argument)
    metavar: $ => seq("&", field("identifier", $.identifier)),

    //find / findAux statements (findAux before find so the longer keyword wins)
    find_statement: $ => seq(
      field("find_keyword", choice("findAux", "find")),
      field("variable_declaration", commaSep1($.variable_declaration))
    ),

    //given statements
    given_statement: $ => seq(
      field("given_keyword", "given"),
      field("variable_declaration", commaSep1($.variable_declaration))
    ),

    COLON: $ => ":",
    
    variable_declaration: $ => seq(
      field("variables", $.variable_list), 
      field("colon", $.COLON), 
      field("domain", $.domain)
    ),

    variable_list: $ => commaSep1($.identifier),

    domain: $ => choice(
      field("bool_domain", $.bool_domain),
      field("int_domain", $.int_domain),
      field("variable_domain", $.identifier),
      field("tuple_domain", $.tuple_domain),
      field("matrix_domain", $.matrix_domain),
      field("record_domain", $.record_domain),
      field("variant_domain", $.variant_domain),
      field("set_domain", $.set_domain),
      field("mset_domain", $.mset_domain),
      field("sequence_domain", $.sequence_domain),
      field("function_domain", $.function_domain),
      field("relation_domain", $.relation_domain),
    ),
    bool_domain: $ => "bool",

    int_domain: $ => seq(
      "int",
      optional(seq(
        "(",
        optional(field("ranges", $.range_list)),
        ")"
      ))
    ),

    range_list: $ => prec(2, commaSep1(choice($.int_range, $.atom, $.arithmetic_expr))),

    int_range: $ => seq(
      optional(field("lower", choice($.atom, $.arithmetic_expr))), 
      "..", 
      optional(field("upper", choice($.atom, $.arithmetic_expr)))
    ),

    tuple_domain: $ => choice(
      // explicit 'tuple' form: allows empty, singleton, or multi-arity
      seq(
        "tuple",
        "(",
        optional(commaSep1($.domain)),
        ")"
      ),
      // parenthesized form without 'tuple' is allowed only for arity >= 2
      seq(
        "(",
        $.domain,
        ",",
        commaSep1($.domain),
        ")"
      )
    ),

    matrix_domain: $ => seq(
      "matrix",
      optional("indexed"),
      optional("by"),
      optional("indexed"),
      optional("by"),
      "[",
      field("index_domain_list", $.index_domain_list),
      "]",
      "of",
      field("value_domain", $.domain)
    ),

    record_domain: $ => seq(
      "record",
      "{",
      commaSep1(field("name_domain_pair", $.name_domain_pair)),
      "}"
    ),

    variant_domain: $ => seq(
      "variant",
      "{",
      commaSep1(field("name_domain_pair", $.name_domain_pair)),
      "}"
    ),

    set_domain: $ => seq(
        "set",
        optional(seq("{", field("representation", $.identifier), "}")),
        optional(seq("(", $.set_attributes, ")")),
        "of",
        field("value_domain", $.domain)
    ),

    set_attributes: $ => choice(
      seq(
        field("attribute", "size"),
        field("size_value", $.integer)
      ),
      seq(
        field("attribute", "minSize"),
        field("min_value", $.integer)
      ),
      seq(
        field("attribute", "maxSize"),
        field("max_value", $.integer)
      ),
      seq(
        field("attribute", "minSize"),
        field("min_value", $.integer),
        ",",
        field("attribute", "maxSize"),
        field("max_value", $.integer)
      )
    ),

    mset_domain: $ => seq(
      "mset",
      optional(seq("{", field("representation", $.identifier), "}")),
      optional(seq("(", $.mset_attributes, ")")),
      "of",
      field("value_domain", $.domain)
    ),

    mset_attributes: $ => commaSep1($.mset_attribute),

    mset_attribute: $ => choice(
      seq(field("attribute", "size"), field("value", $.integer)),
      seq(field("attribute", "minSize"), field("value", $.integer)),
      seq(field("attribute", "maxSize"), field("value", $.integer)),
      seq(field("attribute", "minOccur"), field("value", $.integer)),
      seq(field("attribute", "maxOccur"), field("value", $.integer))
    ),

    // e.g. sequence (size 2) of int(1..3), sequence (maxSize 3) of int(0..2),
    // sequence (minSize 1, maxSize 2) of int(1..3), or bare sequence of int(1..3).
    // Note: the grammar itself does not require a size bound here; that is
    // enforced downstream by domain-construction logic, mirroring set/mset.
    sequence_domain: $ => seq(
      "sequence",
      optional(seq("(", $.sequence_attributes, ")")),
      "of",
      field("value_domain", $.domain)
    ),

    sequence_attributes: $ => commaSep1($.sequence_attribute),

    sequence_attribute: $ => choice(
      seq(field("attribute", "size"), field("value", $.integer)),
      seq(field("attribute", "minSize"), field("value", $.integer)),
      seq(field("attribute", "maxSize"), field("value", $.integer)),
      field("attribute", "injective"),
      field("attribute", "surjective"),
      field("attribute", "bijective")
    ),

    // e.g. function int(1..3) --> int(1..3), function (total) bool --> int(13,17),
    // function (minSize 1) int(1..2) --> set (size 1) of int(1..2). Attribute values may
    // optionally be parenthesised (e.g. `size(2)`, `minSize(1)`), matching Conjure's own
    // grammar where the attribute value is just an integer expression.
    function_domain: $ => seq(
      "function",
      optional(seq("(", $.function_attributes, ")")),
      field("domain_from", $.domain),
      "-->",
      field("domain_to", $.domain)
    ),

    function_attributes: $ => commaSep1($.function_attribute),

    function_attribute: $ => choice(
      seq(field("attribute", "size"), choice(field("value", $.integer), seq("(", field("value", $.integer), ")"))),
      seq(field("attribute", "minSize"), choice(field("value", $.integer), seq("(", field("value", $.integer), ")"))),
      seq(field("attribute", "maxSize"), choice(field("value", $.integer), seq("(", field("value", $.integer), ")"))),
      field("attribute", "total"),
      field("attribute", "injective"),
      field("attribute", "surjective"),
      field("attribute", "bijective")
    ),

    // e.g. relation of (int(1..3) * bool), relation (size 2, irreflexive) of (int(0..5) * int(0..5)),
    // relation of (int(1..3) * int(1..5) * bool). Columns are separated by "*", matching Conjure's
    // own relation-domain syntax; unlike matrix's index_domain_list this is not comma-separated.
    relation_domain: $ => seq(
      "relation",
      optional(seq("(", $.relation_attributes, ")")),
      "of",
      "(",
      field("relation_domain_list", $.relation_domain_list),
      ")"
    ),

    relation_domain_list: $ => starSep1($.domain),

    relation_attributes: $ => commaSep1($.relation_attribute),

    relation_attribute: $ => choice(
      seq(field("attribute", "size"), field("value", $.integer)),
      seq(field("attribute", "minSize"), field("value", $.integer)),
      seq(field("attribute", "maxSize"), field("value", $.integer)),
      field("attribute", "reflexive"),
      field("attribute", "irreflexive"),
      field("attribute", "coreflexive"),
      field("attribute", "symmetric"),
      field("attribute", "antiSymmetric"),
      field("attribute", "aSymmetric"),
      field("attribute", "transitive"),
      field("attribute", "total"),
      field("attribute", "connex"),
      field("attribute", "Euclidean"),
      field("attribute", "serial"),
      field("attribute", "equivalence"),
      field("attribute", "partialOrder")
    ),

    set_literal: $ => seq(
      "{",
      field("element", commaSep1(choice($.bool_expr, $.arithmetic_expr, $.comparison_expr, $.atom))),
      "}"
    ),

    mset_literal: $ => seq(
      "mset",
      "(",
      optional(field("element", commaSep1(choice($.bool_expr, $.arithmetic_expr, $.comparison_expr, $.atom)))),
      ")"
    ),

    sequence_literal: $ => seq(
      "sequence",
      "(",
      optional(field("element", commaSep1(choice($.bool_expr, $.arithmetic_expr, $.comparison_expr, $.atom)))),
      ")"
    ),

    // e.g. function(), function(0-->1,1-->0), function(false --> 5, true --> x)
    function_literal: $ => seq(
      "function",
      "(",
      optional(field("pair", commaSep1($.function_literal_pair))),
      ")"
    ),

    function_literal_pair: $ => seq(
      field("key", choice($.bool_expr, $.arithmetic_expr, $.atom)),
      "-->",
      field("value", choice($.bool_expr, $.arithmetic_expr, $.atom))
    ),

    // e.g. relation(), relation((2,6),(1,4)), relation((1,false,true),(2,false,false)).
    // Each element is itself a parenthesised tuple of >= 2 values, one per column.
    relation_literal: $ => seq(
      "relation",
      "(",
      optional(field("tuple", commaSep1($.relation_literal_tuple))),
      ")"
    ),

    relation_literal_tuple: $ => seq(
      "(",
      field("element", commaSep1(choice($.bool_expr, $.arithmetic_expr, $.comparison_expr, $.atom))),
      ")"
    ),

    // Function operators, all function-call style: keyword immediately followed by a
    // parenthesised, comma-separated argument list (not infix operators).
    image_expr: $ => seq(
      "image",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ",",
      field("argument", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    image_set_expr: $ => seq(
      "imageSet",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ",",
      field("argument", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    pre_image_expr: $ => seq(
      "preImage",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ",",
      field("argument", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    inverse_expr: $ => seq(
      "inverse",
      "(",
      field("left", choice($.arithmetic_expr, $.atom)),
      ",",
      field("right", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    // restrict's second argument is a domain, not an expression.
    restrict_expr: $ => seq(
      "restrict",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ",",
      field("domain", $.domain),
      ")"
    ),

    defined_expr: $ => seq(
      "defined",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    range_expr: $ => seq(
      "range",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    to_set_expr: $ => seq(
      "toSet",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    to_mset_expr: $ => seq(
      "toMSet",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    to_relation_expr: $ => seq(
      "toRelation",
      "(",
      field("function", choice($.arithmetic_expr, $.atom)),
      ")"
    ),

    name_domain_pair: $ => seq(
      field("name", $.identifier),
      ":",
      field("domain", $.domain)
    ),

    index_domain_list: $ => commaSep1($.domain),

    //letting statements
    letting_statement: $ => seq(
      field("letting_keyword", "letting"),
      field("letting_variable_declaration", commaSep1($.letting_variable_declaration))
    ),

    letting_variable_declaration: $ => seq(
      field("variable_list", $.variable_list), 
      field("be", "be"), 
      choice(
        field("expr_or_domain", choice($.bool_expr, $.arithmetic_expr, $.atom)),
        seq(
          field("domain", "domain"),
          field("expr_or_domain", $.domain)
        )
      )
    ),

    // Constraints 
    // boolean expressions require the operands to be boolean and return a boolean
    bool_expr: $ => prec(2, choice(
      field("not_expression", $.not_expr),
      field("and_expression", $.and_expr),
      field("or_expression", $.or_expr),
      field("implication", $.implication),
      field("iff_expr", $.iff_expr),
      field("list_combining_expression_bool", $.list_combining_expr_bool),
      field("sub_bool_expression", $.sub_bool_expr),
      field("quantifier_expression", $.quantifier_expr),
      field("active_expression", $.active_expr),
    )),

    active_expr: $ => seq(
      "active",
      "(",
      field("variant", $.identifier),
      ",",
      field("name", $.identifier),
      ")"
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

    toInt_expr: $ => seq("toInt","(", field("expression", choice($.bool_expr, $.comparison_expr, $.atom)), ")"),

    list_combining_expr_bool: $ => prec(-10, seq(
      field("operator", choice("and", "or")),
      "(",
      field("arg", $.atom),
      ")"
    )),

    quantifier_expr: $ => prec(-5, seq(
      field("operator", choice("forAll", "exists")),
      field("variables", commaSep1($.identifier)),
      choice(
        seq("in", field("collection", choice($.set_literal, $.matrix, $.tuple, $.record, $.identifier, $.index_or_slice))),
        seq(":", field("domain", $.domain))
      ),
      ".",
      field("expression", choice($.bool_expr, $.comparison_expr, $.atom))
    )),

    aggregate_expr: $ => prec(-5, seq(
      field("operator", choice("sum", "min", "max")),
      field("variables", commaSep1($.identifier)),
      choice(
        seq("in", field("collection", choice($.set_literal, $.matrix, $.tuple, $.record, $.identifier, $.index_or_slice))),
        seq(":", field("domain", $.domain))
      ),
      ".",
      field("expression", choice($.arithmetic_expr, $.atom))
    )),

    from_solution: $ => seq(
      "fromSolution",
      "(",
      field("variable", $.identifier),
      ")"
    ),

    // comparison expressions - split by operand types for type checking
    comparison_expr: $ => prec(5, choice(
      $.arithmetic_comparison,
      $.lex_comparison,
      $.equality_comparison,
      $.set_comparison,
      $.sequence_comparison,
      $.all_diff_comparison,
      $.all_different_except_comparison,
      $.global_cardinality_comparison
    )),

    // Arithmetic comparisons: require arithmetic operands, return boolean
    arithmetic_comparison: $ => prec.left(seq(
      field("left", choice($.arithmetic_expr, $.annotation_expr, $.atom)),
      field("operator", choice("<=", ">=", "<", ">")),
      field("right", choice($.arithmetic_expr, $.annotation_expr, $.atom))
    )),

    // Lexicographic comparisons: require collection operands, return boolean.
    lex_comparison: $ => prec.left(seq(
      field("left", $.atom),
      field("operator", choice("<lex", "<=lex", ">lex", ">=lex")),
      field("right", $.atom)
    )),

    // Equality comparisons: work on any type, return boolean
    equality_comparison: $ => prec.left(seq(
      field("left", choice($.bool_expr, $.arithmetic_expr, $.all_diff_comparison, $.all_different_except_comparison, $.annotation_expr, $.atom)),
      field("operator", choice("=", "!=")),
      field("right", choice($.bool_expr, $.arithmetic_expr, $.all_diff_comparison, $.all_different_except_comparison, $.annotation_expr, $.atom))
    )),

    // Set comparisons: require set operands, return boolean
    set_comparison: $ => seq(
      field("left", choice($.arithmetic_expr, $.annotation_expr, $.atom)),
      field("operator", choice("in", "subset", "subsetEq", "supset", "supsetEq")),
      field("right", choice($.annotation_expr, $.atom))
    ),

    // Sequence comparisons: 's substring t' / 's subsequence t', return boolean
    sequence_comparison: $ => seq(
      field("left", choice($.arithmetic_expr, $.annotation_expr, $.atom)),
      field("operator", choice("substring", "subsequence")),
      field("right", choice($.annotation_expr, $.atom))
    ),

    all_diff_comparison: $ => prec(-10, seq(
      field("operator", choice("allDifferent", "allDiff")),
      "(",
      field("arg", choice($.bool_expr, $.arithmetic_expr, $.atom)),
      ")"
    )),

    all_different_except_comparison: $ => prec(-10, seq(
      field("operator", choice(
        "allDifferentExcept",
        "alldifferent_except",
        "alldiff_except",
        "allDiffExcept"
      )),
      "(",
      field("matrix", choice($.matrix, $.identifier, $.index_or_slice, $.flatten, $.bool_expr, $.arithmetic_expr)),
      ",",
      field("except", choice($.identifier, $.constant, $.index_or_slice, $.sub_arith_expr, $.negative_expr, $.arithmetic_expr)),
      ")"
    )),

    global_cardinality_comparison: $ => prec(-10, seq(
      field("operator", choice(
        "globalCardinality",
        "atLeast",
        "atMost",
        "atleast",
        "atmost",
        "gcc"
      )),
      "(",
      field("variables", choice($.arithmetic_expr, $.atom)),
      ",",
      field("arg2", choice($.arithmetic_expr, $.atom)),
      ",",
      field("arg3", choice($.arithmetic_expr, $.atom)),
      ")"
    )),

    sub_bool_expr: $ => seq("(", field("expression", choice($.bool_expr, $.comparison_expr)), ")"),
    
    arithmetic_expr: $ => prec(3, choice(
      field("toInt_expr", $.toInt_expr),
      field("negative_expression", $.negative_expr),
      field("absolute_value", $.abs_value),
      field("factorial_expression", $.factorial_expr),
      field("exponentiation", $.exponent),
      field("product_expression", $.product_expr),
      field("sum_expression", $.sum_expr),
      field("list_combining_expression_arith", $.list_combining_expr_arith),
      field("aggregate_expression", $.aggregate_expr),
      field("sub_arith_expression", $.sub_arith_expr)
    )),

    atom: $ => prec(-1, choice(
      field("sub_atom_expression", $.sub_atom_expr),
      field("constant", $.constant),
      field("variable", $.identifier),
      field("metavar", $.metavar),
      field("tuple", $.tuple),
      field("matrix", $.matrix),
      field("comprehension", $.comprehension),
      field("record", $.record),
      field("variant", $.variant),
      field("from_solution", $.from_solution),
      field("index_or_slice", $.index_or_slice),
      field("set_literal", $.set_literal),
      field("mset_literal", $.mset_literal),
      field("sequence_literal", $.sequence_literal),
      field("function_literal", $.function_literal),
      field("relation_literal", $.relation_literal),
      field("set_operation", $.set_operation),
      field("flatten", $.flatten),
      field("element_id", $.element_id),
      field("table", $.table),
      field("negative_table", $.negative_table),
      field("pareto_expression", $.pareto_expression),
      field("image_expr", $.image_expr),
      field("image_set_expr", $.image_set_expr),
      field("pre_image_expr", $.pre_image_expr),
      field("inverse_expr", $.inverse_expr),
      field("restrict_expr", $.restrict_expr),
      field("defined_expr", $.defined_expr),
      field("range_expr", $.range_expr),
      field("to_set_expr", $.to_set_expr),
      field("to_mset_expr", $.to_mset_expr),
      field("to_relation_expr", $.to_relation_expr)
    )),

    sub_atom_expr: $ => seq("(", field("expression", choice($.annotation_expr, $.atom)), ")"),

    tuple: $ => prec(-5,choice(
      // explicit tuple value using the 'tuple' keyword (allows empty, singleton, multi-arity)
      seq(
        "tuple",
        "(",
        optional(commaSep1(choice($.arithmetic_expr, $.atom))),
        ")"
      ),
      // parenthesized tuple value without keyword (arity >= 2)
      seq(
        "(",
        field("element", choice($.arithmetic_expr, $.atom)),
        ",",
        field("element", commaSep1(choice($.arithmetic_expr, $.atom))),
        ")"
      ))
    ),

    matrix: $ => seq(
      "[",
      optional(field("elements", commaSep1(choice($.bool_expr, $.arithmetic_expr, $.comparison_expr, $.atom)))),
      optional(seq(
        ";",
        field("domain", choice($.int_domain, $.bool_domain, $.identifier))
      )),
      "]"
    ),

    comprehension: $ => prec(1, seq(
      "[",
      field("expression", choice($.arithmetic_expr, $.bool_expr, $.comparison_expr, $.atom)),
      "|",
      field("generator_or_condition", commaSep1(choice(
        $.generator,
        $.condition,
        // TODO: add letting statement support
      ))),
      "]"
    )),

    generator: $ => prec(30, choice(
      // i : int(1..5)
      seq(
        field("variable", choice($.identifier, $.tuple)),
        ":",
        field("domain", $.domain)
      ),
      // i <- [1,2,3]
      seq(
        field("variable", choice($.identifier, $.tuple)),
        "<-",
        field("collection", choice($.arithmetic_expr, $.identifier, $.matrix))
      )
    )),

    condition: $ => field("expression", choice($.bool_expr, $.comparison_expr, $.atom)),

    record: $ => seq(
      "record",
      "{",
      commaSep1(field("name_value_pair", $.name_value_pair)),
      "}"
    ),

    variant: $ => seq(
      "variant",
      "{",
      field("name_value_pair", $.name_value_pair),
      "}"
    ),

    name_value_pair: $ => seq(
      field("name", $.identifier),
      "=",
      field("value", choice($.arithmetic_expr, $.bool_expr, $.comparison_expr, $.atom))
    ),

    index_or_slice: $ => prec.left(seq(
      field("collection", choice($.identifier, $.tuple, $.matrix, $.record, $.flatten, $.index_or_slice)),
      "[",
      field("indices", $.indices),
      "]"
    )),

    // TODO: add unary set operation support (powerSet)
    set_operation: $ => prec.left(seq(
      field("left", $.atom),
      field("operator", choice("union", "intersect")),
      field("right", $.atom)
    )),

    annotation_expr: $ => prec(25, choice(
      $.type_annotation,
      $.domain_annotation
    )),

    type_annotation: $ => prec(25, seq(
      field("left", $.annotation_subject),
      field("operator", token("::")),
      field("type", $.annotation_domain)
    )),

    domain_annotation: $ => prec(25, seq(
      field("left", $.annotation_subject),
      field("operator", token(":")),
      field("domain", $.annotation_domain)
    )),

    annotation_subject: $ => choice(
      $.sub_arith_expr,
      $.sub_bool_expr,
      $.sub_atom_expr,
      $.constant,
      $.identifier,
      $.metavar,
      $.matrix,
      $.comprehension,
      $.tuple,
      $.record,
      $.variant,
      $.set_literal,
      $.mset_literal,
      $.sequence_literal,
      $.function_literal,
      $.index_or_slice,
      $.flatten,
      $.element_id
    ),

    annotation_domain: $ => choice(
      field("bool_domain", $.bool_domain),
      field("int_domain", $.annotation_int_domain),
      field("variable_domain", $.identifier),
      field("tuple_domain", $.annotation_tuple_domain),
      field("matrix_domain", $.annotation_matrix_domain),
      field("record_domain", $.annotation_record_domain),
      field("variant_domain", $.annotation_variant_domain),
      field("set_domain", $.annotation_set_domain),
      field("mset_domain", $.annotation_mset_domain),
      field("sequence_domain", $.annotation_sequence_domain),
      field("function_domain", $.annotation_function_domain),
    ),

    annotation_int_domain: $ => seq(
      "int",
      optional(seq(
        "(",
        optional(field("ranges", $.annotation_range_list)),
        ")"
      ))
    ),

    annotation_range_list: $ => commaSep1(choice($.annotation_int_range, $.integer, $.identifier)),

    annotation_int_range: $ => seq(
      optional(field("lower", choice($.integer, $.identifier))),
      "..",
      optional(field("upper", choice($.integer, $.identifier)))
    ),

    annotation_tuple_domain: $ => choice(
      seq(
        "tuple",
        "(",
        optional(commaSep1($.annotation_domain)),
        ")"
      ),
      seq(
        "(",
        $.annotation_domain,
        ",",
        commaSep1($.annotation_domain),
        ")"
      )
    ),

    annotation_matrix_domain: $ => seq(
      "matrix",
      optional("indexed"),
      optional("by"),
      optional("indexed"),
      optional("by"),
      "[",
      field("index_domain_list", commaSep1($.annotation_domain)),
      "]",
      "of",
      field("value_domain", $.annotation_domain)
    ),

    annotation_record_domain: $ => seq(
      "record",
      "{",
      commaSep1(field("name_domain_pair", $.annotation_name_domain_pair)),
      "}"
    ),

    annotation_variant_domain: $ => seq(
      "variant",
      "{",
      commaSep1(field("name_domain_pair", $.annotation_name_domain_pair)),
      "}"
    ),

    annotation_name_domain_pair: $ => seq(
      field("name", $.identifier),
      ":",
      field("domain", $.annotation_domain)
    ),

    annotation_set_domain: $ => seq(
      "set",
      optional(seq("{", field("representation", $.identifier), "}")),
      optional(seq("(", $.set_attributes, ")")),
      "of",
      field("value_domain", $.annotation_domain)
    ),

    annotation_mset_domain: $ => seq(
      "mset",
      optional(seq("{", field("representation", $.identifier), "}")),
      optional(seq("(", $.mset_attributes, ")")),
      "of",
      field("value_domain", $.annotation_domain)
    ),

    annotation_sequence_domain: $ => seq(
      "sequence",
      optional(seq("(", $.sequence_attributes, ")")),
      "of",
      field("value_domain", $.annotation_domain)
    ),

    annotation_function_domain: $ => seq(
      "function",
      optional(seq("(", $.function_attributes, ")")),
      field("domain_from", $.annotation_domain),
      "-->",
      field("domain_to", $.annotation_domain)
    ),

    // binary_set_operation: $ => prec.left(seq(
    //   field("left", $.atom),
    //   choice("union", "intersect"),
    //   field("right", $.atom)
    // )),

    // unary_set_operation: $ => prec(1,seq(
    //   "powerSet",
    //   field("argument", $.atom)
    // )),

    flatten: $ => seq(
      "flatten",
      "(",
      optional(field("depth", seq($.integer, ","))),
      field("expression", $.atom),
      ")"
    ),

    element_id: $ => seq(
      "elementId",
      "(",
      field("matrix", choice($.matrix, $.identifier, $.index_or_slice, $.flatten)),
      ",",
      field("value", choice($.identifier, $.index_or_slice, $.constant, $.sub_arith_expr, $.negative_expr, $.abs_value, $.factorial_expr)),
      ")"
    ),

    table: $ => seq(
      "table",
      "(",
      field("variables", choice($.matrix, $.identifier, $.index_or_slice, $.flatten)),
      ",",
      field("rows", choice($.matrix, $.identifier, $.index_or_slice, $.flatten)),
      ")"
    ),

    negative_table: $ => seq(
      choice("negativeTable", "negative_table"),
      "(",
      field("variables", choice($.matrix, $.identifier, $.index_or_slice, $.flatten)),
      ",",
      field("rows", choice($.matrix, $.identifier, $.index_or_slice, $.flatten)),
      ")"
    ),

    indices: $ => commaSep1(choice(field("index", choice($.arithmetic_expr, $.atom)), field("null_index", $.null_index))),

    null_index: $ => "..",

    sub_arith_expr: $ => seq("(", field("expression", $.arithmetic_expr), ")"),

    negative_expr: $ => prec(15, prec.left(seq("-", field("expression", choice($.arithmetic_expr, $.atom))))),
    
    abs_value: $ => prec(20, seq("|", field("expression", choice($.arithmetic_expr, $.atom)), "|")),

    factorial_expr: $ => prec(19, choice(
      seq(field("expression", choice($.arithmetic_expr, $.atom)), "!"),
      seq("factorial", "(", field("expression", choice($.arithmetic_expr, $.atom)), ")")
    )),
    
    exponent: $ => prec(18, prec.right(seq(
      field("left", choice($.arithmetic_expr, $.annotation_expr, $.atom)), 
      field("operator", "**"),
      field("right", choice($.arithmetic_expr, $.annotation_expr, $.atom))
    ))),

    product_expr: $ => prec(10, prec.left(seq(
      field("left", choice($.arithmetic_expr, $.annotation_expr, $.atom)), 
      field("operator", $.mulitcative_op), 
      field("right", choice($.arithmetic_expr, $.annotation_expr, $.atom))
    ))),
    
    mulitcative_op: $ => choice("*", "/", "%"),
    
    sum_expr: $ => prec(1, prec.left(seq(
      field("left", choice($.arithmetic_expr, $.annotation_expr, $.atom)), 
      field("operator", $.additive_op), 
      field("right", choice($.arithmetic_expr, $.annotation_expr, $.atom))
    ))),

    additive_op: $ => choice("+", "-"),

    list_combining_expr_arith: $ => prec(-10, seq(
      field("operator", choice("min", "max", "sum", "product")),
      "(",
      field("arg", choice($.bool_expr, $.comparison_expr, $.arithmetic_expr, $.annotation_expr, $.atom)),
      ")"
    )),

    pareto_expression: $ => seq(
      "pareto",
      "(",
      field("components", $.pareto_items),
      ")"
    ),

    pareto_items: $ => commaSep1($.pareto_item),

    pareto_item: $ => seq(
      field("direction", choice("minimising", "maximising")),
      field("expression", choice($.bool_expr, $.comparison_expr, $.arithmetic_expr, $.atom))
    ),

    objective_statement: $ => seq(
      field("direction", choice("minimising", "maximising")),
      field("expression", choice($.bool_expr, $.comparison_expr, $.arithmetic_expr, $.atom))
    ),

    dominance_relation: $ => seq(
      "dominance relation",
      field("expression", choice($.bool_expr, $.comparison_expr, $.arithmetic_expr, $.atom)),
    )
  }
});

function commaSep1(rule) {
  return seq(rule, optional(repeat(seq(",", rule))), optional(","));
}

function starSep1(rule) {
  return seq(rule, optional(repeat(seq("*", rule))));
}

module.exports = grammar({
  name: "essence",

  extras: ($) => [$.single_line_comment, /\s/, $.language_declaration],

  word: ($) => $.identifier,

  rules: {
    // Top-level statements
    program: ($) =>
      repeat(
        choice(
          field("find_statement", $.find_statement),
          field("given_statement", $.given_statement),
          seq(field("such_that_keyword", "such that"), commaSep1($.expression)),
          field("where_statement", $.where_statement),
          field("letting_statement", $.letting_statement),
          field("objective_statement", $.objective_statement),
          field("dominance_relation", $.dominance_relation),
        ),
      ),

    single_line_comment: ($) => token(seq("$", /.*/)),

    language_declaration: ($) => token(seq("language", /.*/)),

    where_statement: ($) =>
      seq(field("where_keyword", "where"), commaSep1($.expression)),

    //general
    constant: ($) =>
      choice(
        field("integer", $.integer),
        field("true", $.TRUE),
        field("false", $.FALSE),
      ),

    integer: ($) => /-?[0-9]+/,

    TRUE: ($) => "true",

    FALSE: ($) => "false",

    identifier: ($) => {
      const emoji =
        "\\p{Extended_Pictographic}[\\p{Emoji_Modifier}\\uFE0F]?(\\u200D\\p{Extended_Pictographic}[\\p{Emoji_Modifier}\\uFE0F]?)*";
      return new RegExp(`([a-zA-Z_]|${emoji})([a-zA-Z0-9_]|${emoji})*'*`, "u");
    },

    //meta-variable (aka template argument)
    metavar: ($) => seq("&", field("identifier", $.identifier)),

    //find / findAux statements (findAux before find so the longer keyword wins)
    find_statement: ($) =>
      seq(
        field("find_keyword", choice("findAux", "find")),
        field("variable_declaration", commaSep1($.variable_declaration)),
      ),

    //given statements
    given_statement: ($) =>
      seq(
        field("given_keyword", "given"),
        field("variable_declaration", commaSep1($.variable_declaration)),
      ),

    COLON: ($) => ":",

    variable_declaration: ($) =>
      seq(
        field("variables", $.variable_list),
        field("colon", $.COLON),
        field("domain", $.domain),
      ),

    variable_list: ($) => commaSep1($.identifier),

    domain_attribute: ($) =>
      seq(
        field("attribute", $.domain_attribute_identifier),
        optional(field("value", $.expression)),
      ),

    domain_attribute_identifier: ($) =>
      choice(
        "size",
        "minSize",
        "maxSize",
        "minOccur",
        "maxOccur",
        "representation",
        "total",
        "injective",
        "surjective",
        "bijective",
        "reflexive",
        "irreflexive",
        "coreflexive",
        "symmetric",
        "antiSymmetric",
        "aSymmetric",
        "transitive",
        "total",
        "connex",
        "Euclidean",
        "serial",
        "equivalence",
        "partialOrder",
        "leftTotal",
        "rightTotal",
        "linearOrder",
        "weakOrder",
        "preOrder",
        "strictPartialOrder",
      ),

    domain_attribute_list: ($) => seq("(", commaSep1($.domain_attribute), ")"),

    domain: ($) =>
      choice(
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
        field("partition_domain", $.partition_domain),
        field("permutation_domain", $.permutation_domain),
      ),

    bool_domain: ($) => "bool",

    int_domain: ($) =>
      seq(
        "int",
        optional(seq("(", optional(field("ranges", $.range_list)), ")")),
      ),

    range_list: ($) => prec(2, commaSep1(choice($.int_range, $.expression))),

    int_range: ($) =>
      seq(
        optional(field("lower", $.expression)),
        "..",
        optional(field("upper", $.expression)),
      ),

    tuple_domain: ($) =>
      choice(
        // explicit 'tuple' form: allows empty, singleton, or multi-arity
        seq("tuple", "(", optional(commaSep1($.domain)), ")"),
        // parenthesized form without 'tuple' is allowed only for arity >= 2
        seq("(", $.domain, ",", commaSep1($.domain), ")"),
      ),

    matrix_domain: ($) =>
      seq(
        "matrix",
        "indexed",
        "by",
        "[",
        field("index_domain_list", $.index_domain_list),
        "]",
        "of",
        field("value_domain", $.domain),
      ),

    index_domain_list: ($) => commaSep1($.domain),

    name_domain_pair: ($) =>
      seq(field("name", $.identifier), ":", field("domain", $.domain)),

    record_domain: ($) =>
      seq(
        "record",
        "{",
        commaSep1(field("name_domain_pair", $.name_domain_pair)),
        "}",
      ),

    variant_domain: ($) =>
      seq(
        "variant",
        "{",
        commaSep1(field("name_domain_pair", $.name_domain_pair)),
        "}",
      ),

    set_domain: ($) =>
      seq(
        "set",
        optional($.domain_attribute_list),
        "of",
        field("value_domain", $.domain),
      ),

    mset_domain: ($) =>
      seq(
        "mset",
        optional($.domain_attribute_list),
        "of",
        field("value_domain", $.domain),
      ),

    // e.g. sequence (size 2) of int(1..3), sequence (maxSize 3) of int(0..2),
    // sequence (minSize 1, maxSize 2) of int(1..3), or bare sequence of int(1..3).
    // Note: the grammar itself does not require a size bound here; that is
    // enforced downstream by domain-construction logic, mirroring set/mset.
    sequence_domain: ($) =>
      seq(
        "sequence",
        optional($.domain_attribute_list),
        "of",
        field("value_domain", $.domain),
      ),

    // e.g. function int(1..3) --> int(1..3), function (total) bool --> int(13,17),
    // function (minSize 1) int(1..2) --> set (size 1) of int(1..2). Attribute values may
    // optionally be parenthesised (e.g. `size(2)`, `minSize(1)`), matching Conjure's own
    // grammar where the attribute value is just an integer expression.
    function_domain: ($) =>
      seq(
        "function",
        optional($.domain_attribute_list),
        field("domain_from", $.domain),
        "-->",
        field("domain_to", $.domain),
      ),

    // e.g. relation of (int(1..3) * bool), relation (size 2, irreflexive) of (int(0..5) * int(0..5)),
    // relation of (int(1..3) * int(1..5) * bool). Columns are separated by "*", matching Conjure's
    // own relation-domain syntax; unlike matrix's index_domain_list this is not comma-separated.
    relation_domain: ($) =>
      seq(
        "relation",
        optional($.domain_attribute_list),
        "of",
        "(",
        field("relation_domain_list", $.relation_domain_list),
        ")",
      ),

    relation_domain_list: ($) => starSep1($.domain),

    // e.g. partition from int(1..3), partition (regular) from int(1..4),
    // partition (numParts 2, partSize 3) from int(3..8).
    partition_domain: ($) =>
      seq(
        "partition",
        optional($.domain_attribute_list),
        "from",
        field("value_domain", $.domain),
      ),

    // e.g. permutation of int(1..3), permutation (numMoved 2) of int(1..4),
    // permutation (minNumMoved 1, maxNumMoved 3) of int(3..8). A permutation is inherently total
    // and bijective, so its only attribute is a size constraint on the number of moved points
    // (real Conjure's own keyword is `numMoved`, not `size`).
    permutation_domain: ($) =>
      seq(
        "permutation",
        optional($.domain_attribute_list),
        "of",
        field("value_domain", $.domain),
      ),

    set_literal: ($) =>
      seq("{", field("element", commaSep1($.expression)), "}"),

    mset_literal: ($) =>
      seq(
        "mset",
        "(",
        optional(field("element", commaSep1($.expression))),
        ")",
      ),

    sequence_literal: ($) =>
      seq(
        "sequence",
        "(",
        optional(field("element", commaSep1($.expression))),
        ")",
      ),

    // e.g. function(), function(0-->1,1-->0), function(false --> 5, true --> x)
    function_literal: ($) =>
      seq(
        "function",
        "(",
        optional(field("pair", commaSep1($.function_literal_pair))),
        ")",
      ),

    function_literal_pair: ($) =>
      seq(field("key", $.expression), "-->", field("value", $.expression)),

    // e.g. relation(), relation((2,6),(1,4)), relation((1,false,true),(2,false,false)).
    // Each element is itself a parenthesised tuple of >= 2 values, one per column.
    relation_literal: ($) =>
      seq(
        "relation",
        "(",
        optional(field("tuple", commaSep1($.relation_literal_tuple))),
        ")",
      ),

    relation_literal_tuple: ($) =>
      seq("(", field("element", commaSep1($.expression)), ")"),

    // e.g. partition(), partition({1}), partition({1,3},{6}). Each part is itself a set literal.
    partition_literal: ($) =>
      seq(
        "partition",
        "(",
        optional(field("part", commaSep1($.set_literal))),
        ")",
      ),

    // e.g. permutation(), permutation((1,2,3)), permutation((1,2),(4,5)). Cycle notation: each
    // cycle is a parenthesised list of elements; any domain element not mentioned in any cycle is
    // an implicit fixed point.
    permutation_literal: ($) =>
      seq(
        "permutation",
        "(",
        optional(field("cycle", commaSep1($.permutation_literal_cycle))),
        ")",
      ),

    permutation_literal_cycle: ($) =>
      seq("(", field("element", commaSep1($.expression)), ")"),

    // Function or sequence application, e.g. `f(1)` or `horizontalLocs[row](index)`. This is
    // sugar for `image(f, x)`, and is parsed into the same node. Several arguments make a tuple,
    // so `f(1, 2)` and `f((1, 2))` mean the same thing.
    apply_expr: ($) =>
      prec.left(
        seq(
          field("function", $.expression),
          "(",
          commaSep1(field("argument", $.expression)),
          ")",
        ),
      ),

    //letting statements
    letting_statement: ($) =>
      seq(
        field("letting_keyword", "letting"),
        field(
          "letting_variable_declaration",
          commaSep1($.letting_variable_declaration),
        ),
      ),

    letting_variable_declaration: ($) =>
      seq(
        field("variable_list", $.variable_list),
        field("be", "be"),
        choice(
          field("expr_or_domain", $.expression),
          seq(field("domain", "domain"), field("expr_or_domain", $.domain)),
        ),
      ),

    // Unary boolean operations (just not)
    not_expr: ($) => prec(20, seq("!", field("expression", $.expression))),

    // Binary boolean operations
    and_expr: ($) => binop($, "/\\", -1),
    or_expr: ($) => binop($, "\\/", -2),
    implication_expr: ($) => binop($, "->", -4),
    iff_expr: ($) => binop($, "<->", -4),

    // Unary arithmetic operations
    negative_expr: ($) =>
      prec.left(15, seq("-", field("expression", $.expression))),

    abs_value_expr: ($) =>
      prec(20, seq("|", field("expression", $.expression), "|")),

    factorial_expr: ($) =>
      prec(19, seq(field("expression", $.expression), "!")),

    // Binary arithmetic operations
    exponent_expr: ($) => binop($, "**", 18, prec.right),
    product_expr: ($) => binop($, "*", 10),
    division_expr: ($) => binop($, "/", 10),
    modulo_expr: ($) => binop($, "%", 10),
    add_expr: ($) => binop($, "+", 1),
    subtract_expr: ($) => binop($, "-", 1),

    // Destructuring pattern for a generator variable, e.g. `(i, j)` over a set of pairs or
    // `(index, _)` over a sequence. `_` discards that component.
    quantifier_expr: ($) =>
      prec(
        -5,
        seq(
          field("operator", choice("forAll", "exists", "sum", "min", "max")),
          // A tuple here is a destructuring pattern, e.g. `forAll (i, j) in pairs`; `_` discards
          // that component.
          field("variables", commaSep1(choice($.identifier, $.tuple_literal))),
          choice(
            seq("in", field("collection", $.expression)),
            seq(":", field("domain", $.domain)),
          ),
          // A comma here is unambiguous: the variable list has already ended at `in` or `:`.
          optional(seq(",", field("guard", $.expression))),
          ".",
          field("expression", $.expression),
        ),
      ),

    from_solution: ($) =>
      seq("fromSolution", "(", field("variable", $.identifier), ")"),

    // Arithmetic comparison
    comparison_expr: ($) => binop($, choice("<", "<=", ">", ">="), 10),

    // Lexicographic comparison (comparing collections)
    lex_comparison_expr: ($) =>
      binop($, choice("<lex", "<=lex", ">lex", ">=lex"), 10),

    // General equality comparisons
    equals_expr: ($) => binop($, "=", 10),
    not_equals_expr: ($) => binop($, "!=", 10),

    // Set comparisons
    in_expr: ($) => binop($, "in", 25),
    subset_expr: ($) => binop($, "subset", 25),
    subsetEq_expr: ($) => binop($, "subsetEq", 25),
    supset_expr: ($) => binop($, "supset", 25),
    supsetEq_expr: ($) => binop($, "supsetEq", 25),

    // Sequence comparisons: 's substring t' / 's subsequence t', return boolean
    substring_expr: ($) => binop($, "substring", 25),
    subsequence_expr: ($) => binop($, "subsequence", 25),

    // TODO: add unary set operation support (powerSet)
    union_expr: ($) => binop($, "union", 25),
    intersect_expr: ($) => binop($, "intersect", 25),

    // A name bound for the rest of the comprehension, e.g.
    // `[ x[i] | i : int(1..3), letting j be i + 1, x[j] > 0 ]`. Pure sugar: the parser
    // substitutes the bound expression wherever the name is used.
    comprehension_letting: ($) =>
      seq(
        "letting",
        field("name", $.identifier),
        "be",
        field("value", $.expression),
      ),

    comprehension: ($) =>
      prec(
        1,
        seq(
          "[",
          field("expression", $.expression),
          "|",
          field("generator", $.generator),
          optional(
            seq(
              ",",
              field(
                "lettings_and_conditions",
                commaSep1(choice($.condition, $.comprehension_letting)),
              ),
            ),
          ),
          "]",
        ),
      ),

    generator: ($) =>
      prec(
        30,
        choice(
          // i : int(1..5)
          seq(
            field("variable", choice($.identifier, $.tuple_literal)),
            ":",
            field("domain", $.domain),
          ),
          // i <- [1,2,3]
          seq(
            field("variable", choice($.identifier, $.tuple_literal)),
            "<-",
            field("collection", $.expression),
          ),
        ),
      ),

    condition: ($) => field("expression", $.expression),

    tuple_literal: ($) =>
      prec(
        -5,
        choice(
          // explicit tuple value using the 'tuple' keyword (allows empty, singleton, multi-arity)
          seq("tuple", "(", optional(commaSep1($.expression)), ")"),
          // parenthesized tuple value without keyword (arity >= 2)
          seq(
            "(",
            field("element", $.expression),
            ",",
            field("element", commaSep1($.expression)),
            ")",
          ),
        ),
      ),

    matrix_literal: ($) =>
      seq(
        "[",
        optional(field("elements", commaSep1($.expression))),
        optional(seq(";", field("domain", $.expression))),
        "]",
      ),

    record_literal: ($) =>
      seq(
        "record",
        "{",
        commaSep1(field("name_value_pair", $.name_value_pair)),
        "}",
      ),

    variant_literal: ($) =>
      seq("variant", "{", field("name_value_pair", $.name_value_pair), "}"),

    name_value_pair: ($) =>
      seq(field("name", $.identifier), "=", field("value", $.expression)),

    index_or_slice: ($) =>
      prec.left(
        seq(
          field("collection", $.expression),
          "[",
          field("indices", $.indices),
          "]",
        ),
      ),

    annotation_expr: ($) =>
      prec(25, choice($.type_annotation, $.domain_annotation)),

    type_annotation: ($) =>
      prec(
        25,
        seq(
          field("left", $.expression),
          field("operator", token("::")),
          field("type", $.domain),
        ),
      ),

    domain_annotation: ($) =>
      prec(
        25,
        seq(
          field("left", $.expression),
          field("operator", token(":")),
          field("domain", $.domain),
        ),
      ),

    indices: ($) =>
      commaSep1(
        choice(field("index", $.expression), field("null_index", $.null_index)),
      ),

    null_index: ($) => "..",

    sub_expr: ($) => seq("(", field("expression", $.expression), ")"),

    pareto_expression: ($) =>
      seq("pareto", "(", field("components", $.pareto_items), ")"),

    pareto_items: ($) => commaSep1($.pareto_item),

    pareto_item: ($) =>
      seq(
        field("direction", choice("minimising", "maximising")),
        field("expression", $.expression),
      ),

    objective_statement: ($) =>
      seq(
        field("direction", choice("minimising", "maximising")),
        field("expression", $.expression),
      ),

    dominance_relation: ($) =>
      seq("dominance relation", field("expression", $.expression)),

    expression: ($) =>
      choice(
        // basics
        $.identifier,
        $.sub_expr,

        // literals
        $.constant,
        $.tuple_literal,
        $.matrix_literal,
        $.record_literal,
        $.variant_literal,
        $.set_literal,
        $.mset_literal,
        $.sequence_literal,
        $.function_literal,
        $.relation_literal,
        $.partition_literal,
        $.permutation_literal,

        // comparison operations
        $.comparison_expr,
        $.lex_comparison_expr,
        $.equals_expr,
        $.not_equals_expr,

        // set operations
        $.in_expr,
        $.subset_expr,
        $.subsetEq_expr,
        $.supset_expr,
        $.supsetEq_expr,
        $.substring_expr,
        $.subsequence_expr,
        $.union_expr,
        $.intersect_expr,

        // boolean operations
        $.not_expr,
        $.and_expr,
        $.or_expr,
        $.implication_expr,
        $.iff_expr,

        // arithmetic operations
        $.negative_expr,
        $.abs_value_expr,
        $.factorial_expr,
        $.exponent_expr,
        $.product_expr,
        $.division_expr,
        $.modulo_expr,
        $.add_expr,
        $.subtract_expr,

        // other expressions
        $.index_or_slice,
        $.apply_expr,
        $.quantifier_expr,
        $.from_solution,
      ),
  },
});

function commaSep1(rule) {
  return seq(rule, optional(repeat(seq(",", rule))), optional(","));
}

function starSep1(rule) {
  return seq(rule, optional(repeat(seq("*", rule))));
}

function binop($, operator, precedence, assoc = prec.left) {
  return assoc(
    precedence,
    seq(
      field("left", $.expression),
      field("operator", operator),
      field("right", $.expression),
    ),
  );
}

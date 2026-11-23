module.exports = grammar({
  name: "essence",

  extras: ($) => [$.single_line_comment, /\s/],

  word: ($) => $.identifier,

  supertypes: ($) => [$.expression, $.domain],

  rules: {
    // Top-level statements
    program: ($) =>
      choice(
        seq("_FRAGMENT_EXPRESSION", field("fragment", $.expression)), // HACK
        repeat(
          choice(
            $.language_statement,
            $.find_statement,
            $.given_statement,
            $.such_that_statement,
            $.where_statement,
            $.letting_statement,
            $.objective_statement,
            $.dominance_relation,
          ),
        ),
      ),

    single_line_comment: ($) => token(seq("$", /.*/)),

    language_statement: ($) => token(seq("language", field("language", /.*/))),

    where_statement: ($) => seq("where", commaSep1($.expression)),

    such_that_statement: ($) => seq("such that", commaSep1($.expression)),

    //general
    constant: ($) => choice($.integer, $.true, $.false),

    integer: ($) => /-?[0-9]+/,

    true: ($) => "true",

    false: ($) => "false",

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
        "given",
        field("variable_declaration", commaSep1($.variable_declaration)),
      ),

    COLON: ($) => ":",

    variable_declaration: ($) =>
      seq(
        field("variables", commaSep1($.identifier)),
        ":",
        choice(field("domain", $.domain), field("enum", "new type enum")),
      ),

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
        $.identifier,
        $.bool_domain,
        $.int_domain,
        $.tuple_domain,
        $.matrix_domain,
        $.record_domain,
        $.variant_domain,
        $.set_domain,
        $.mset_domain,
        $.sequence_domain,
        $.function_domain,
        $.relation_domain,
        $.partition_domain,
        $.permutation_domain,
      ),

    bool_domain: ($) => "bool",

    int_domain: ($) =>
      seq(
        "int",
        optional(seq("(", optional(field("ranges", $.range_list)), ")")),
      ),

    range_list: ($) => prec(2, commaSep1($.range_or_value)),

    range_or_value: ($) =>
      choice(field("range", $.int_range), field("value", $.expression)),

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
        field("index_domain_list", commaSep1($.domain)),
        "]",
        "of",
        field("value_domain", $.domain),
      ),

    name_domain_pair: ($) =>
      seq(field("name", $.identifier), ":", field("domain", $.domain)),

    record_domain: ($) =>
      seq("record", "{", commaSep1($.name_domain_pair), "}"),

    variant_domain: ($) =>
      seq("variant", "{", commaSep1($.name_domain_pair), "}"),

    set_domain: ($) =>
      seq(
        "set",
        field("attrs", optional($.domain_attribute_list)),
        "of",
        field("value_domain", $.domain),
      ),

    mset_domain: ($) =>
      seq(
        "mset",
        field("attrs", optional($.domain_attribute_list)),
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
        field("attrs", optional($.domain_attribute_list)),
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
        field("attrs", optional($.domain_attribute_list)),
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
        field("attrs", optional($.domain_attribute_list)),
        "of",
        "(",
        field("relation_domain_list", $._relation_domain_list),
        ")",
      ),

    _relation_domain_list: ($) => starSep1($.domain),

    // e.g. partition from int(1..3), partition (regular) from int(1..4),
    // partition (numParts 2, partSize 3) from int(3..8).
    partition_domain: ($) =>
      seq(
        "partition",
        field("attrs", optional($.domain_attribute_list)),
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
        field("attrs", optional($.domain_attribute_list)),
        "of",
        field("value_domain", $.domain),
      ),

    set_literal: ($) => seq("{", commaSep1($.expression), "}"),

    mset_literal: ($) =>
      seq("mset", "(", optional(commaSep1($.expression)), ")"),

    sequence_literal: ($) =>
      seq("sequence", "(", optional(commaSep1($.expression)), ")"),

    // e.g. function(), function(0-->1,1-->0), function(false --> 5, true --> x)
    function_literal: ($) =>
      seq("function", "(", optional(commaSep1($.function_literal_pair)), ")"),

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

    builtin_function: ($) =>
      choice(
        "allDifferent",
        "allDifferentExcept",
        "and",
        "apart",
        "atLeast",
        "atMost",
        "catchUndef",
        "compose",
        "defined",
        "factorial",
        "flatten",
        "gcc",
        "image",
        "imageSet",
        "inverse",
        "max",
        "min",
        "negativeTable",
        "or",
        "participants",
        "parts",
        "party",
        "permInverse",
        "preImage",
        "product",
        "range",
        "restrict",
        "sum",
        "table",
        "toInt",
        "toMSet",
        "toRelation",
        "toSet",
        "together",
      ),

    // Function or sequence application, e.g. `f(1)` or `horizontalLocs[row](index)`. This is
    // sugar for `image(f, x)`, and is parsed into the same node. Several arguments make a tuple,
    // so `f(1, 2)` and `f((1, 2))` mean the same thing.
    apply_expr: ($) =>
      prec.left(
        seq(
          field("function", choice($.expression, $.builtin_function)),
          "(",
          commaSep1(field("argument", $.expression)),
          ")",
        ),
      ),

    //letting statements
    letting_statement: ($) =>
      seq(
        "letting",
        field(
          "letting_variable_declaration",
          commaSep1($.letting_variable_declaration),
        ),
      ),

    letting_variable_declaration: ($) =>
      seq(
        field("variables", commaSep1($.identifier)),
        "be",
        choice(
          field("expression", $.expression),
          seq("domain", field("domain", $.domain)),
          seq("new type enum", field("enum", $.enum_variants)),
          seq("unnamed type of size", field("unnamed", $.expression)),
        ),
      ),

    enum_variants: ($) => seq("{", commaSep1($.identifier), "}"),

    // Unary boolean operations (just not)
    not_expr: ($) => prec(20, seq("!", field("expression", $.expression))),

    // Unary arithmetic operations
    negative_expr: ($) =>
      prec.left(15, seq("-", field("expression", $.expression))),

    abs_value_expr: ($) =>
      prec(20, seq("|", field("expression", $.expression), "|")),

    factorial_expr: ($) =>
      prec(19, seq(field("expression", $.expression), "!")),

    // Destructuring pattern for a generator variable, e.g. `(i, j)` over a set of pairs or
    // `(index, _)` over a sequence. `_` discards that component.
    quantifier_expr: ($) =>
      prec(
        -5,
        seq(
          field("operator", choice("forAll", "exists", "sum", "min", "max")),
          // A tuple here is a destructuring pattern, e.g. `forAll (i, j) in pairs`; `_` discards
          // that component.
          field("variables", commaSep1($.tuple_destructuring)),
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

    tuple_destructuring: ($) =>
      choice(
        field("identifier", $.identifier),
        field("placeholder", "_"),
        seq("(", field("tuple", commaSep1($.tuple_destructuring)), ")"),
      ),

    binary_expr: ($) =>
      choice(
        // convert binary operators to binary expression grammar
        ...[
          ["+", 1],
          ["-", 1],
          ["*", 10],
          ["/", 10],
          ["%", 10],
          ["/\\", -1], // and expr
          ["\\/", -2], // or expr
          ["->", -4], // implication expr
          ["<->", -4], // iff expr
          // Equality comparison
          [choice("==", "!="), 10],
          // Arithmetic comparison
          [choice("<", "<=", ">", ">="), 10],
          // Lexicographic comparison
          [choice("<lex", "<=lex", ">lex", ">=lex"), 10],
          // Set comparison
          ["in", 25],
          [choice("subset", "subsetEq", "supset", "supsetEq"), 25],
          // Sequence comparisons
          ["substring", 25],
          ["subsequence", 25],
          // Set operations
          ["union", 25],
          ["intersect", 25],
        ].map(([op, prec]) => binop($, op, prec)),
        // handle ** specially (right association)
        binop($, "**", 10, prec.right),
      ),

    from_solution: ($) =>
      seq("fromSolution", "(", field("variable", $.identifier), ")"),

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
          commaSep1(field("generator", $.generator)),
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
        seq(
          commaSep1(field("variable", $.tuple_destructuring)),
          choice(
            // i : int(1..5)
            seq(":", field("domain", $.domain)),
            // i <- [1,2,3]
            seq("<-", field("collection", $.expression)),
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
        optional(seq(";", field("domain", $.domain))),
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

        // operations
        $.binary_expr,
        $.not_expr,
        $.negative_expr,
        $.abs_value_expr,
        $.factorial_expr,

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

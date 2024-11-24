#include "tree_sitter/parser.h"

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#endif

#define LANGUAGE_VERSION 14
#define STATE_COUNT 78
#define LARGE_STATE_COUNT 2
#define SYMBOL_COUNT 57
#define ALIAS_COUNT 0
#define TOKEN_COUNT 27
#define EXTERNAL_TOKEN_COUNT 0
#define FIELD_COUNT 0
#define MAX_ALIAS_SEQUENCE_LENGTH 4
#define PRODUCTION_ID_COUNT 1

enum ts_symbol_identifiers {
  anon_sym_find = 1,
  anon_sym_COLON = 2,
  anon_sym_COMMA = 3,
  anon_sym_suchthat = 4,
  anon_sym_int = 5,
  anon_sym_LPAREN = 6,
  anon_sym_RPAREN = 7,
  sym_bool_domain = 8,
  anon_sym_DOT_DOT = 9,
  sym_identifier = 10,
  anon_sym_or = 11,
  anon_sym_and = 12,
  anon_sym_PLUS = 13,
  anon_sym_DASH = 14,
  anon_sym_STAR = 15,
  anon_sym_SLASH = 16,
  anon_sym_min_LPAREN_LBRACK = 17,
  anon_sym_RBRACK_RPAREN = 18,
  anon_sym_max_LPAREN_LBRACK = 19,
  anon_sym_EQ = 20,
  anon_sym_BANG_EQ = 21,
  anon_sym_LT_EQ = 22,
  anon_sym_GT_EQ = 23,
  anon_sym_LT = 24,
  anon_sym_GT = 25,
  sym_integer = 26,
  sym_program = 27,
  sym_find_statement = 28,
  sym_variable_list = 29,
  sym_constraint = 30,
  sym_domain = 31,
  sym_int_domain = 32,
  sym_range_list = 33,
  sym_lower_bound_range = 34,
  sym_upper_bound_range = 35,
  sym_closed_range = 36,
  sym_variable = 37,
  sym_expression = 38,
  sym_conjunction = 39,
  sym_comparison = 40,
  sym_addition = 41,
  sym_term = 42,
  sym_factor = 43,
  sym_min = 44,
  sym_max = 45,
  sym_comp_op = 46,
  sym_constant = 47,
  aux_sym_program_repeat1 = 48,
  aux_sym_variable_list_repeat1 = 49,
  aux_sym_range_list_repeat1 = 50,
  aux_sym_expression_repeat1 = 51,
  aux_sym_conjunction_repeat1 = 52,
  aux_sym_comparison_repeat1 = 53,
  aux_sym_addition_repeat1 = 54,
  aux_sym_term_repeat1 = 55,
  aux_sym_min_repeat1 = 56,
};

static const char * const ts_symbol_names[] = {
  [ts_builtin_sym_end] = "end",
  [anon_sym_find] = "find",
  [anon_sym_COLON] = ":",
  [anon_sym_COMMA] = ",",
  [anon_sym_suchthat] = "such that",
  [anon_sym_int] = "int",
  [anon_sym_LPAREN] = "(",
  [anon_sym_RPAREN] = ")",
  [sym_bool_domain] = "bool_domain",
  [anon_sym_DOT_DOT] = "..",
  [sym_identifier] = "identifier",
  [anon_sym_or] = "or",
  [anon_sym_and] = "and",
  [anon_sym_PLUS] = "+",
  [anon_sym_DASH] = "-",
  [anon_sym_STAR] = "*",
  [anon_sym_SLASH] = "/",
  [anon_sym_min_LPAREN_LBRACK] = "min([",
  [anon_sym_RBRACK_RPAREN] = "])",
  [anon_sym_max_LPAREN_LBRACK] = "max([",
  [anon_sym_EQ] = "=",
  [anon_sym_BANG_EQ] = "!=",
  [anon_sym_LT_EQ] = "<=",
  [anon_sym_GT_EQ] = ">=",
  [anon_sym_LT] = "<",
  [anon_sym_GT] = ">",
  [sym_integer] = "integer",
  [sym_program] = "program",
  [sym_find_statement] = "find_statement",
  [sym_variable_list] = "variable_list",
  [sym_constraint] = "constraint",
  [sym_domain] = "domain",
  [sym_int_domain] = "int_domain",
  [sym_range_list] = "range_list",
  [sym_lower_bound_range] = "lower_bound_range",
  [sym_upper_bound_range] = "upper_bound_range",
  [sym_closed_range] = "closed_range",
  [sym_variable] = "variable",
  [sym_expression] = "expression",
  [sym_conjunction] = "conjunction",
  [sym_comparison] = "comparison",
  [sym_addition] = "addition",
  [sym_term] = "term",
  [sym_factor] = "factor",
  [sym_min] = "min",
  [sym_max] = "max",
  [sym_comp_op] = "comp_op",
  [sym_constant] = "constant",
  [aux_sym_program_repeat1] = "program_repeat1",
  [aux_sym_variable_list_repeat1] = "variable_list_repeat1",
  [aux_sym_range_list_repeat1] = "range_list_repeat1",
  [aux_sym_expression_repeat1] = "expression_repeat1",
  [aux_sym_conjunction_repeat1] = "conjunction_repeat1",
  [aux_sym_comparison_repeat1] = "comparison_repeat1",
  [aux_sym_addition_repeat1] = "addition_repeat1",
  [aux_sym_term_repeat1] = "term_repeat1",
  [aux_sym_min_repeat1] = "min_repeat1",
};

static const TSSymbol ts_symbol_map[] = {
  [ts_builtin_sym_end] = ts_builtin_sym_end,
  [anon_sym_find] = anon_sym_find,
  [anon_sym_COLON] = anon_sym_COLON,
  [anon_sym_COMMA] = anon_sym_COMMA,
  [anon_sym_suchthat] = anon_sym_suchthat,
  [anon_sym_int] = anon_sym_int,
  [anon_sym_LPAREN] = anon_sym_LPAREN,
  [anon_sym_RPAREN] = anon_sym_RPAREN,
  [sym_bool_domain] = sym_bool_domain,
  [anon_sym_DOT_DOT] = anon_sym_DOT_DOT,
  [sym_identifier] = sym_identifier,
  [anon_sym_or] = anon_sym_or,
  [anon_sym_and] = anon_sym_and,
  [anon_sym_PLUS] = anon_sym_PLUS,
  [anon_sym_DASH] = anon_sym_DASH,
  [anon_sym_STAR] = anon_sym_STAR,
  [anon_sym_SLASH] = anon_sym_SLASH,
  [anon_sym_min_LPAREN_LBRACK] = anon_sym_min_LPAREN_LBRACK,
  [anon_sym_RBRACK_RPAREN] = anon_sym_RBRACK_RPAREN,
  [anon_sym_max_LPAREN_LBRACK] = anon_sym_max_LPAREN_LBRACK,
  [anon_sym_EQ] = anon_sym_EQ,
  [anon_sym_BANG_EQ] = anon_sym_BANG_EQ,
  [anon_sym_LT_EQ] = anon_sym_LT_EQ,
  [anon_sym_GT_EQ] = anon_sym_GT_EQ,
  [anon_sym_LT] = anon_sym_LT,
  [anon_sym_GT] = anon_sym_GT,
  [sym_integer] = sym_integer,
  [sym_program] = sym_program,
  [sym_find_statement] = sym_find_statement,
  [sym_variable_list] = sym_variable_list,
  [sym_constraint] = sym_constraint,
  [sym_domain] = sym_domain,
  [sym_int_domain] = sym_int_domain,
  [sym_range_list] = sym_range_list,
  [sym_lower_bound_range] = sym_lower_bound_range,
  [sym_upper_bound_range] = sym_upper_bound_range,
  [sym_closed_range] = sym_closed_range,
  [sym_variable] = sym_variable,
  [sym_expression] = sym_expression,
  [sym_conjunction] = sym_conjunction,
  [sym_comparison] = sym_comparison,
  [sym_addition] = sym_addition,
  [sym_term] = sym_term,
  [sym_factor] = sym_factor,
  [sym_min] = sym_min,
  [sym_max] = sym_max,
  [sym_comp_op] = sym_comp_op,
  [sym_constant] = sym_constant,
  [aux_sym_program_repeat1] = aux_sym_program_repeat1,
  [aux_sym_variable_list_repeat1] = aux_sym_variable_list_repeat1,
  [aux_sym_range_list_repeat1] = aux_sym_range_list_repeat1,
  [aux_sym_expression_repeat1] = aux_sym_expression_repeat1,
  [aux_sym_conjunction_repeat1] = aux_sym_conjunction_repeat1,
  [aux_sym_comparison_repeat1] = aux_sym_comparison_repeat1,
  [aux_sym_addition_repeat1] = aux_sym_addition_repeat1,
  [aux_sym_term_repeat1] = aux_sym_term_repeat1,
  [aux_sym_min_repeat1] = aux_sym_min_repeat1,
};

static const TSSymbolMetadata ts_symbol_metadata[] = {
  [ts_builtin_sym_end] = {
    .visible = false,
    .named = true,
  },
  [anon_sym_find] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COLON] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COMMA] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_suchthat] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_int] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RPAREN] = {
    .visible = true,
    .named = false,
  },
  [sym_bool_domain] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_DOT_DOT] = {
    .visible = true,
    .named = false,
  },
  [sym_identifier] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_or] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_and] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_PLUS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DASH] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_STAR] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_SLASH] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_min_LPAREN_LBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACK_RPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_max_LPAREN_LBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_EQ] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_BANG_EQ] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LT_EQ] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_GT_EQ] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_GT] = {
    .visible = true,
    .named = false,
  },
  [sym_integer] = {
    .visible = true,
    .named = true,
  },
  [sym_program] = {
    .visible = true,
    .named = true,
  },
  [sym_find_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_variable_list] = {
    .visible = true,
    .named = true,
  },
  [sym_constraint] = {
    .visible = true,
    .named = true,
  },
  [sym_domain] = {
    .visible = true,
    .named = true,
  },
  [sym_int_domain] = {
    .visible = true,
    .named = true,
  },
  [sym_range_list] = {
    .visible = true,
    .named = true,
  },
  [sym_lower_bound_range] = {
    .visible = true,
    .named = true,
  },
  [sym_upper_bound_range] = {
    .visible = true,
    .named = true,
  },
  [sym_closed_range] = {
    .visible = true,
    .named = true,
  },
  [sym_variable] = {
    .visible = true,
    .named = true,
  },
  [sym_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_conjunction] = {
    .visible = true,
    .named = true,
  },
  [sym_comparison] = {
    .visible = true,
    .named = true,
  },
  [sym_addition] = {
    .visible = true,
    .named = true,
  },
  [sym_term] = {
    .visible = true,
    .named = true,
  },
  [sym_factor] = {
    .visible = true,
    .named = true,
  },
  [sym_min] = {
    .visible = true,
    .named = true,
  },
  [sym_max] = {
    .visible = true,
    .named = true,
  },
  [sym_comp_op] = {
    .visible = true,
    .named = true,
  },
  [sym_constant] = {
    .visible = true,
    .named = true,
  },
  [aux_sym_program_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_variable_list_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_range_list_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_expression_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_conjunction_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_comparison_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_addition_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_term_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_min_repeat1] = {
    .visible = false,
    .named = false,
  },
};

static const TSSymbol ts_alias_sequences[PRODUCTION_ID_COUNT][MAX_ALIAS_SEQUENCE_LENGTH] = {
  [0] = {0},
};

static const uint16_t ts_non_terminal_alias_map[] = {
  0,
};

static const TSStateId ts_primary_state_ids[STATE_COUNT] = {
  [0] = 0,
  [1] = 1,
  [2] = 2,
  [3] = 3,
  [4] = 4,
  [5] = 5,
  [6] = 6,
  [7] = 7,
  [8] = 8,
  [9] = 9,
  [10] = 10,
  [11] = 11,
  [12] = 12,
  [13] = 13,
  [14] = 14,
  [15] = 15,
  [16] = 16,
  [17] = 17,
  [18] = 18,
  [19] = 19,
  [20] = 20,
  [21] = 21,
  [22] = 22,
  [23] = 23,
  [24] = 24,
  [25] = 25,
  [26] = 26,
  [27] = 27,
  [28] = 28,
  [29] = 29,
  [30] = 30,
  [31] = 31,
  [32] = 32,
  [33] = 33,
  [34] = 34,
  [35] = 35,
  [36] = 36,
  [37] = 37,
  [38] = 38,
  [39] = 39,
  [40] = 40,
  [41] = 41,
  [42] = 42,
  [43] = 43,
  [44] = 44,
  [45] = 45,
  [46] = 46,
  [47] = 47,
  [48] = 48,
  [49] = 49,
  [50] = 50,
  [51] = 51,
  [52] = 52,
  [53] = 53,
  [54] = 54,
  [55] = 55,
  [56] = 56,
  [57] = 57,
  [58] = 58,
  [59] = 59,
  [60] = 60,
  [61] = 61,
  [62] = 62,
  [63] = 63,
  [64] = 64,
  [65] = 65,
  [66] = 66,
  [67] = 67,
  [68] = 68,
  [69] = 69,
  [70] = 70,
  [71] = 71,
  [72] = 72,
  [73] = 73,
  [74] = 74,
  [75] = 75,
  [76] = 76,
  [77] = 77,
};

static bool ts_lex(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (eof) ADVANCE(32);
      ADVANCE_MAP(
        '!', 8,
        '(', 38,
        ')', 39,
        '*', 56,
        '+', 54,
        ',', 35,
        '-', 55,
        '.', 7,
        '/', 57,
        ':', 34,
        '<', 65,
        '=', 61,
        '>', 66,
        ']', 6,
        'a', 20,
        'b', 25,
        'f', 18,
        'i', 21,
        'm', 11,
        'o', 26,
        's', 30,
      );
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(0);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(67);
      END_STATE();
    case 1:
      if (lookahead == ' ') ADVANCE(29);
      END_STATE();
    case 2:
      if (lookahead == '(') ADVANCE(38);
      if (lookahead == ',') ADVANCE(35);
      if (lookahead == 's') ADVANCE(49);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(2);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 3:
      if (lookahead == '(') ADVANCE(38);
      if (lookahead == '.') ADVANCE(7);
      if (lookahead == ']') ADVANCE(6);
      if (lookahead == 'm') ADVANCE(45);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(3);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(67);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 4:
      if (lookahead == '(') ADVANCE(9);
      END_STATE();
    case 5:
      if (lookahead == '(') ADVANCE(10);
      END_STATE();
    case 6:
      if (lookahead == ')') ADVANCE(59);
      END_STATE();
    case 7:
      if (lookahead == '.') ADVANCE(41);
      END_STATE();
    case 8:
      if (lookahead == '=') ADVANCE(62);
      END_STATE();
    case 9:
      if (lookahead == '[') ADVANCE(60);
      END_STATE();
    case 10:
      if (lookahead == '[') ADVANCE(58);
      END_STATE();
    case 11:
      if (lookahead == 'a') ADVANCE(31);
      if (lookahead == 'i') ADVANCE(23);
      END_STATE();
    case 12:
      if (lookahead == 'a') ADVANCE(28);
      END_STATE();
    case 13:
      if (lookahead == 'c') ADVANCE(16);
      END_STATE();
    case 14:
      if (lookahead == 'd') ADVANCE(53);
      END_STATE();
    case 15:
      if (lookahead == 'd') ADVANCE(33);
      END_STATE();
    case 16:
      if (lookahead == 'h') ADVANCE(1);
      END_STATE();
    case 17:
      if (lookahead == 'h') ADVANCE(12);
      END_STATE();
    case 18:
      if (lookahead == 'i') ADVANCE(22);
      END_STATE();
    case 19:
      if (lookahead == 'l') ADVANCE(40);
      END_STATE();
    case 20:
      if (lookahead == 'n') ADVANCE(14);
      END_STATE();
    case 21:
      if (lookahead == 'n') ADVANCE(27);
      END_STATE();
    case 22:
      if (lookahead == 'n') ADVANCE(15);
      END_STATE();
    case 23:
      if (lookahead == 'n') ADVANCE(5);
      END_STATE();
    case 24:
      if (lookahead == 'o') ADVANCE(19);
      END_STATE();
    case 25:
      if (lookahead == 'o') ADVANCE(24);
      END_STATE();
    case 26:
      if (lookahead == 'r') ADVANCE(52);
      END_STATE();
    case 27:
      if (lookahead == 't') ADVANCE(37);
      END_STATE();
    case 28:
      if (lookahead == 't') ADVANCE(36);
      END_STATE();
    case 29:
      if (lookahead == 't') ADVANCE(17);
      END_STATE();
    case 30:
      if (lookahead == 'u') ADVANCE(13);
      END_STATE();
    case 31:
      if (lookahead == 'x') ADVANCE(4);
      END_STATE();
    case 32:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    case 33:
      ACCEPT_TOKEN(anon_sym_find);
      END_STATE();
    case 34:
      ACCEPT_TOKEN(anon_sym_COLON);
      END_STATE();
    case 35:
      ACCEPT_TOKEN(anon_sym_COMMA);
      END_STATE();
    case 36:
      ACCEPT_TOKEN(anon_sym_suchthat);
      END_STATE();
    case 37:
      ACCEPT_TOKEN(anon_sym_int);
      END_STATE();
    case 38:
      ACCEPT_TOKEN(anon_sym_LPAREN);
      END_STATE();
    case 39:
      ACCEPT_TOKEN(anon_sym_RPAREN);
      END_STATE();
    case 40:
      ACCEPT_TOKEN(sym_bool_domain);
      END_STATE();
    case 41:
      ACCEPT_TOKEN(anon_sym_DOT_DOT);
      END_STATE();
    case 42:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == ' ') ADVANCE(29);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 43:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == '(') ADVANCE(9);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 44:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == '(') ADVANCE(10);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 45:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == 'a') ADVANCE(50);
      if (lookahead == 'i') ADVANCE(48);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 46:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == 'c') ADVANCE(47);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 47:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == 'h') ADVANCE(42);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 48:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == 'n') ADVANCE(44);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 49:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == 'u') ADVANCE(46);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 50:
      ACCEPT_TOKEN(sym_identifier);
      if (lookahead == 'x') ADVANCE(43);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 51:
      ACCEPT_TOKEN(sym_identifier);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(51);
      END_STATE();
    case 52:
      ACCEPT_TOKEN(anon_sym_or);
      END_STATE();
    case 53:
      ACCEPT_TOKEN(anon_sym_and);
      END_STATE();
    case 54:
      ACCEPT_TOKEN(anon_sym_PLUS);
      END_STATE();
    case 55:
      ACCEPT_TOKEN(anon_sym_DASH);
      END_STATE();
    case 56:
      ACCEPT_TOKEN(anon_sym_STAR);
      END_STATE();
    case 57:
      ACCEPT_TOKEN(anon_sym_SLASH);
      END_STATE();
    case 58:
      ACCEPT_TOKEN(anon_sym_min_LPAREN_LBRACK);
      END_STATE();
    case 59:
      ACCEPT_TOKEN(anon_sym_RBRACK_RPAREN);
      END_STATE();
    case 60:
      ACCEPT_TOKEN(anon_sym_max_LPAREN_LBRACK);
      END_STATE();
    case 61:
      ACCEPT_TOKEN(anon_sym_EQ);
      END_STATE();
    case 62:
      ACCEPT_TOKEN(anon_sym_BANG_EQ);
      END_STATE();
    case 63:
      ACCEPT_TOKEN(anon_sym_LT_EQ);
      END_STATE();
    case 64:
      ACCEPT_TOKEN(anon_sym_GT_EQ);
      END_STATE();
    case 65:
      ACCEPT_TOKEN(anon_sym_LT);
      if (lookahead == '=') ADVANCE(63);
      END_STATE();
    case 66:
      ACCEPT_TOKEN(anon_sym_GT);
      if (lookahead == '=') ADVANCE(64);
      END_STATE();
    case 67:
      ACCEPT_TOKEN(sym_integer);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(67);
      END_STATE();
    default:
      return false;
  }
}

static const TSLexMode ts_lex_modes[STATE_COUNT] = {
  [0] = {.lex_state = 0},
  [1] = {.lex_state = 0},
  [2] = {.lex_state = 3},
  [3] = {.lex_state = 0},
  [4] = {.lex_state = 0},
  [5] = {.lex_state = 3},
  [6] = {.lex_state = 3},
  [7] = {.lex_state = 0},
  [8] = {.lex_state = 0},
  [9] = {.lex_state = 0},
  [10] = {.lex_state = 0},
  [11] = {.lex_state = 0},
  [12] = {.lex_state = 0},
  [13] = {.lex_state = 0},
  [14] = {.lex_state = 0},
  [15] = {.lex_state = 0},
  [16] = {.lex_state = 0},
  [17] = {.lex_state = 3},
  [18] = {.lex_state = 0},
  [19] = {.lex_state = 0},
  [20] = {.lex_state = 0},
  [21] = {.lex_state = 3},
  [22] = {.lex_state = 0},
  [23] = {.lex_state = 0},
  [24] = {.lex_state = 0},
  [25] = {.lex_state = 3},
  [26] = {.lex_state = 0},
  [27] = {.lex_state = 0},
  [28] = {.lex_state = 3},
  [29] = {.lex_state = 0},
  [30] = {.lex_state = 3},
  [31] = {.lex_state = 2},
  [32] = {.lex_state = 2},
  [33] = {.lex_state = 2},
  [34] = {.lex_state = 0},
  [35] = {.lex_state = 3},
  [36] = {.lex_state = 0},
  [37] = {.lex_state = 0},
  [38] = {.lex_state = 0},
  [39] = {.lex_state = 0},
  [40] = {.lex_state = 0},
  [41] = {.lex_state = 2},
  [42] = {.lex_state = 3},
  [43] = {.lex_state = 3},
  [44] = {.lex_state = 0},
  [45] = {.lex_state = 0},
  [46] = {.lex_state = 3},
  [47] = {.lex_state = 3},
  [48] = {.lex_state = 0},
  [49] = {.lex_state = 3},
  [50] = {.lex_state = 0},
  [51] = {.lex_state = 0},
  [52] = {.lex_state = 2},
  [53] = {.lex_state = 0},
  [54] = {.lex_state = 2},
  [55] = {.lex_state = 0},
  [56] = {.lex_state = 0},
  [57] = {.lex_state = 0},
  [58] = {.lex_state = 0},
  [59] = {.lex_state = 2},
  [60] = {.lex_state = 0},
  [61] = {.lex_state = 0},
  [62] = {.lex_state = 0},
  [63] = {.lex_state = 3},
  [64] = {.lex_state = 2},
  [65] = {.lex_state = 0},
  [66] = {.lex_state = 0},
  [67] = {.lex_state = 3},
  [68] = {.lex_state = 0},
  [69] = {.lex_state = 0},
  [70] = {.lex_state = 0},
  [71] = {.lex_state = 0},
  [72] = {.lex_state = 0},
  [73] = {.lex_state = 0},
  [74] = {.lex_state = 0},
  [75] = {.lex_state = 0},
  [76] = {.lex_state = 0},
  [77] = {.lex_state = 0},
};

static const uint16_t ts_parse_table[LARGE_STATE_COUNT][SYMBOL_COUNT] = {
  [0] = {
    [ts_builtin_sym_end] = ACTIONS(1),
    [anon_sym_find] = ACTIONS(1),
    [anon_sym_COLON] = ACTIONS(1),
    [anon_sym_COMMA] = ACTIONS(1),
    [anon_sym_suchthat] = ACTIONS(1),
    [anon_sym_int] = ACTIONS(1),
    [anon_sym_LPAREN] = ACTIONS(1),
    [anon_sym_RPAREN] = ACTIONS(1),
    [sym_bool_domain] = ACTIONS(1),
    [anon_sym_DOT_DOT] = ACTIONS(1),
    [anon_sym_or] = ACTIONS(1),
    [anon_sym_and] = ACTIONS(1),
    [anon_sym_PLUS] = ACTIONS(1),
    [anon_sym_DASH] = ACTIONS(1),
    [anon_sym_STAR] = ACTIONS(1),
    [anon_sym_SLASH] = ACTIONS(1),
    [anon_sym_min_LPAREN_LBRACK] = ACTIONS(1),
    [anon_sym_RBRACK_RPAREN] = ACTIONS(1),
    [anon_sym_max_LPAREN_LBRACK] = ACTIONS(1),
    [anon_sym_EQ] = ACTIONS(1),
    [anon_sym_BANG_EQ] = ACTIONS(1),
    [anon_sym_LT_EQ] = ACTIONS(1),
    [anon_sym_GT_EQ] = ACTIONS(1),
    [anon_sym_LT] = ACTIONS(1),
    [anon_sym_GT] = ACTIONS(1),
    [sym_integer] = ACTIONS(1),
  },
  [1] = {
    [sym_program] = STATE(72),
    [anon_sym_find] = ACTIONS(3),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 14,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(7), 1,
      anon_sym_DOT_DOT,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(15), 1,
      sym_integer,
    STATE(7), 1,
      sym_factor,
    STATE(22), 1,
      sym_term,
    STATE(23), 1,
      sym_addition,
    STATE(36), 1,
      sym_comparison,
    STATE(39), 1,
      sym_conjunction,
    STATE(74), 2,
      sym_range_list,
      sym_expression,
    STATE(50), 3,
      sym_lower_bound_range,
      sym_upper_bound_range,
      sym_closed_range,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [49] = 2,
    ACTIONS(19), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(17), 14,
      ts_builtin_sym_end,
      anon_sym_COLON,
      anon_sym_COMMA,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [70] = 6,
    ACTIONS(21), 1,
      anon_sym_COMMA,
    ACTIONS(23), 1,
      anon_sym_RPAREN,
    ACTIONS(25), 1,
      anon_sym_DOT_DOT,
    STATE(60), 1,
      aux_sym_range_list_repeat1,
    ACTIONS(29), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(27), 10,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [99] = 12,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(31), 1,
      sym_integer,
    STATE(7), 1,
      sym_factor,
    STATE(22), 1,
      sym_term,
    STATE(23), 1,
      sym_addition,
    STATE(36), 1,
      sym_comparison,
    STATE(39), 1,
      sym_conjunction,
    STATE(73), 1,
      sym_expression,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [139] = 12,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(31), 1,
      sym_integer,
    STATE(7), 1,
      sym_factor,
    STATE(22), 1,
      sym_term,
    STATE(23), 1,
      sym_addition,
    STATE(36), 1,
      sym_comparison,
    STATE(39), 1,
      sym_conjunction,
    STATE(69), 1,
      sym_expression,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [179] = 4,
    STATE(9), 1,
      aux_sym_term_repeat1,
    ACTIONS(35), 2,
      anon_sym_STAR,
      anon_sym_SLASH,
    ACTIONS(37), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(33), 10,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [203] = 4,
    STATE(8), 1,
      aux_sym_term_repeat1,
    ACTIONS(41), 2,
      anon_sym_STAR,
      anon_sym_SLASH,
    ACTIONS(44), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(39), 10,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [227] = 4,
    STATE(8), 1,
      aux_sym_term_repeat1,
    ACTIONS(35), 2,
      anon_sym_STAR,
      anon_sym_SLASH,
    ACTIONS(48), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(46), 10,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [251] = 2,
    ACTIONS(29), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(27), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [270] = 2,
    ACTIONS(52), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(50), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [289] = 2,
    ACTIONS(56), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(54), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [308] = 2,
    ACTIONS(60), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(58), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [327] = 2,
    ACTIONS(64), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(62), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [346] = 2,
    ACTIONS(68), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(66), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [365] = 2,
    ACTIONS(44), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(39), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [384] = 11,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(31), 1,
      sym_integer,
    STATE(7), 1,
      sym_factor,
    STATE(22), 1,
      sym_term,
    STATE(23), 1,
      sym_addition,
    STATE(36), 1,
      sym_comparison,
    STATE(53), 1,
      sym_conjunction,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [421] = 2,
    ACTIONS(72), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(70), 12,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_STAR,
      anon_sym_SLASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [440] = 4,
    STATE(19), 1,
      aux_sym_addition_repeat1,
    ACTIONS(76), 2,
      anon_sym_PLUS,
      anon_sym_DASH,
    ACTIONS(79), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(74), 8,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [462] = 4,
    STATE(19), 1,
      aux_sym_addition_repeat1,
    ACTIONS(83), 2,
      anon_sym_PLUS,
      anon_sym_DASH,
    ACTIONS(85), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(81), 8,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [484] = 10,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(31), 1,
      sym_integer,
    STATE(7), 1,
      sym_factor,
    STATE(22), 1,
      sym_term,
    STATE(23), 1,
      sym_addition,
    STATE(45), 1,
      sym_comparison,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [518] = 4,
    STATE(20), 1,
      aux_sym_addition_repeat1,
    ACTIONS(83), 2,
      anon_sym_PLUS,
      anon_sym_DASH,
    ACTIONS(89), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(87), 8,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [540] = 5,
    STATE(24), 1,
      aux_sym_comparison_repeat1,
    STATE(25), 1,
      sym_comp_op,
    ACTIONS(95), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(91), 4,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
    ACTIONS(93), 4,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [563] = 5,
    STATE(25), 1,
      sym_comp_op,
    STATE(26), 1,
      aux_sym_comparison_repeat1,
    ACTIONS(95), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(93), 4,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
    ACTIONS(97), 4,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
  [586] = 9,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(31), 1,
      sym_integer,
    STATE(7), 1,
      sym_factor,
    STATE(22), 1,
      sym_term,
    STATE(29), 1,
      sym_addition,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [617] = 5,
    STATE(25), 1,
      sym_comp_op,
    STATE(26), 1,
      aux_sym_comparison_repeat1,
    ACTIONS(104), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(99), 4,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
    ACTIONS(101), 4,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [640] = 2,
    ACTIONS(79), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(74), 10,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_PLUS,
      anon_sym_DASH,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [657] = 8,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(31), 1,
      sym_integer,
    STATE(7), 1,
      sym_factor,
    STATE(27), 1,
      sym_term,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [685] = 2,
    ACTIONS(107), 2,
      anon_sym_LT,
      anon_sym_GT,
    ACTIONS(99), 8,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
      anon_sym_EQ,
      anon_sym_BANG_EQ,
      anon_sym_LT_EQ,
      anon_sym_GT_EQ,
  [700] = 7,
    ACTIONS(5), 1,
      anon_sym_LPAREN,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(11), 1,
      anon_sym_min_LPAREN_LBRACK,
    ACTIONS(13), 1,
      anon_sym_max_LPAREN_LBRACK,
    ACTIONS(31), 1,
      sym_integer,
    STATE(16), 1,
      sym_factor,
    STATE(11), 4,
      sym_variable,
      sym_min,
      sym_max,
      sym_constant,
  [725] = 6,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(109), 1,
      anon_sym_suchthat,
    STATE(56), 1,
      sym_variable,
    STATE(70), 1,
      sym_constraint,
    STATE(76), 1,
      sym_variable_list,
    STATE(32), 2,
      sym_find_statement,
      aux_sym_program_repeat1,
  [745] = 6,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(109), 1,
      anon_sym_suchthat,
    STATE(56), 1,
      sym_variable,
    STATE(76), 1,
      sym_variable_list,
    STATE(77), 1,
      sym_constraint,
    STATE(33), 2,
      sym_find_statement,
      aux_sym_program_repeat1,
  [765] = 5,
    ACTIONS(111), 1,
      anon_sym_suchthat,
    ACTIONS(113), 1,
      sym_identifier,
    STATE(56), 1,
      sym_variable,
    STATE(76), 1,
      sym_variable_list,
    STATE(33), 2,
      sym_find_statement,
      aux_sym_program_repeat1,
  [782] = 3,
    ACTIONS(7), 1,
      anon_sym_DOT_DOT,
    ACTIONS(116), 1,
      sym_integer,
    STATE(66), 3,
      sym_lower_bound_range,
      sym_upper_bound_range,
      sym_closed_range,
  [794] = 2,
    ACTIONS(120), 1,
      sym_identifier,
    ACTIONS(118), 4,
      anon_sym_LPAREN,
      anon_sym_min_LPAREN_LBRACK,
      anon_sym_max_LPAREN_LBRACK,
      sym_integer,
  [804] = 3,
    ACTIONS(124), 1,
      anon_sym_and,
    STATE(38), 1,
      aux_sym_conjunction_repeat1,
    ACTIONS(122), 3,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
  [816] = 3,
    ACTIONS(128), 1,
      anon_sym_and,
    STATE(37), 1,
      aux_sym_conjunction_repeat1,
    ACTIONS(126), 3,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
  [828] = 3,
    ACTIONS(124), 1,
      anon_sym_and,
    STATE(37), 1,
      aux_sym_conjunction_repeat1,
    ACTIONS(131), 3,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
  [840] = 3,
    ACTIONS(135), 1,
      anon_sym_or,
    STATE(48), 1,
      aux_sym_expression_repeat1,
    ACTIONS(133), 2,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
  [851] = 4,
    ACTIONS(137), 1,
      anon_sym_int,
    ACTIONS(139), 1,
      sym_bool_domain,
    STATE(52), 1,
      sym_int_domain,
    STATE(54), 1,
      sym_domain,
  [864] = 3,
    ACTIONS(143), 1,
      anon_sym_LPAREN,
    ACTIONS(145), 1,
      sym_identifier,
    ACTIONS(141), 2,
      anon_sym_COMMA,
      anon_sym_suchthat,
  [875] = 4,
    ACTIONS(147), 1,
      sym_identifier,
    ACTIONS(150), 1,
      anon_sym_RBRACK_RPAREN,
    STATE(42), 1,
      aux_sym_min_repeat1,
    STATE(71), 1,
      sym_variable,
  [888] = 4,
    ACTIONS(152), 1,
      sym_identifier,
    ACTIONS(154), 1,
      anon_sym_RBRACK_RPAREN,
    STATE(47), 1,
      aux_sym_min_repeat1,
    STATE(71), 1,
      sym_variable,
  [901] = 3,
    ACTIONS(158), 1,
      anon_sym_or,
    STATE(44), 1,
      aux_sym_expression_repeat1,
    ACTIONS(156), 2,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
  [912] = 1,
    ACTIONS(126), 4,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
      anon_sym_and,
  [919] = 4,
    ACTIONS(152), 1,
      sym_identifier,
    ACTIONS(161), 1,
      anon_sym_RBRACK_RPAREN,
    STATE(42), 1,
      aux_sym_min_repeat1,
    STATE(71), 1,
      sym_variable,
  [932] = 4,
    ACTIONS(152), 1,
      sym_identifier,
    ACTIONS(163), 1,
      anon_sym_RBRACK_RPAREN,
    STATE(42), 1,
      aux_sym_min_repeat1,
    STATE(71), 1,
      sym_variable,
  [945] = 3,
    ACTIONS(135), 1,
      anon_sym_or,
    STATE(44), 1,
      aux_sym_expression_repeat1,
    ACTIONS(165), 2,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
  [956] = 4,
    ACTIONS(152), 1,
      sym_identifier,
    ACTIONS(167), 1,
      anon_sym_RBRACK_RPAREN,
    STATE(46), 1,
      aux_sym_min_repeat1,
    STATE(71), 1,
      sym_variable,
  [969] = 3,
    ACTIONS(21), 1,
      anon_sym_COMMA,
    ACTIONS(23), 1,
      anon_sym_RPAREN,
    STATE(60), 1,
      aux_sym_range_list_repeat1,
  [979] = 3,
    ACTIONS(169), 1,
      anon_sym_COLON,
    ACTIONS(171), 1,
      anon_sym_COMMA,
    STATE(55), 1,
      aux_sym_variable_list_repeat1,
  [989] = 2,
    ACTIONS(175), 1,
      sym_identifier,
    ACTIONS(173), 2,
      anon_sym_COMMA,
      anon_sym_suchthat,
  [997] = 1,
    ACTIONS(156), 3,
      ts_builtin_sym_end,
      anon_sym_RPAREN,
      anon_sym_or,
  [1003] = 3,
    ACTIONS(177), 1,
      anon_sym_COMMA,
    ACTIONS(179), 1,
      anon_sym_suchthat,
    ACTIONS(181), 1,
      sym_identifier,
  [1013] = 3,
    ACTIONS(183), 1,
      anon_sym_COLON,
    ACTIONS(185), 1,
      anon_sym_COMMA,
    STATE(55), 1,
      aux_sym_variable_list_repeat1,
  [1023] = 3,
    ACTIONS(171), 1,
      anon_sym_COMMA,
    ACTIONS(188), 1,
      anon_sym_COLON,
    STATE(51), 1,
      aux_sym_variable_list_repeat1,
  [1033] = 2,
    ACTIONS(25), 1,
      anon_sym_DOT_DOT,
    ACTIONS(190), 2,
      anon_sym_COMMA,
      anon_sym_RPAREN,
  [1041] = 2,
    ACTIONS(194), 1,
      sym_integer,
    ACTIONS(192), 2,
      anon_sym_COMMA,
      anon_sym_RPAREN,
  [1049] = 2,
    ACTIONS(198), 1,
      sym_identifier,
    ACTIONS(196), 2,
      anon_sym_COMMA,
      anon_sym_suchthat,
  [1057] = 3,
    ACTIONS(21), 1,
      anon_sym_COMMA,
    ACTIONS(200), 1,
      anon_sym_RPAREN,
    STATE(61), 1,
      aux_sym_range_list_repeat1,
  [1067] = 3,
    ACTIONS(190), 1,
      anon_sym_RPAREN,
    ACTIONS(202), 1,
      anon_sym_COMMA,
    STATE(61), 1,
      aux_sym_range_list_repeat1,
  [1077] = 1,
    ACTIONS(183), 2,
      anon_sym_COLON,
      anon_sym_COMMA,
  [1082] = 2,
    ACTIONS(152), 1,
      sym_identifier,
    STATE(62), 1,
      sym_variable,
  [1089] = 2,
    ACTIONS(205), 1,
      anon_sym_suchthat,
    ACTIONS(207), 1,
      sym_identifier,
  [1096] = 1,
    ACTIONS(209), 2,
      anon_sym_COMMA,
      anon_sym_RPAREN,
  [1101] = 1,
    ACTIONS(190), 2,
      anon_sym_COMMA,
      anon_sym_RPAREN,
  [1106] = 1,
    ACTIONS(150), 2,
      sym_identifier,
      anon_sym_RBRACK_RPAREN,
  [1111] = 1,
    ACTIONS(211), 2,
      anon_sym_COMMA,
      anon_sym_RPAREN,
  [1116] = 1,
    ACTIONS(213), 1,
      anon_sym_RPAREN,
  [1120] = 1,
    ACTIONS(215), 1,
      ts_builtin_sym_end,
  [1124] = 1,
    ACTIONS(217), 1,
      anon_sym_COMMA,
  [1128] = 1,
    ACTIONS(219), 1,
      ts_builtin_sym_end,
  [1132] = 1,
    ACTIONS(221), 1,
      ts_builtin_sym_end,
  [1136] = 1,
    ACTIONS(223), 1,
      anon_sym_RPAREN,
  [1140] = 1,
    ACTIONS(225), 1,
      sym_integer,
  [1144] = 1,
    ACTIONS(227), 1,
      anon_sym_COLON,
  [1148] = 1,
    ACTIONS(229), 1,
      ts_builtin_sym_end,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(2)] = 0,
  [SMALL_STATE(3)] = 49,
  [SMALL_STATE(4)] = 70,
  [SMALL_STATE(5)] = 99,
  [SMALL_STATE(6)] = 139,
  [SMALL_STATE(7)] = 179,
  [SMALL_STATE(8)] = 203,
  [SMALL_STATE(9)] = 227,
  [SMALL_STATE(10)] = 251,
  [SMALL_STATE(11)] = 270,
  [SMALL_STATE(12)] = 289,
  [SMALL_STATE(13)] = 308,
  [SMALL_STATE(14)] = 327,
  [SMALL_STATE(15)] = 346,
  [SMALL_STATE(16)] = 365,
  [SMALL_STATE(17)] = 384,
  [SMALL_STATE(18)] = 421,
  [SMALL_STATE(19)] = 440,
  [SMALL_STATE(20)] = 462,
  [SMALL_STATE(21)] = 484,
  [SMALL_STATE(22)] = 518,
  [SMALL_STATE(23)] = 540,
  [SMALL_STATE(24)] = 563,
  [SMALL_STATE(25)] = 586,
  [SMALL_STATE(26)] = 617,
  [SMALL_STATE(27)] = 640,
  [SMALL_STATE(28)] = 657,
  [SMALL_STATE(29)] = 685,
  [SMALL_STATE(30)] = 700,
  [SMALL_STATE(31)] = 725,
  [SMALL_STATE(32)] = 745,
  [SMALL_STATE(33)] = 765,
  [SMALL_STATE(34)] = 782,
  [SMALL_STATE(35)] = 794,
  [SMALL_STATE(36)] = 804,
  [SMALL_STATE(37)] = 816,
  [SMALL_STATE(38)] = 828,
  [SMALL_STATE(39)] = 840,
  [SMALL_STATE(40)] = 851,
  [SMALL_STATE(41)] = 864,
  [SMALL_STATE(42)] = 875,
  [SMALL_STATE(43)] = 888,
  [SMALL_STATE(44)] = 901,
  [SMALL_STATE(45)] = 912,
  [SMALL_STATE(46)] = 919,
  [SMALL_STATE(47)] = 932,
  [SMALL_STATE(48)] = 945,
  [SMALL_STATE(49)] = 956,
  [SMALL_STATE(50)] = 969,
  [SMALL_STATE(51)] = 979,
  [SMALL_STATE(52)] = 989,
  [SMALL_STATE(53)] = 997,
  [SMALL_STATE(54)] = 1003,
  [SMALL_STATE(55)] = 1013,
  [SMALL_STATE(56)] = 1023,
  [SMALL_STATE(57)] = 1033,
  [SMALL_STATE(58)] = 1041,
  [SMALL_STATE(59)] = 1049,
  [SMALL_STATE(60)] = 1057,
  [SMALL_STATE(61)] = 1067,
  [SMALL_STATE(62)] = 1077,
  [SMALL_STATE(63)] = 1082,
  [SMALL_STATE(64)] = 1089,
  [SMALL_STATE(65)] = 1096,
  [SMALL_STATE(66)] = 1101,
  [SMALL_STATE(67)] = 1106,
  [SMALL_STATE(68)] = 1111,
  [SMALL_STATE(69)] = 1116,
  [SMALL_STATE(70)] = 1120,
  [SMALL_STATE(71)] = 1124,
  [SMALL_STATE(72)] = 1128,
  [SMALL_STATE(73)] = 1132,
  [SMALL_STATE(74)] = 1136,
  [SMALL_STATE(75)] = 1140,
  [SMALL_STATE(76)] = 1144,
  [SMALL_STATE(77)] = 1148,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = true}}, SHIFT(31),
  [5] = {.entry = {.count = 1, .reusable = true}}, SHIFT(6),
  [7] = {.entry = {.count = 1, .reusable = true}}, SHIFT(75),
  [9] = {.entry = {.count = 1, .reusable = false}}, SHIFT(3),
  [11] = {.entry = {.count = 1, .reusable = true}}, SHIFT(49),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(43),
  [15] = {.entry = {.count = 1, .reusable = true}}, SHIFT(4),
  [17] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable, 1, 0, 0),
  [19] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_variable, 1, 0, 0),
  [21] = {.entry = {.count = 1, .reusable = true}}, SHIFT(34),
  [23] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_range_list, 1, 0, 0),
  [25] = {.entry = {.count = 1, .reusable = true}}, SHIFT(58),
  [27] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_constant, 1, 0, 0),
  [29] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_constant, 1, 0, 0),
  [31] = {.entry = {.count = 1, .reusable = true}}, SHIFT(10),
  [33] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_term, 1, 0, 0),
  [35] = {.entry = {.count = 1, .reusable = true}}, SHIFT(30),
  [37] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_term, 1, 0, 0),
  [39] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_term_repeat1, 2, 0, 0),
  [41] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_term_repeat1, 2, 0, 0), SHIFT_REPEAT(30),
  [44] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_term_repeat1, 2, 0, 0),
  [46] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_term, 2, 0, 0),
  [48] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_term, 2, 0, 0),
  [50] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_factor, 1, 0, 0),
  [52] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_factor, 1, 0, 0),
  [54] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_min, 2, 0, 0),
  [56] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_min, 2, 0, 0),
  [58] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_max, 2, 0, 0),
  [60] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_max, 2, 0, 0),
  [62] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_min, 3, 0, 0),
  [64] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_min, 3, 0, 0),
  [66] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_factor, 3, 0, 0),
  [68] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_factor, 3, 0, 0),
  [70] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_max, 3, 0, 0),
  [72] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_max, 3, 0, 0),
  [74] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_addition_repeat1, 2, 0, 0),
  [76] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_addition_repeat1, 2, 0, 0), SHIFT_REPEAT(28),
  [79] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_addition_repeat1, 2, 0, 0),
  [81] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_addition, 2, 0, 0),
  [83] = {.entry = {.count = 1, .reusable = true}}, SHIFT(28),
  [85] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_addition, 2, 0, 0),
  [87] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_addition, 1, 0, 0),
  [89] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_addition, 1, 0, 0),
  [91] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_comparison, 1, 0, 0),
  [93] = {.entry = {.count = 1, .reusable = true}}, SHIFT(35),
  [95] = {.entry = {.count = 1, .reusable = false}}, SHIFT(35),
  [97] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_comparison, 2, 0, 0),
  [99] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_comparison_repeat1, 2, 0, 0),
  [101] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_comparison_repeat1, 2, 0, 0), SHIFT_REPEAT(35),
  [104] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_comparison_repeat1, 2, 0, 0), SHIFT_REPEAT(35),
  [107] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_comparison_repeat1, 2, 0, 0),
  [109] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [111] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_program_repeat1, 2, 0, 0),
  [113] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_program_repeat1, 2, 0, 0), SHIFT_REPEAT(3),
  [116] = {.entry = {.count = 1, .reusable = true}}, SHIFT(57),
  [118] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_comp_op, 1, 0, 0),
  [120] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_comp_op, 1, 0, 0),
  [122] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_conjunction, 1, 0, 0),
  [124] = {.entry = {.count = 1, .reusable = true}}, SHIFT(21),
  [126] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_conjunction_repeat1, 2, 0, 0),
  [128] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_conjunction_repeat1, 2, 0, 0), SHIFT_REPEAT(21),
  [131] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_conjunction, 2, 0, 0),
  [133] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_expression, 1, 0, 0),
  [135] = {.entry = {.count = 1, .reusable = true}}, SHIFT(17),
  [137] = {.entry = {.count = 1, .reusable = true}}, SHIFT(41),
  [139] = {.entry = {.count = 1, .reusable = true}}, SHIFT(52),
  [141] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_int_domain, 1, 0, 0),
  [143] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [145] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_int_domain, 1, 0, 0),
  [147] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_min_repeat1, 2, 0, 0), SHIFT_REPEAT(3),
  [150] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_min_repeat1, 2, 0, 0),
  [152] = {.entry = {.count = 1, .reusable = true}}, SHIFT(3),
  [154] = {.entry = {.count = 1, .reusable = true}}, SHIFT(13),
  [156] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_expression_repeat1, 2, 0, 0),
  [158] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_expression_repeat1, 2, 0, 0), SHIFT_REPEAT(17),
  [161] = {.entry = {.count = 1, .reusable = true}}, SHIFT(14),
  [163] = {.entry = {.count = 1, .reusable = true}}, SHIFT(18),
  [165] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_expression, 2, 0, 0),
  [167] = {.entry = {.count = 1, .reusable = true}}, SHIFT(12),
  [169] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_list, 2, 0, 0),
  [171] = {.entry = {.count = 1, .reusable = true}}, SHIFT(63),
  [173] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_domain, 1, 0, 0),
  [175] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_domain, 1, 0, 0),
  [177] = {.entry = {.count = 1, .reusable = true}}, SHIFT(64),
  [179] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_find_statement, 3, 0, 0),
  [181] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_find_statement, 3, 0, 0),
  [183] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_variable_list_repeat1, 2, 0, 0),
  [185] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_variable_list_repeat1, 2, 0, 0), SHIFT_REPEAT(63),
  [188] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_list, 1, 0, 0),
  [190] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_range_list_repeat1, 2, 0, 0),
  [192] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_lower_bound_range, 2, 0, 0),
  [194] = {.entry = {.count = 1, .reusable = true}}, SHIFT(68),
  [196] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_int_domain, 4, 0, 0),
  [198] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_int_domain, 4, 0, 0),
  [200] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_range_list, 2, 0, 0),
  [202] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_range_list_repeat1, 2, 0, 0), SHIFT_REPEAT(34),
  [205] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_find_statement, 4, 0, 0),
  [207] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_find_statement, 4, 0, 0),
  [209] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_upper_bound_range, 2, 0, 0),
  [211] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_closed_range, 3, 0, 0),
  [213] = {.entry = {.count = 1, .reusable = true}}, SHIFT(15),
  [215] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_program, 2, 0, 0),
  [217] = {.entry = {.count = 1, .reusable = true}}, SHIFT(67),
  [219] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
  [221] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_constraint, 2, 0, 0),
  [223] = {.entry = {.count = 1, .reusable = true}}, SHIFT(59),
  [225] = {.entry = {.count = 1, .reusable = true}}, SHIFT(65),
  [227] = {.entry = {.count = 1, .reusable = true}}, SHIFT(40),
  [229] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_program, 3, 0, 0),
};

#ifdef __cplusplus
extern "C" {
#endif
#ifdef TREE_SITTER_HIDE_SYMBOLS
#define TS_PUBLIC
#elif defined(_WIN32)
#define TS_PUBLIC __declspec(dllexport)
#else
#define TS_PUBLIC __attribute__((visibility("default")))
#endif

TS_PUBLIC const TSLanguage *tree_sitter_essence(void) {
  static const TSLanguage language = {
    .version = LANGUAGE_VERSION,
    .symbol_count = SYMBOL_COUNT,
    .alias_count = ALIAS_COUNT,
    .token_count = TOKEN_COUNT,
    .external_token_count = EXTERNAL_TOKEN_COUNT,
    .state_count = STATE_COUNT,
    .large_state_count = LARGE_STATE_COUNT,
    .production_id_count = PRODUCTION_ID_COUNT,
    .field_count = FIELD_COUNT,
    .max_alias_sequence_length = MAX_ALIAS_SEQUENCE_LENGTH,
    .parse_table = &ts_parse_table[0][0],
    .small_parse_table = ts_small_parse_table,
    .small_parse_table_map = ts_small_parse_table_map,
    .parse_actions = ts_parse_actions,
    .symbol_names = ts_symbol_names,
    .symbol_metadata = ts_symbol_metadata,
    .public_symbol_map = ts_symbol_map,
    .alias_map = ts_non_terminal_alias_map,
    .alias_sequences = &ts_alias_sequences[0][0],
    .lex_modes = ts_lex_modes,
    .lex_fn = ts_lex,
    .primary_state_ids = ts_primary_state_ids,
  };
  return &language;
}
#ifdef __cplusplus
}
#endif

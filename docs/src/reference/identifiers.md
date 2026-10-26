# Identifiers

An identifier is a unique name assigned to a [value](/docs/src/reference/statements/letting.md#value-declaration), [parameter](/docs/src/reference/statements/given.md), [decision variable](/docs/src/reference/statements/find.md), or [named domain](/docs/src/reference/statements/letting.md#domain-declaration).

```bnf
<Identifier> ::= `[a-zA-Z_][a-zA-Z0-9_]*'*`
```

Put simply:

- Identifiers consists of letters, numbers and underscores.
- Identifiers must start with a letter or underscore.
- Identifiers can optionally end with any number of apostrophes (for Haskell-like or mathematical notation).

Examples of valid identifiers:

```essence
a
my_2nd_var
x0'
_
__ab__c''
___'''
```

Examples of invalid identifiers:

```essence
2             $ refers to the integer 2 (use e.g. _2 or var2 instead)
8queens       $ cannot start with number
abc'def       $ apostrophe can only be at the end
my#var        $ cannot contain non-alphanumeric characters
hello-world   $ use hello_world instead
```

# Relations

## Relation Domains

; QUESTION: should accept expressions? argue yes.

```bnf
<RelationDomain> ::= "relation" ["(" list(<RelationAttribute>) ")"] "of" "(" <RelationDomainList> ")"
<RelationDomainList> ::= <Domain> [{ "*" <domain> }]
<RelationAttribute> ::= "size" <Expression.Integer>
                      | "minSize" <Expression.Integer>
                      | "maxSize" <Expression.Integer>
                      | "reflexive"
                      | "irreflexive"
                      | "coreflexive"
                      | "symmetric"
                      | "antiSymmetric"
                      | "aSymmetric"
                      | "transitive"
                      | "total"
                      | "connex"
                      | "Euclidean"
                      | "serial"
                      | "equivalence"
                      | "partialOrder"
                      | "leftTotal"
                      | "rightTotal"
                      | "linearOrder"
                      | "weakOrder"
                      | "preOrder"
                      | "strictPartialOrder"
```

A relation is effectively a set of tuples which relate elements of different domains.

- Every tuple in the relation must have the same arity and internal domains.
- If a relation is binary (arity 2) we can also add additional attributes to define properties of the relation.

### Attributes

#### Size

| Attribute | Domain     | Description                                   |
| --------- | ---------- | --------------------------------------------- |
| `size`    | `int(0..)` | The exact number of tuples in the relation.   |
| `minSize` | `int(0..)` | The minimum number of tuples in the relation. |
| `maxSize` | `int(0..)` | The maximum number of tuples in the relation. |

A range of sizes may be specified using `maxSize x minSize y`.

#### Binary

Binary attributes determine properties of **binary relations only** (relations with tuple arity 2).

| Attribute       | Domain | Description |
| --------------- | ------ | ----------- |
| `reflexive`     | `bool` | TODO        |
| `irreflexive`   | `bool` | TODO        |
| `coreflexive`   | `bool` | TODO        |
| `symmetric`     | `bool` | TODO        |
| `antiSymmetric` | `bool` | TODO        |
| `aSymmetric`    | `bool` | TODO        |
| `transitive`    | `bool` | TODO        |
| `total`         | `bool` | TODO        |
| `connex`        | `bool` | TODO        |
| `euclidean`     | `bool` | TODO        |
| `serial`        | `bool` | TODO        |
| `equivalence`   | `bool` | TODO        |
| `partialOrder`  | `bool` | TODO        |

### Relation Literals

```bnf
<RelationLiteral> ::= "relation" "(" [list(<RelationLiteralTuple>)] ")"
<RelationLiteralTuple> ::= "(" list(<Expression>) ")"
```

# Relations

## Relation Domains

; QUESTION: should accept expressions? argue yes.

```bnf
<RelationDomain> ::= "relation" ["(" list(<RelationAttribute>) ")"] "of" "(" <RelationDomainList> ")"
<RelationDomainList> ::= <Domain> [{ "*" <Domain> }]
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

```essence
$ binary relations
letting a be domain relation of (int * bool)"
letting b be domain relation (partialOrder) of (int(1..2) * int(1..2))"

$ n-ary relation
letting c be domain relation(int, int, int)
```

### Attributes

_N.B.: in this section `A ~ B` denotes "`A` is related to `B`", and `A !~ B` denotes `!(A ~ B)`._

#### Size

| Attribute | Domain     | Description                                   |
| --------- | ---------- | --------------------------------------------- |
| `size`    | `int(0..)` | The exact number of tuples in the relation.   |
| `minSize` | `int(0..)` | The minimum number of tuples in the relation. |
| `maxSize` | `int(0..)` | The maximum number of tuples in the relation. |

A range of sizes may be specified using `maxSize x minSize y`.

#### Reflexivity

Only applicable to relations of domain `D * D`.

| Attribute     | Domain | Description               |
| ------------- | ------ | ------------------------- |
| `reflexive`   | `bool` | For all `a`, `a ~ a`.     |
| `irreflexive` | `bool` | For all `a`, `a !~ a`.    |
| `coreflexive` | `bool` | If `a ~ b`, then `a = b`. |

#### Symmetry

TODO binary and categorisation

| Attribute       | Domain | Description                             |
| --------------- | ------ | --------------------------------------- |
| `symmetric`     | `bool` | If `a ~ b`, then `b ~ a`.               |
| `antiSymmetric` | `bool` | If `a ~ b` and `a != b`, then `b !~ a`. |
| `aSymmetric`    | `bool` | If `a ~ b`, then `b !~ a`.              |

#### Transitivity

TODO binary and categorisation

| Attribute      | Domain | Description                                                                           |
| -------------- | ------ | ------------------------------------------------------------------------------------- |
| `transitive`   | `bool` | If `a ~ b` and `b ~ c`, then `a ~ c`.                                                 |
| `euclidean`    | `bool` | If `a ~ b` and `a ~ c`, then `b ~ c`.                                                 |
| `total`        | `bool` | Every element of the first domain must be related to an element of the second domain. |
| `connex`       | `bool` | All elements in either domain are related (TODO).                                     |
| `serial`       | `bool` | TODO                                                                                  |
| `equivalence`  | `bool` | Equivalent to setting both `reflexive` and `symmetric`.                               |
| `partialOrder` | `bool` | Equivalent to setting both `reflective` and `antiSymmetric`.                          |

### Relation Literals

```bnf
<RelationLiteral> ::= "relation" "(" [list(<RelationLiteralTuple>)] ")"
<RelationLiteralTuple> ::= "(" list(<Expression>) ")"
```

```essence
$ binary relation domains
letting a be relation((1,1,0),(1,0,1),(0,1,1))
letting b be relation((true, false), (false, true))
```

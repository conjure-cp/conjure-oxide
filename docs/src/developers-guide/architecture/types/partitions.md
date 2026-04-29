# Partitions
## What are Partitions
A partition is a datatype that splits some domain into parts. A good example of a partition in use is in the [Social Golfers](https://www.csplib.org/Problems/prob010/models/SocialGolfersProblem.essence.html) problem (CSPLib.org, problem #10). 

## Attributes
* Number of Parts (as a range): `numParts`, `minNumParts`, `maxNumParts`
* Cardinality of each Part (as a range): `partSize`, `minPartSize`, `maxPartSize`
* Regularity (i.e. every part has the same cardinality): `regular`

For example:
```
find foo : partition (maxNumParts 3, partSize 1) from int(1..5) 
$ ({1})                 i.e. 1 part of cardinality 1
$ ... 
$ ({2}, {4}, {5})       i.e. 3 parts of cardinality 3
$ ({3}, {4}, {5})       i.e. 3 parts of cardinality 3

find bar : partition (minNumParts 1, maxNumParts 3, minPartSize 2, maxPartSize 4, regular) from int(1..20)
$ ({1, 2})                           i.e. 1 part of cadinality 2
$ ({1,2,5}, {3,4,12}, {6,7,15})      i.e. 3 parts of cardinality 3
$ 
$ ({2,3}, {5})                       this would be invalid for two reasons: 
$                                       > it is not regular (different cardinalities)
$                                       > there is a minPartSize of 2, and one part has a cardinality of 1
```

## Operators
* `apart`: test if a list of elements are not all contained in one part of the partition
* `participants`: union of all parts of a partition
* `party`: part of partition that contains specified element
* `parts`: partition to its set of parts
* `together`: test if a list of elements are all in the same part of the partition

> As of writing (26/04/2026): none of these Operators are implemented in conjure-oxide, partitions cannot be parsed using the native parser, and there is no support for rewriting or solving with partitions.
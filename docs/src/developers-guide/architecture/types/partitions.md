# Partitions
## What are Partitions
A partitions is a datatype that splits some domain into parts. A good example of a partition in use is in the [Social Golfers](https://www.csplib.org/Problems/prob010/models/SocialGolfersProblem.essence.html) problem (CSPLib.org, problem #10). 

## Attributes
* Number of Parts (as a range): `numParts`, `minNumParts`, `maxNumParts`
* Cardinality of each Part (as a range): `partSize`, `minPartSize`, `maxPartSize`
* Regularity (i.e. every part has the same cardinality): `regular`

## Operators
* `apart`: test if a list of elements are not all contained in one part of the partition
* `participants`: union of all parts of a partition
* `party`: part of partition that contains specified element
* `parts`: partition to its set of parts
* `together`: test if a list of elements are all in the same part of the partition

> As of writing (26/04/2026), none of these Operators are implemented in conjure-oxide. 
> As of writing (26/04/2026), Partitions cannot be parsed using the native parser, they must be parsed with the legacy parser. 
> As of writing (26/04/2026), there is no support for rewriting or solving with partitions.
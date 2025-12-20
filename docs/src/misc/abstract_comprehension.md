# AbstractComprehension

`Abstract Comprehensions` are a distinct version of a `Comprehension` that is, as of now, used when creating Set Rules. `Abstract Comprehensions` are currently a temporary feature and may be significantly refactored or integrated into the current `Comprehension` type. 

## Comprehensions Use

Comprehensions can be used to construct representations of lists. A comprehension is declared using square brackets '[]' that contain a generator expression (such as a = i) followed by a "|" and a comma separated list of conditions that are boolean expressions. 
Examples include:
[i-1 | i <- [5,6,7]] 
or 
[i = a | a in b] (in-set rule)

More details on the use of comprehensions in the language of Essence can be found at: https://conjure.readthedocs.io/en/latest/essence.html#comprehensions

## Distinctions between the Abstract Comprehension Type and Comprehension Type
The original motivation for creating abstract comprehensions was to allow for domains to be extracted from Expressions, like existing sets or matrices. In it's current iteration, a Comprehension type requires a Declaration when generating a domain (eg int(1..5)) which makes it difficult to implement set rules such as intersect or union which generate a domain from an existing set. 

Abstract comprehensions have a single symbol table for return expressions and generators which are split up in the original comprehension builder. The model for an abstract comprehension resembles the features in the original conjure model more closely. 




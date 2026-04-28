# Domain Refinement
## The Project
In Conjure-Oxide when we convert Essence-level domains into lower-level domains for the solver they can often be blown-up necessarily large, which negatively impacts solver performance.

We can see an example of this in the following Essence code:<br>
```
find a : set (maxSize 2) of int(1..10)
find b : set of int(1..1000)
such that b subset a
```
<br>
Instead of being treated as a set of maximum size 1000, we can deduce that `b` has a maximum size of 1.

There is lot of domain refinement that can be done at an Essence level, making it a very interesting project. This has the potential to make measurable improvements to Conjure-Oxide in a way that goes beyond Conjure, and is the first step in an Essence-level solver.

## How Would It Work
To carry out domain refinement we suggest creating an arc-consistency-style algorithm. This can be run between parsing and rule rewriting, to infer the minimal domains of all variables before they are expanded. The algorithm would iterate over constraints, stored as expressions in Conjure-Oxide, and refine the internal variable’s domains. Not all constraints in Conjure-Oxide are binary, and so a form of generalised arc consistency might be necessary. Once the domains have been refined any constraints also containing the newly changed variable must be rechecked for further refinements.

## What Are The Current Roadblocks
Currently all expressions contain the domain of the result of the expression. The first task and a current roadblock would be to propagate this expression domain to the internal variables involved. Conjure-Oxide does not currently use the expression domains in any way when it comes to rewriting.

A final roadblock for this project is how to test it. In specific cases, where you know a refinement has occurred, you can use Conjure-Oxide Pretty’s new expression-domains option to view all domains in the model. However, for a more comprehensive testing you need a large number of test cases. One consideration was using a LLM to generate these cases. As of Dec-2025 the free AI models included in APIs were not capable of doing this at scale. 

## What Has Been Done So Far
Before work on the consistency algorithm could have began it was important that the domain of expressions is actually fully-tightened. This will allow for maximum inference. As such, all work up to this point has been focused on tightening the domains returned from the `Expression::domain_of()` function.

### Set Operators

### Function Operators
The operators on functions have a lot of potential to be tightened due to them resulting in sets and functions. These operators include `defined`, `range`, `imageSet`, `preimage`, and `restrict`. For a function domain we can infer information from both its attributes and the length of its domain and codomain. 

To understand this, consider the defined operator, which results in a set of all domain values defined for the function. This has the domain of a set containing the same domain as the function. However, we can infer a tighter set domain using set attributes. For this operator, the size attributes of the function directly map to the set. We consider the partiality and jectivity attributes.

For example:
- If the function is total we know the size of the set will be the size of the function’s domain;
- If the function is injective we know we cannot exceed the size of the codomain, because every domain element must map to a different codomain element;
- If the function is surjective or bijective then the size of the set must be either at least the size of the codomain or exactly equal to it respectively, so every codomain element is defined.

Whilst these conditions are specific to defined as an operator, similarly logic is applied to all other function operators. All partiality and jectivity inference is based around the length of the domain / codomain, and so can only be done if the domain is ground.

There is however, one currently known limitation, so the results are not always completely minimal. We cannot guarantee a domain is ground and the lengths are known, so there are specific cases where we could infer simple bounds based on domain size, but the added complexity is not worth it in individual parts of the code. In these cases, the size is left as UnboundedR. Instead of adding these extra checks, I propose a separate small project could be to add the inferred domain size of a function as size attributes to that function, when the domain lengths are first known as ground. This means we do not need to separately check the domain size every time.

### Partition Operators
There are 5 operators for Partitions, three return sets (`Parts`, `Party`, `Participants`) and two return booleans (`Together`, `Apart`). The current implementations of these may be improved upon, and have just been added as a part of supporting the Partition type.

### Sequence Operators
There are 2 operators for Sequences (`Subsequence`, `Substring`), and booth return booleans. They are both of the form `s sub.. t`, where `s` must occur in `t`. You can conclude that either a `Subsequence` or `Substring` is definitively false if `s` is strictly larger than `t`, but as of writing this documentation there is no mechanism to 'restrict' the domain of a boolean to being strictly true or false.

When it becomes possible to alter the domains of children of an expression, `s` and `t` may be able to trim each others domains. Take the following example:
```
find s : sequence (minSize 3, maxSize 5) ...
find t : sequence (minSize 2, maxSize 4) ...
s subsequence t
```
- You can alter the attribute of `s` such that it has a maximum size of 4 (it cannot be longer than `t`)
- You can alter the attributes of `t` such that it has a minimum size of 3 (it cannot be shorter than `s`)
This was quickly attempted in [PR #1786](https://github.com/conjure-cp/conjure-oxide/pull/1786) just for experimentation, but was abandoned because there was not a clear way of mutating the state inside the `Moo` - after all, the Moo exists to stop that from happening. 


### Cardinality Operator
One other operator which can be tightened is the cardinality operator. As opposed to returning an unbounded integer, we can make inferences on the maximum size of a type using its attributes and domain length. This inference is also limited by the domains being ground. When the domains are ground, we can get an exact length for matrices, and a tightened bound for the size of set, multisets, relations and functions using their attributes.

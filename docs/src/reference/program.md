# Problem Specification

```bnf
<ProblemSpecification> ::= { <Statement> }
```

A problem specification in Essence is composed of a list of [statements](reference/statements/statements.md) Statements can declare [decision variables](reference/statements/find.md), [parameters](reference/statements/given.md), or [aliases](reference/statements/letting.md). They can also post [constraints](reference/statements/such-that.md), [parameter conditions](reference/statements/where.md), and [dominance relations](reference/statements/dominance-relations.md)

## Comments

Single-line comments begin with a `$` character.

```bnf
<SingleLineComment> ::= "$" `.*`
```

```
$ this is a comment

letting a be 1    $ this is also a comment
```

Essence does not currently support multi-line comments.

## Parameter File(s)

A parameter file is an Essence file containing [`letting` statements](reference/statements/letting.md) **only**. Passing one or more parameter files to Conjure Oxide will bind all parameter files' `letting`s to their respective `given` statements in the problem specification.

```essence
$ problem.essence
given a: int(0..10)

$ params.essence
letting a be 5
```

Passing a parameter value which falls outside the parameter's domain or breaks a [`where` constraint](reference/statements/where.md) will result in an error.

```essence
$ problem.essence
given a: int(0..10)

$ params.essence
letting a be 13   $ error!
```

<pre>
  <code class="language-bnf"><a href="">&lt;Domain&gt;</a> ::= hi </code>
</pre>

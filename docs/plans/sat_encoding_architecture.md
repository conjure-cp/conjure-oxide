# Oxide SAT Encoding Architecture: Detailed Implementation Plan

Recovered from the user's browser conversation on 2026-09-30. Updated on 2026-10-02 to distinguish BinaryValue, BinaryOffset, and BinaryRank. This is the target architecture, not a record of implemented features. See `sat_ir_status.md` for progress.

The initial test portfolio uses `channelling = "uniform"`: one representation kind per outer type family throughout each model, with no channelling. Domain bounds and element types do not split a family. Matrix layout and integer encoding remain separate choices.

The existing AST already gives declarations stable object identities; implementation should reuse those identities at the SAT IR boundary rather than replacing identities with names or unnecessarily migrating the entire AST.


## AST ownership and clause-generation boundary (2026-10-02)

Modelling and encoding decisions are first-class, serialisable AST nodes owned by the model. They retain semantic operands, per-use representation requests, decomposition choices, algorithm/provider/options, assertion or reification mode, and provenance. Rewriting ends with these decisions; it does not construct CNF expression trees.

```text
semantic model AST
  → modelling decisions (decomposition, representations, channelling)
  → encoding decisions (constraint, selected algorithm, options)
  → SAT adaptor: shared literal allocation + RustSAT/Pindakaas encoding
  → library-owned clauses / solver / DIMACS
```

The target `Model` and `SerdeModel` have no CNF field. Remove `CnfClause`, `RuleEffect::new_clauses`, clause-producing Boolean/integer rules and the legacy converters after migrating their semantic decisions. Encoding auxiliaries created only by a library remain in the adaptor and never become model declarations. Clause statistics and DIMACS are solver artifacts, separate from AST snapshots.

The initial implementation places the typed decision arena in `Model::sat_encoding`; the existing clause field remains temporarily for the production integer path. The SAT adaptor accepts terminal Boolean decision ASTs directly and rejects mixed legacy inputs. This is a migration step, not completion of CNF removal. Non-SAT adaptors reject SAT decision inputs.

Reuse the verified encoders in [the library inventory](sat_encoder_inventory.md). Keep RustSAT as solver/allocation infrastructure and evaluate Pindakaas as an additional encoding provider, starting with sorting networks, BDD and SWC. Selection stays visible in Oxide's AST; library defaults or normalization must not silently substitute an unrecorded algorithm or representation.

## 1. Objective

Extend Oxide so that:

1. each semantic E′ variable may have one or more SAT representations;
2. different references to the same variable may request different representations;
3. representations are created lazily;
4. channeling constraints are added automatically whenever multiple representations of the same semantic variable coexist;
5. encoding decisions are represented explicitly and independently of clause generation;
6. multiple alternative encodings can coexist and later be selected by heuristics;
7. native and externally wrapped clause generators can be used interchangeably;
8. every implementation stage has robust unit, property, integration, and regression tests.

The central distinction is:

```text
semantic variable
    ≠ variable reference
    ≠ representation instance
    ≠ SAT literal
```

A semantic variable has one mathematical value. Individual references to that variable may be encoded using different representations, but all representations must denote the same value.

---

# 2. Core architectural model

## 2.1 Semantic variables

Every E′ decision or auxiliary variable must have a stable identity independent of its occurrences in the AST.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemanticVarId(u32);
```

The semantic variable record should include at least:

```rust
pub struct SemanticVariable {
    pub id: SemanticVarId,
    pub name: Option<String>,
    pub domain: Domain,
    pub origin: VariableOrigin,
}
```

Possible origins:

```rust
pub enum VariableOrigin {
    User,
    RewriteAuxiliary,
    EncodingAuxiliary,
}
```

Encoding auxiliary variables that exist only as SAT literals may not need semantic variable identities. However, any auxiliary integer or Boolean variable introduced before final CNF generation should receive a `SemanticVarId`.

### Invariant

All AST references to the same mathematical variable must point to the same `SemanticVarId`.

---

## 2.2 Variable references and use sites

A reference is an occurrence of a semantic variable in a constraint or expression.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VarRefId(u32);
```

```rust
pub struct VariableReference {
    pub id: VarRefId,
    pub variable: SemanticVarId,
    pub context: ReferenceContext,
    pub source_location: Option<SourceLocation>,
}
```

`ReferenceContext` should record why the occurrence exists and what operations use it:

```rust
pub enum ReferenceContext {
    Equality,
    Disequality,
    Comparison,
    LinearTerm { coefficient: i64 },
    AllDifferentMember,
    TableColumn,
    ElementIndex,
    ElementValue,
    ObjectiveTerm,
    ReifiedConstraint,
    Other,
}
```

A single AST node may contain several `VarRefId` values referring to the same semantic variable.

### Important design rule

Representations are selected for uses, but materialised for semantic variables.

For example:

```text
reference r1 to x requests Direct
reference r2 to x requests Order
reference r3 to x requests Binary
```

This produces:

```text
x:
    Direct representation
    Order representation
    Binary representation
```

not three separate direct/order/binary copies per occurrence.

The use-site decision determines which shared representation each occurrence consumes.

---

## 2.3 Representation kinds

Start with:

```rust
pub enum RepresentationKind {
    Boolean,
    Direct,
    Order,
    BinaryValue,
    BinaryOffset,
    BinaryRank,
}
```

Potential later extensions:

```rust
pub enum RepresentationKind {
    Boolean,
    Direct,
    Order,
    BinaryValue,
    BinaryOffset,
    BinaryRank,
    MixedRadix,
    SparseDirect,
}
```

Semantics:

- `Boolean`: a native Boolean variable.
- `Direct`: one equality indicator per domain value.
- `Order`: threshold literals over the ordered domain.
- `BinaryValue`: bits encode the actual numeric value, with explicit unsigned or two's-complement semantics.
- `BinaryOffset`: unsigned bits encode `x - min(domain)`.
- `BinaryRank`: bits encode the index of the value in the ordered domain.

`BinaryValue` and `BinaryRank` must remain distinct because they behave differently for sparse domains.

---

## 2.4 Representation identity

Each representation instance belongs to one semantic variable and one representation kind.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RepresentationId(u32);
```

```rust
pub struct RepresentationInstance {
    pub id: RepresentationId,
    pub variable: SemanticVarId,
    pub kind: RepresentationKind,
    pub data: RepresentationData,
}
```

```rust
pub enum RepresentationData {
    Boolean(BooleanRepresentation),
    Direct(DirectRepresentation),
    Order(OrderRepresentation),
    BinaryValue(BinaryRepresentation),
    BinaryOffset(BinaryRepresentation),
    BinaryRank(BinaryRepresentation),
}
```

The registry must enforce uniqueness:

```text
(variable x, Direct)      → at most one representation
(variable x, Order)       → at most one representation
(variable x, BinaryValue) → at most one representation
```

---

## 2.5 Representation requests

Rules must not generate clauses directly. They should emit representation requests and encoding plans.

```rust
pub struct RepresentationRequest {
    pub reference: VarRefId,
    pub variable: SemanticVarId,
    pub kind: RepresentationKind,
    pub reason: RepresentationReason,
}
```

```rust
pub enum RepresentationReason {
    RequiredByEncoding(EncodingPlanId),
    PreferredByHeuristic,
    RequiredForDecoding,
    RequiredForChanneling,
    ExplicitUserChoice,
}
```

The request is attached to a use site. Resolution maps it to the shared representation instance:

```rust
pub struct ResolvedReference {
    pub reference: VarRefId,
    pub representation: RepresentationId,
}
```

---

# 3. Phase 0: establish baseline and guardrails

## 3.1 Record current behaviour

Before structural changes:

1. run the complete existing test suite;
2. record the number of passing, ignored, and failing tests;
3. generate representative CNF outputs for a fixed corpus;
4. record solver results and decoded solutions;
5. record performance metrics where stable enough:
   - variables;
   - clauses;
   - literal occurrences;
   - translation time;
   - solve time.

Create a versioned baseline fixture directory:

```text
tests/baseline/
    models/
    expected-status/
    expected-solutions/
    expected-cnf-statistics/
```

Avoid requiring exact CNF equality unless deterministic clause output is already an intentional invariant. Prefer semantic and structural assertions.

## 3.2 Add deterministic allocation mode

Tests will be substantially easier if semantic variable, representation, auxiliary variable, and SAT literal allocation can be deterministic.

Add a deterministic mode enabled in tests:

```rust
EncodingConfig {
    deterministic: true,
    ...
}
```

### Test gate

Do not proceed until:

- all existing tests pass;
- baseline results are captured;
- running the same model twice produces the same semantic result;
- deterministic mode produces stable IDs and clause ordering where expected.

---

# 4. Phase 1: introduce stable semantic variable identities

## 4.1 Replace variable-name-based identity

Audit all places where variables are identified by:

- names;
- AST pointers;
- expression equality;
- indices local to a constraint;
- cloned AST nodes.

Replace these with `SemanticVarId`.

Maintain names only as metadata.

## 4.2 Add a semantic variable arena

```rust
pub struct SemanticVariableArena {
    variables: Vec<SemanticVariable>,
}
```

Essential methods:

```rust
impl SemanticVariableArena {
    pub fn insert(
        &mut self,
        name: Option<String>,
        domain: Domain,
        origin: VariableOrigin,
    ) -> SemanticVarId;

    pub fn get(&self, id: SemanticVarId) -> &SemanticVariable;

    pub fn domain(&self, id: SemanticVarId) -> &Domain;
}
```

## 4.3 Domain canonicalisation

Representation creation depends on domains. Introduce a canonical domain form.

Suggested initial form:

```rust
pub enum Domain {
    Bool,
    Interval { lower: i64, upper: i64 },
    Explicit(Vec<i64>),
}
```

Canonicalisation rules:

- explicit values are sorted;
- duplicates are removed;
- a contiguous explicit domain may become an interval;
- empty domains are rejected or represented explicitly as unsatisfiable;
- singleton domains remain valid.

### Tests

Unit tests:

- two references to the same declared variable share one `SemanticVarId`;
- two variables with the same name in different scopes receive different IDs;
- cloned expressions preserve semantic identities;
- auxiliary variables receive unique IDs;
- domain canonicalisation is deterministic.

Property tests:

- canonicalisation preserves domain membership;
- canonicalisation is idempotent.

Integration tests:

- existing models solve identically after conversion to semantic IDs.

### Phase gate

No representation work begins until every AST variable reference is backed by a stable semantic identity.

---

# 5. Phase 2: introduce explicit variable references

## 5.1 Assign `VarRefId` values

During AST normalisation or E′ construction, assign each variable occurrence a stable reference identity.

```rust
pub struct ReferenceArena {
    references: Vec<VariableReference>,
}
```

A variable appearing twice in one expression gets two reference IDs:

```text
x + x
```

produces:

```text
r1 → x
r2 → x
```

This is necessary because the two use sites may later receive different encoding decisions.

## 5.2 Preserve reference identity through rewrites

Define clear rewrite behaviour:

- a rewrite that preserves an occurrence should preserve its `VarRefId`;
- a rewrite that duplicates an occurrence should create fresh `VarRefId` values pointing to the same `SemanticVarId`;
- a rewrite that introduces a genuinely new semantic variable creates both a new `SemanticVarId` and references to it;
- a rewrite that replaces a reference with a constant removes that use site.

Provide helper APIs so individual rules do not implement this inconsistently.

```rust
pub struct RewriteContext<'a> {
    pub variables: &'a mut SemanticVariableArena,
    pub references: &'a mut ReferenceArena,
}
```

Helpers:

```rust
fn clone_reference_for_new_use(&mut self, original: VarRefId) -> VarRefId;
fn new_reference(&mut self, variable: SemanticVarId, context: ReferenceContext) -> VarRefId;
```

### Tests

Unit tests:

- `x + x` has two references but one semantic variable;
- duplicating a subexpression produces fresh references;
- all duplicated references retain the original semantic identity;
- deleting a rewritten branch removes its references from the live plan.

Integration test:

- create a model where one occurrence of `x` is in `allDifferent` and another is in arithmetic;
- verify that the occurrence contexts are distinct.

---

# 6. Phase 3: define encoding plans independently of clauses

## 6.1 Encoding plan IR

Introduce an encoding-oriented IR that records decisions without allocating SAT literals.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EncodingPlanId(u32);
```

```rust
pub struct EncodingPlan {
    pub id: EncodingPlanId,
    pub source_constraint: ConstraintId,
    pub kind: EncodingPlanKind,
    pub required_representations: Vec<RepresentationRequest>,
    pub capabilities: EncodingCapabilities,
    pub provenance: PlanProvenance,
}
```

Example plan kinds:

```rust
pub enum EncodingPlanKind {
    BooleanTseitin(BooleanTseitinPlan),
    Amo(AmoPlan),
    Cardinality(CardinalityPlan),
    PseudoBoolean(PseudoBooleanPlan),
    LinearInteger(LinearIntegerPlan),
    AllDifferent(AllDifferentPlan),
    Table(TablePlan),
    Element(ElementPlan),
    Equality(IntegerEqualityPlan),
    Comparison(IntegerComparisonPlan),
}
```

Plans may contain child plans, but avoid arbitrary nesting where a flat dependency graph is clearer.

## 6.2 Explicit alternatives

Represent algorithm alternatives as enums:

```rust
pub enum AmoEncoding {
    Pairwise,
    Ladder,
    Product,
    Commander,
    Bimander,
    Tree,
    Bitwise,
}
```

```rust
pub enum PbEncoding {
    Mdd,
    Gpw,
    Lpw,
    Swc,
    Ggt,
    Rggt,
    Ggth,
    Gmto,
    Tree,
    BinaryAdder,
    Bdd,
    GeneralisedTotaliser,
}
```

```rust
pub enum TableEncoding {
    Tuple,
    Mdd,
    BinarySupport,
}
```

A plan must say exactly which alternative was selected.

## 6.3 Plan provenance

```rust
pub enum PlanProvenance {
    DefaultRule { rule: RuleId },
    ExplicitConfiguration,
    Heuristic { heuristic: String },
    Fallback { rejected_plan: EncodingPlanId },
}
```

This will later support explanations, debugging, and learned heuristics.

## 6.4 Decision phase output

The decision phase should produce:

```rust
pub struct EncodingDecision {
    pub plans: Vec<EncodingPlan>,
    pub reference_requirements: HashMap<VarRefId, RepresentationKind>,
}
```

A reference should normally consume one primary representation in a given plan. Multiple requirements may arise if the same reference participates in multiple derived constraints. Resolve these carefully rather than silently overwriting them.

Prefer:

```rust
HashMap<VarRefId, SmallVec<[RepresentationKind; 2]>>
```

until conflict handling is explicit.

### Tests

Unit tests:

- plans contain no SAT literals;
- plans are serialisable and printable;
- each algorithm alternative is distinguishable;
- representation requirements refer to use sites;
- two references to the same semantic variable can request different representations.

Golden tests:

- print the plan for small E′ examples;
- compare readable plan output rather than CNF.

Example expected diagnostic:

```text
variable x : 0..15
  ref r3 in allDifferent -> Direct
  ref r9 in x + y <= 12  -> Order
  ref r14 in objective   -> BinaryValue
```

---

# 7. Phase 4: representation registry and lazy materialisation

## 7.1 Registry

```rust
pub struct RepresentationRegistry {
    by_variable_and_kind:
        HashMap<(SemanticVarId, RepresentationKind), RepresentationId>,
    representations: Vec<RepresentationInstance>,
}
```

Essential operation:

```rust
pub fn get_or_create(
    &mut self,
    variable: SemanticVarId,
    kind: RepresentationKind,
    ctx: &mut ClauseGenerationContext,
) -> Result<RepresentationId>;
```

`get_or_create` must:

1. return the existing representation if present;
2. otherwise validate domain compatibility;
3. allocate required SAT variables;
4. emit representation-internal validity clauses;
5. register the representation;
6. request channeling to all existing representations of the same semantic variable.

## 7.2 Representation materialisation must be idempotent

Calling:

```rust
get_or_create(x, Direct)
```

multiple times must allocate one representation and emit its validity clauses once.

## 7.3 Separate metadata creation from clause generation

Prefer two stages:

```rust
fn allocate_representation(...);
fn emit_representation_invariants(...);
```

This avoids re-entrancy problems when channeling one representation requires access to another.

Possible workflow:

1. allocate representation metadata;
2. register it as pending;
3. allocate SAT literals;
4. emit internal clauses;
5. mark complete;
6. create channeling edges.

Track lifecycle:

```rust
pub enum RepresentationState {
    Allocating,
    Ready,
}
```

Detect recursive allocation errors rather than producing duplicate structures.

---

# 8. Phase 5: implement native representations

## 8.1 Boolean representation

For `Domain::Bool`, allocate a single SAT literal.

```rust
pub struct BooleanRepresentation {
    pub literal: Lit,
}
```

Map:

```text
false ↔ ¬literal
true  ↔ literal
```

Tests:

- both truth values decode correctly;
- fixed Boolean domains simplify correctly;
- repeated creation is idempotent.

---

## 8.2 Direct representation

```rust
pub struct DirectRepresentation {
    pub values: Vec<i64>,
    pub equals: Vec<Lit>,
}
```

Semantics:

```text
equals[i] ↔ x = values[i]
```

Internal validity:

- at least one value is selected;
- at most one value is selected.

The AMO implementation should itself be selected by a subordinate encoding plan rather than hard-coded permanently.

Initially use native pairwise AMO, then allow alternative AMO generators.

Special cases:

- empty domain: emit contradiction;
- singleton domain: use a constant true literal or unit clause;
- Boolean domain: either reuse Boolean representation or support direct as a two-literal view. Prefer a defined canonical policy.

Tests:

- exactly one direct literal is true;
- sparse domains work;
- singleton domains work;
- decoded values are correct;
- invalid assignments are unsatisfiable.

---

## 8.3 Order representation

For ordered values:

```text
v0 < v1 < ... < vn
```

choose and document one convention.

Recommended:

```text
leq[i] ↔ x ≤ values[i]
```

Do not allocate a literal for a threshold that is always false or always true unless uniformity clearly outweighs efficiency.

```rust
pub struct OrderRepresentation {
    pub values: Vec<i64>,
    pub leq: Vec<ThresholdLiteral>,
}
```

Internal validity:

```text
x ≤ vi → x ≤ vi+1
```

For sparse domains, thresholds should correspond to meaningful cut points between allowed values.

Example domain:

```text
{1, 4, 9}
```

may use:

```text
x ≤ 1
x ≤ 4
```

with `x ≤ 9` represented as constant true.

Tests:

- monotonicity;
- all domain values have exactly one corresponding threshold pattern;
- no non-domain value is decoded;
- sparse domains behave correctly;
- lower and upper bound literals are handled consistently.

---

## 8.4 Binary offset representation

Recommended initial semantics:

```text
encoded = x - lower_bound
```

for contiguous domains.

```rust
pub struct BinaryRepresentation {
    pub bits: Vec<Lit>,
    pub semantics: BinarySemantics,
    pub bit_width: usize,
    pub lower_bound: i64,
    pub allowed_values: Vec<i64>,
}
```

```rust
pub enum BinarySemantics {
    UnsignedOffset,
    UnsignedAbsolute,
    TwosComplement,
    Rank,
}
```

`BinaryOffset` uses `UnsignedOffset`. `BinaryValue` uses unsigned actual values for nonnegative domains or signed two's-complement actual values for domains containing negatives. Arithmetic must widen without wraparound.

For a domain of size not equal to a power of two, emit a range restriction.

For sparse domains, initially reject `BinaryOffset` unless the domain is contiguous; later add explicit membership encoding. Selecting `BinaryRank` must be a separate, recorded representation decision, never a silent fallback.

The safest staged approach is:

- Phase 5: `BinaryOffset` only for intervals;
- later: support sparse numeric domains through membership encoding.

Tests:

- all valid values encode and decode;
- unused bit patterns are unsatisfiable;
- lower bounds other than zero work;
- negative domains work via offset;
- singleton domains use zero bits or a fixed representation consistently;
- domains crossing zero work.

---

### Actual-value representation

BinaryValue encodes the actual value. For nonnegative domains its unsigned bit width covers the maximum value; for domains containing negatives its signed two's-complement width covers both bounds. Restrict patterns to the declared domain, including sparse membership. BinaryOffset instead encodes the unsigned difference from the domain minimum. Keep signedness and offset metadata explicit.

## 8.5 Binary rank representation

For sparse domain:

```text
D = [v0, v1, ..., vn]
```

bits encode index `i`, and value is `vi`.

Tests:

- every rank maps to the correct domain value;
- out-of-range ranks are forbidden;
- arithmetic encodings do not mistakenly treat rank bits as value bits;
- diagnostics distinguish rank from value representation.

---

# 9. Phase 6: channeling architecture

## 9.1 Channeling graph

Channeling should be represented explicitly rather than emitted ad hoc.

```rust
pub struct ChannelingRegistry {
    emitted: HashSet<ChannelingKey>,
}
```

```rust
pub struct ChannelingKey {
    pub variable: SemanticVarId,
    pub left: RepresentationKind,
    pub right: RepresentationKind,
}
```

Canonicalise ordering so:

```text
Direct ↔ Order
```

and:

```text
Order ↔ Direct
```

are the same edge.

## 9.2 Channeling policy

When a new representation is created, there are two broad strategies.

### Complete pairwise channeling

Channel the new representation directly to every existing representation.

For three representations:

```text
Direct ↔ Order
Direct ↔ Binary
Order  ↔ Binary
```

Advantages:

- potentially stronger propagation;
- no dependence on a hub representation;
- each pair has explicit semantics.

Disadvantages:

- more clauses;
- more implementations;
- possibly redundant.

### Spanning-tree channeling

Choose a canonical hub, for example Direct:

```text
Order ↔ Direct ↔ Binary
```

Advantages:

- fewer channeling encodings;
- simpler implementation.

Disadvantages:

- weaker propagation;
- direct representation may be expensive;
- creating a hub solely for channeling may defeat representation choices.

### Recommended policy

Support both, but initially implement:

```rust
pub enum ChannelingTopology {
    PairwiseExisting,
    MinimalSpanning,
    PreferredHub(RepresentationKind),
}
```

Default initially to `PairwiseExisting` for correctness and simpler semantics. Add cost-aware topology selection later.

Do not create an unrequested hub representation merely to simplify channeling unless a plan explicitly chooses that strategy.

---

## 9.3 Direct ↔ order channeling

For each domain value \(v_i\), connect:

```text
x = vi
```

to the corresponding order pattern.

For sorted values:

```text
v0 < v1 < ... < vn
```

with `leq[i]` meaning \(x \le v_i\):

```text
x = v0 ↔ leq[0]
x = vi ↔ ¬leq[i-1] ∧ leq[i]
x = vn ↔ ¬leq[n-1]
```

Possible encoding:

```text
eq_i → lower-pattern
eq_i → upper-pattern
pattern → eq_i
```

Optimise constants at boundaries.

Tests:

- each direct value forces its exact order pattern;
- each valid order pattern forces the correct direct value;
- inconsistent assignments are unsatisfiable;
- channeling remains correct for sparse domains.

---

## 9.4 Direct ↔ binary channeling

For every allowed value \(v\):

```text
x = v ↔ bits = encoding(v)
```

Naive encoding:

```text
eq_v → each bit literal
bit pattern → eq_v
```

The reverse implication can be one clause containing:

```text
mismatch_1 ∨ mismatch_2 ∨ ... ∨ eq_v
```

This is suitable as a first correct implementation.

Later alternatives:

- BDD channeling;
- shared decoding circuits;
- binary decision trees;
- implication-only channeling where equisatisfiability is sufficient and decoding is managed elsewhere.

Tests:

- each direct literal determines all bits;
- each valid bit pattern determines one direct literal;
- invalid bit patterns remain forbidden;
- offset and rank semantics are distinguished.

---

## 9.5 Order ↔ binary channeling

Implement directly rather than relying on Direct unless Direct is already present.

For each threshold:

```text
x ≤ c ↔ binary_comparator(bits, c)
```

Start with a simple comparator circuit or truth-table encoding for small widths. Prefer a reusable comparator plan because the same machinery will support integer comparison constraints.

Tests:

- threshold literals agree with all binary assignments;
- all boundaries work;
- negative lower bounds and offsets work;
- no Direct representation is created as a side effect.

---

## 9.6 Channeling transitivity and duplicate suppression

Creating representations in any order must lead to equivalent channeling.

Test all creation permutations:

```text
Direct, Order, Binary
Direct, Binary, Order
Order, Direct, Binary
Order, Binary, Direct
Binary, Direct, Order
Binary, Order, Direct
```

Assertions:

- each representation is created once;
- each required channeling pair is emitted once;
- solver-visible semantics are identical;
- clause count is deterministic under deterministic mode, or differences are documented if creation order affects topology.

---

# 10. Phase 7: clause-generation interface

## 10.1 Backend-neutral clause sink

```rust
pub trait ClauseSink {
    fn new_var(&mut self, origin: SatVarOrigin) -> Var;
    fn add_clause(&mut self, clause: &[Lit]);
}
```

Optional later methods:

```rust
fn add_unit(&mut self, lit: Lit);
fn add_binary(&mut self, a: Lit, b: Lit);
fn supports_native_xor(&self) -> bool;
```

Do not expose solver-specific APIs to encoding plans.

## 10.2 Clause generator trait

```rust
pub trait ClauseGenerator<P> {
    fn generate(
        &self,
        plan: &P,
        ctx: &mut ClauseGenerationContext,
    ) -> Result<GeneratedEncoding>;
}
```

```rust
pub struct ClauseGenerationContext<'a> {
    pub variables: &'a SemanticVariableArena,
    pub references: &'a ReferenceArena,
    pub representations: &'a mut RepresentationRegistry,
    pub channeling: &'a mut ChannelingRegistry,
    pub clauses: &'a mut dyn ClauseSink,
    pub statistics: &'a mut EncodingStatistics,
}
```

## 10.3 Generated encoding record

```rust
pub struct GeneratedEncoding {
    pub plan_id: EncodingPlanId,
    pub output_literals: Vec<Lit>,
    pub auxiliary_variables: Vec<Var>,
    pub statistics: LocalEncodingStatistics,
}
```

The returned object should support:

- reified constraints;
- parent plans consuming child-plan results;
- diagnostics;
- testing.

---

# 11. Phase 8: native foundational generators

Implement natively first:

1. Boolean Tseitin encoding;
2. pairwise AMO;
3. ladder AMO;
4. direct representation validity;
5. order representation validity;
6. binary range restriction;
7. direct/order channeling;
8. direct/binary channeling;
9. order/binary channeling;
10. equality and disequality over each representation.

These form a dependency-free correctness core.

## Testing pattern for each generator

For every encoding with up to a manageable domain size:

1. enumerate all assignments to the semantic input variables;
2. calculate the expected truth value mathematically;
3. fix input representations to that assignment;
4. invoke SAT;
5. verify satisfiable exactly when expected;
6. test all possible output/reification literal values;
7. optionally enumerate all auxiliary assignments for very small encodings.

This should be packaged as reusable test infrastructure.

Example:

```rust
assert_encoding_equivalent(
    semantic_domain,
    semantic_predicate,
    generated_cnf,
);
```

---

# 12. Phase 9: representation-aware primitive constraints

Implement E′ primitive constraints one family at a time.

## 12.1 Equality

Support alternatives:

```text
Direct equality
Order equality
Binary equality
Mixed-representation equality
```

A rule may choose:

- request the same representation on both sides;
- encode directly between different representations;
- request extra representations and channel.

Do not assume both operands use the same representation.

Examples:

```text
x[Direct] = y[Direct]
x[Order] = y[Order]
x[Binary] = y[Binary]
x[Direct] = y[Binary]
```

For mixed equality, consider whether direct mixed encoding is cheaper than adding a second representation.

## 12.2 Disequality

Similarly support:

- direct;
- order;
- binary;
- mixed.

## 12.3 Comparisons

Support:

```text
x < y
x ≤ y
x > y
x ≥ y
```

Initial alternatives:

- order-order comparison;
- binary comparator;
- table/support encoding for small domains;
- direct support encoding.

## 12.4 Constant comparisons

Optimise:

```text
x = c
x ≠ c
x ≤ c
x < c
```

These should normally consume existing representation literals directly.

### Tests

For each primitive:

- all representation combinations;
- all small domains;
- unequal domains;
- sparse domains where supported;
- singleton domains;
- negative values;
- reified and non-reified forms.

---

# 13. Phase 10: structural constraint plans

## 13.1 `allDifferent`

Provide at least two alternatives initially.

### Value-wise direct decomposition

For each value \(v\):

```text
AMO([x1 = v, x2 = v, ...])
```

This requests Direct representation for each participating reference.

### Pairwise disequality

```text
xi ≠ xj
```

The disequality encoding may choose Direct, Order, Binary, or mixed forms.

This is important because requiring Direct for every `allDifferent` occurrence may be expensive for large domains.

Later alternatives:

- binary disequality network;
- pigeonhole/cardinality encodings;
- matching-based or Hall-inspired encodings where appropriate.

Tests:

- all permutations satisfy;
- duplicate assignments fail;
- variables may simultaneously have Binary representations elsewhere;
- channeling is created when the same variables are used in arithmetic.

---

## 13.2 Cardinality

Support:

```text
atMostK
atLeastK
exactlyK
```

Inputs may be Boolean expressions rather than only variables.

Initial alternatives:

- totaliser;
- sequential counter;
- sorting network;
- PB fallback.

Rules produce a `CardinalityPlan`; generators create clauses.

Tests:

- all Boolean assignments for small arities;
- boundary values \(k=0\), \(k=n\), \(k<0\), \(k>n\);
- reification;
- incremental strengthening if supported.

---

## 13.3 Pseudo-Boolean

Canonicalise:

```text
Σ ai bi relation k
```

before choosing encoding.

Normalisation should handle:

- negative coefficients;
- constant terms;
- repeated literals;
- complementary literals;
- trivial or contradictory bounds;
- equality splitting where required by an encoder.

Initial generators:

- native simple adder or totaliser baseline;
- wrapped RustSAT alternatives;
- wrapped Pindakaas alternatives where exact selection remains under Oxide control.

Tests:

- exhaustive small instances;
- random weighted instances checked against direct evaluation;
- cross-backend equivalence;
- negative and zero coefficients;
- equality, lower-bound, and upper-bound forms.

---

## 13.4 Linear integer constraints

Represent:

```text
Σ ai xi relation k
```

with references carrying independently selected representations.

Initial alternatives:

### Order-tree

- request Order for each term;
- introduce order-represented intermediate sums;
- encode ternary additions.

### Direct-to-PB

- request Direct for each term;
- expand integer values into weighted Boolean terms;
- pass to selected PB encoder.

### Binary-adder

- request BinaryValue where supported;
- construct weighted bit sum;
- use adder/comparator encoding.

### Mixed representation

Permit some terms to be Direct, some Order, and some Binary. The plan may:

- normalise all terms into a common PB form;
- use specialised mixed encoders;
- create additional representations selectively.

Tests:

- random small linear constraints checked exhaustively;
- heterogeneous domains;
- mixed representations within one sum;
- same semantic variable appearing multiple times with different references;
- negative coefficients;
- equality and inequalities;
- reified constraints.

---

# 14. Phase 11: external backend adapters

## 14.1 Adapter rule

An adapter must realise an already selected encoding. It must not silently choose a different algorithm.

Good:

```text
Plan: PB using GeneralisedTotaliser
Generator: RustSAT adapter
```

Bad:

```text
Plan: generic PB
Generator: external library chooses anything
```

## 14.2 RustSAT adapter

Implement conversions between:

- Oxide literals and RustSAT literals;
- Oxide clause sink and RustSAT collector;
- Oxide bounds and RustSAT encoding API;
- RustSAT auxiliary variables and Oxide provenance metadata.

Test each wrapped encoding against the native baseline on small exhaustive instances.

## 14.3 Pindakaas adapter

Wrap only APIs where Oxide can control:

- selected algorithm;
- input representation;
- output clauses;
- variable allocation or mapping.

Do not delegate semantic variable representation ownership.

## 14.4 Differential tests

For any encoding implemented by multiple backends:

```text
native pairwise AMO
RustSAT pairwise AMO
Pindakaas pairwise AMO
```

compare:

- semantic correctness;
- satisfiability;
- projected solutions;
- clause and variable statistics;
- reification behaviour.

Exact CNF equality is unnecessary.

---

# 15. Phase 12: heuristic-ready decision system

## 15.1 Candidate generation

Rules should generate compatible candidate plans.

```rust
pub struct EncodingCandidate {
    pub plan: EncodingPlan,
    pub estimated_cost: EstimatedEncodingCost,
    pub preconditions: Vec<PlanPrecondition>,
}
```

Example for one linear constraint:

```text
Candidate 1:
    term representation: Order
    encoding: Tree

Candidate 2:
    term representation: Direct
    encoding: MDD

Candidate 3:
    term representation: BinaryValue
    encoding: BinaryAdder
```

Candidate generation must consider existing representations.

If `x` already has Direct, the marginal cost of using Direct is lower. If selecting Binary would create a new representation and two channeling edges, the cost model must include those costs.

## 15.2 Global rather than purely local costs

The same representation may be shared by many constraints. Therefore:

```text
cost(candidate)
    ≠ cost of constraint encoding alone
```

Use:

```text
total marginal cost =
    constraint clauses
  + newly required representation clauses
  + newly required channeling clauses
  - shared representation benefits
```

Initial heuristic may remain simple, but the data model must support global reasoning.

## 15.3 Decision ledger

Record:

```rust
pub struct DecisionLedger {
    pub selected_candidates: Vec<SelectedCandidate>,
    pub rejected_candidates: Vec<RejectedCandidate>,
}
```

Include rejection reasons:

```text
unsupported sparse domain
requires unavailable reification
estimated clause count too high
external backend disabled
```

This is valuable for debugging future heuristics.

---

# 16. Phase 13: decoding and model validation

## 16.1 Semantic decoding

Decode each semantic variable once, even if it has multiple representations.

Preferred strategy:

1. choose a canonical available representation;
2. decode the value;
3. verify every other representation agrees;
4. report an internal error on disagreement.

```rust
pub fn decode_variable(
    variable: SemanticVarId,
    model: &SatModel,
    registry: &RepresentationRegistry,
) -> Result<i64>;
```

## 16.2 Cross-representation validation

In debug and test builds:

```text
Direct says x = 4
Order says x = 4
Binary says x = 4
```

must agree.

This validation catches missing or one-directional channeling bugs.

## 16.3 Solution projection

Only user-visible semantic variables should normally appear in final solutions. Rewrite and encoding auxiliaries should remain hidden unless diagnostics request them.

### Tests

- models with one, two, and three representations per variable;
- decoding from each available representation;
- injected inconsistent SAT models are detected;
- projected solutions match E′ semantics.

---

# 17. Phase 14: test infrastructure

## 17.1 Four test levels

### Unit tests

Test isolated data structures and clause generators.

Examples:

- representation registry idempotence;
- domain mapping;
- one channeling pair;
- one AMO encoder.

### Property tests

Use `proptest` or equivalent.

Properties:

- every semantic value has a valid encoding;
- invalid bit patterns are rejected;
- channeling preserves values;
- representation creation order does not change semantics;
- normalisation is idempotent;
- all generated clauses refer to allocated variables.

### Integration tests

Run complete E′ models through:

```text
parse
→ rewrite
→ plan
→ select
→ materialise representations
→ channel
→ generate CNF
→ solve
→ decode
```

### Differential tests

Compare:

- old Oxide pipeline versus new pipeline;
- Oxide versus Savile Row on supported E′ subsets;
- native versus RustSAT;
- native versus Pindakaas;
- multiple encoding alternatives against one another.

---

## 17.2 Exhaustive semantic test harness

Build a reusable harness for small models.

For each assignment to semantic variables:

1. evaluate the E′ constraint directly;
2. add assumptions fixing representations to the assignment;
3. solve generated CNF;
4. compare SAT result with direct evaluation.

This harness should support:

```rust
check_constraint_encoding(
    domains,
    semantic_constraint,
    encoding_configuration,
);
```

Use small domains such as:

```text
Bool
0..2
-1..2
{0, 2, 5}
```

---

## 17.3 Channeling matrix tests

For each supported pair:

| Left | Right |
|---|---|
| Direct | Order |
| Direct | BinaryValue |
| Direct | BinaryOffset |
| Direct | BinaryRank |
| Order | BinaryValue |
| Order | BinaryOffset |
| Order | BinaryRank |

Test:

- left-to-right consistency;
- right-to-left consistency;
- all valid values;
- invalid representation assignments;
- sparse and contiguous domains where applicable.

---

## 17.4 Multi-use tests

These directly target the central requirement.

### Test A: direct and binary

```text
find x : int(0..7)
such that
    allDiff([x, y])
    x + z = 5
```

Expected:

```text
x in allDiff     → Direct
x in arithmetic  → BinaryValue
Direct ↔ Binary channeling created
```

### Test B: direct, order, and binary

Use `x` in:

- `allDifferent`;
- an inequality;
- an arithmetic sum.

Expected:

```text
x:
    Direct
    Order
    BinaryValue
```

with all required channeling.

### Test C: repeated use in one constraint

```text
2*x + x ≤ 9
```

Distinct references may select different representations, though the initial heuristic may choose one shared representation.

The system must remain semantically correct either way.

### Test D: shared representation reuse

Two different Direct-requiring references to `x` must reuse one Direct representation.

### Test E: no unnecessary channeling

If all references to `x` use Order, only one Order representation should exist and no channeling should be emitted.

---

## 17.5 Mutation and negative tests

Deliberately introduce errors to verify the test suite detects them:

- remove one direction of direct/order channeling;
- reverse a binary bit;
- omit a binary range restriction;
- allocate duplicate Direct representations;
- map two semantic variables to one representation;
- decode using rank bits as numeric bits.

These tests validate the strength of the testing approach, not just the implementation.

---

# 18. Phase 15: observability and diagnostics

Add a representation and encoding report.

Example:

```text
Semantic variable x : int(0..31)

References:
  r12  allDifferent member     -> Direct
  r18  linear inequality       -> Order
  r27  objective               -> BinaryValue

Materialised representations:
  Direct       32 literals
  Order        31 literals
  BinaryValue   5 literals

Channeling:
  Direct <-> Order         93 clauses
  Direct <-> BinaryValue  192 clauses
  Order  <-> BinaryValue  141 clauses

Constraint encodings:
  c4 allDifferent  -> value-wise AMO / product
  c7 linear <=     -> order-tree
  c9 objective     -> binary weighted sum
```

Provide machine-readable JSON as well as human-readable text.

Statistics should distinguish:

- representation clauses;
- channeling clauses;
- constraint clauses;
- auxiliary variables by source;
- wrapped versus native generators.

---

# 19. Recommended commit sequence

Each item should be a reviewable commit or small pull request.

1. Add baseline test corpus and deterministic test mode.
2. Introduce `SemanticVarId` and semantic variable arena.
3. Migrate AST variable identity to `SemanticVarId`.
4. Introduce `VarRefId` and reference arena.
5. Preserve reference identity through rewrites.
6. Add encoding plan IR with no behavioural change.
7. Add plan diagnostics and golden tests.
8. Introduce representation registry.
9. Implement Boolean representation.
10. Implement Direct representation with pairwise AMO.
11. Implement Order representation.
12. Implement BinaryValue for actual numeric values and BinaryOffset for interval domains.
13. Implement BinaryRank for explicit sparse domains.
14. Add channeling registry and duplicate suppression.
15. Implement Direct ↔ Order.
16. Implement Direct ↔ BinaryValue.
17. Implement Direct ↔ BinaryRank.
18. Implement Order ↔ BinaryValue.
19. Implement Order ↔ BinaryRank.
20. Add semantic decoding and cross-representation validation.
21. Migrate equality/disequality to representation-aware plans.
22. Migrate comparisons.
23. Implement `allDifferent` alternatives.
24. Implement cardinality plan interface.
25. Add native baseline cardinality encoder.
26. Add RustSAT cardinality adapter.
27. Implement PB plan interface and normalisation.
28. Add native baseline PB encoder.
29. Add RustSAT PB adapters.
30. Add Pindakaas adapters where useful.
31. Implement linear integer plan alternatives.
32. Add order-tree encoding.
33. Add direct-to-PB encoding.
34. Add binary-adder encoding.
35. Add mixed-representation linear tests.
36. Add candidate generation and compatibility checks.
37. Add marginal representation/channeling cost model.
38. Add configurable heuristic selection.
39. Add Savile Row differential testing.
40. Add complete diagnostics and statistics reporting.

Every commit must preserve a passing test suite.

---

# 20. Definition of done for each feature

An encoding or representation is not complete until it has:

1. a documented semantic definition;
2. an explicit plan variant;
3. capability metadata;
4. deterministic clause generation in test mode;
5. unit tests;
6. exhaustive tests over small domains;
7. reification tests where supported;
8. decoding tests;
9. statistics reporting;
10. interaction tests with at least one other representation;
11. negative tests demonstrating that broken channeling is detected;
12. integration into at least one end-to-end E′ model.

---

# 21. Initial configuration

Use a conservative initial default configuration:

```rust
EncodingConfig {
    default_integer_representation: RepresentationKind::Order,
    all_different_encoding: AllDifferentEncoding::ValueWiseAmo,
    amo_encoding: AmoEncoding::Pairwise,
    cardinality_encoding: CardinalityEncoding::Totaliser,
    pb_encoding: PbEncoding::GeneralisedTotaliser,
    linear_integer_encoding: LinearIntegerEncoding::OrderTree,
    channeling_topology: ChannelingTopology::PairwiseExisting,
    validate_cross_representation_models: cfg!(debug_assertions),
    deterministic: false,
}
```

Tests should commonly enable:

```rust
deterministic: true
validate_cross_representation_models: true
```

---

# 22. Key implementation rules

The coding agent should follow these constraints throughout.

1. Never identify semantic variables by name.
2. Never create one representation per occurrence.
3. Never let a clause generator choose a representation implicitly.
4. Never let an external library own semantic variable identities.
5. Never emit the same channeling pair twice.
6. Never create a representation merely because another representation exists, unless an explicit channeling or encoding plan requires it.
7. Never assume two references to the same variable use the same representation.
8. Never assume two operands of a primitive constraint use the same representation.
9. Never decode a semantic variable independently from multiple representations without checking agreement.
10. Never add a new encoding alternative without exhaustive small-instance tests.
11. Never expose external-library literal or variable types outside the adapter layer.
12. Never treat `BinaryValue`, `BinaryOffset`, and `BinaryRank` as interchangeable.

---

# 23. First practical milestone

The first useful end-to-end milestone should support:

- stable semantic variable identities;
- occurrence-level representation requests;
- shared Direct, Order, BinaryValue, and BinaryOffset representations;
- automatic pairwise channeling;
- equality, disequality, and comparison;
- value-wise `allDifferent`;
- simple linear constraints using Order or BinaryValue;
- complete decoding;
- exhaustive multi-representation tests.

A demonstration model should intentionally use one variable in three ways:

```text
x in allDifferent  -> Direct
x in x <= y        -> Order
x in x + z = 10    -> BinaryValue
```

The diagnostic output should show three shared representations for `x`, the selected encoding for each use, and the channeling constraints added between them.

This milestone validates the architecture before adding the larger portfolio of AMO, cardinality, PB, table, and linear encodings.
# SAT Adaptor

The **SAT Adaptor**:
- **Lowers IR to CNF:** Converts boolean and integer constraints (via SAT encodings) into Conjunctive Normal Form.
- **Interfaces with RustSAT:** Utilizes the `rustsat` library to communicate with SAT solvers like CaDiCaL.
- **Maps Literals:** Maintains the mapping between high-level variable names and low-level SAT literals to decode solutions back into Essence values.

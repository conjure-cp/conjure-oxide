# SAT Adaptor

An **adaptor** in `conjure-oxide` acts as a translation layer between the core intermediate
 representation and a specific solver's API. 

The **SAT Adaptor** specifically:
- **Lowers IR to CNF:** Converts boolean and integer constraints (via SAT encodings) into Conjunctive Normal Form.
- **Interfaces with RustSAT:** Utilizes the `rustsat` library to communicate with SAT solvers like CaDiCaL.
- **Maps Literals:** Maintains the mapping between high-level variable names and low-level SAT literals to decode solutions back into Essence values.

## Priorities

Rule are applied in priority order (highest number first).

Rule priority levels are currently the following:

| Priority level | Usage                                                                                     |
| ---            | -----                                                                                     | 
| **9001**   | **Total evaluation**                                                                          |
| **9000**   | **Partial evaluation**                                                                        |
|   8800     | Trivial simplifications: removing nesting, removing empty / unit constraints                  | 
|   8400     | Transformation into canonical forms: (distributivity, associativity, commutativity, etc.)     |
| **8000**   | **Simplifications**                                                                           |
| **6000**   | **Modelling enhancing reformulations**                                                        |
| **4000**   | **Solver Specific**                                                                           |
| **2000**   | **Non solver specific, non enhancing reformulations** (e.g. restate a constraint using simpler constraints when needed for solver compatability)    |

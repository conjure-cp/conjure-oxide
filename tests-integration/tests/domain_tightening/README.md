# Domain Tightening Tests
These tests are designed to check whether or not the pruning stage reduces the domains of expressions without making them invalid.

They will work by running the same underlying functions used in the `pretty <> --output-format=expression-domains` command.

The general steps of the algorithm (TBC) will likely involve:
1. Parse essence file
2. Get the domains of the expressions

We can then add subsequent steps that will run some ac-style algorithm and recheck the domains.

## How to Run
cargo test tests_domain_tightening
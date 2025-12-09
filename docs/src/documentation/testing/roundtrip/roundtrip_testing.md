[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 09/12/2025)

# Roundtrip Testing
## Overview

Roundtrip tests check that parsing is valid and that there is no unexpected behaviour during a full roundtrip of an Essence file.
Roundtrip tests do not consider rewriting or solving.

## Full Test Structure
The structure of a roundtrip test is shown in the following diagram:


<img src="./roundtripTest.png" width="500">


The first phase tests that the parser is still performing as expected. As with the other tests in the suite, if the expected should genuinely have changed the suite can be run with `ACCEPT=true` to overwrite these expected files with whatever is generated.
- The test parses a provided Essence or Essence' file.
- If this parse is valid, it saves the generated AST model JSON and generated Essence representation and compare these to the expected versions.
- Otherwise, is the parse fails; it saves the generated error outputs and compares this to the expected version.


Now the second phase tests that the structure of the input does not change during parsing without applying any rules to it, ensuring validity.

- Now if the initial parse was successful the roundtrip phase of the test occurs. 
- The newly generated Essence is then parsed back into Conjure-Oxide and used to generate a new AST and output Essence file
- This new Essence file is then compared with the previously generated one and asserted equal.

## Multiple Parsers
Roundtrip tests support both the 'legacy' and 'native' parser.
The parser used for the test can be specified in a config.TOML in the test directory using the form `parsers = [<string list of parsers>]`
Both parsers will be used by default for each test unless specified and will use separate generated and expected files as this may be different per parser.

## Creating a New Roundtrip Test
To create a new roundtrip test you must create a new directory under `./tests-integration/tests/roundtrip/`.
Any directories within here than contain a singular `.essence` or `.eprime` file will be treated as a test.
After creating the Essence or Essence' file for your test you should run the test suite with `ACCEPT=true` to generate the expected files.
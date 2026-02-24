[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 09/12/2025)

# Creating Types Guide
Currently Conjure-Oxide does not have any support for Essence's sequences, relations, partitions or variants.
This guide outlines the process of creating support for those types to the AST. This does not currently cover rewriting rules or solving.
## Creating the Type
- The domains of these types can contain pointers and so must be able to be both ground and unresolved. Hence you must add to the `GroundDomain` and `UnresolvedDomain` Enums
- Adding a new domain will require adding implementations for it, for a lot of related functions. To use the roundtrip tests the Display function `fmt()` must be implemented to match the Essence of the type.
- Next to complete some of these implementation you will need to add to the `ReturnType` Enum. This is the returned value from a specific element of the abstract type.
- Finally these functions will also require you to add to the `AbstractLiteral` Enum. This defines the values of a literal of that type. This will be required for defining the type explicitly. You should also note that we implement `Uniplate` for `AbstractLiteral`, and so this implementation will be required.
## Adding to the Legacy Parser
Once the type can be stored in Conjure-Oxide's AST, you will need to add parsing support.
- For the legacy parser this involves adding to `parse_model.rs` to parse the JSON output of Conjure's parser.
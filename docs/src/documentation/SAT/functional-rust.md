## Functional Rust

Consider the following soft-quoted statement about Rust:

*Rust is blazingly fast and memory-efficient: with no runtime or garbage collector. Rust's rich type system and ownership model guarantee memory-safety and thread-safety — enabling you to eliminate many classes of bugs at compile-time.*

Taken specifically from the Rust Foundation’s[^1] page talking about why Rust is a good language to use.

But let us focus on the implications of this statement on Conjure Oxide specifically. The key details that one needs to know here are that, despite arguably being imperative, Rust adopts many functional programming concepts in its design[^2]. This is significant because the Conjure Oxide codebase makes extensive use of these functional programming concepts.

Why do this? Making use of them allows the codebase to leverage Rust’s type system to eliminate failure cases through coding style, making the code more robust. It also makes the code easier to write, because Rust’s safety system enforces function return types. By using the `Result` type, functions can explicitly state success or failure. This also allows functions to call each other linearly in a safe manner[^3].

This enables the code to be written so that functions can do essentially whatever they need to do, as long as they record a success or failure result. The error type allows error propagation in a more sophisticated way than exit codes, while still being more efficient than exceptions.

---

[^1]: Who are, of course, an entirely unbiased source  
[^2]: Rust’s original implementation language was OCaml, which is functional  
[^3]: Similar to calling functions in dynamically typed languages like Python, but with enforced types

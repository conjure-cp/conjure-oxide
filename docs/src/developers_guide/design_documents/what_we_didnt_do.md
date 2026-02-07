# What we didn't do

The intention of this page is to chronicle decisions made during the development of conjure-oxide. There have been many situations where we have thought of something interesting that ends up not being the best solution to a problem and we believe others may come up with the same ideas. This page should be read before contributing so that potential contributors do not go down rabbit holes that have already been thoroughly searched.
<details>
<summary><h2>Nested expressions within the polymorphic metadata field </h2></summary>

### Background
As discussed in [Issue 182](https://github.com/conjure-cp/conjure-oxide/issues/182), we wanted to create a polymorphic metadata field that would be contained within the expression struct and that could be changed on a case-by-case basis as metadata might only be needed for a single module for example. 

### What we thought of
One interesting idea that was suggested as a structure like this:

```rust
// Define a trait for metadata in each module.
pub trait Metadata {
    // Define methods specific to the metadata.
    fn print_metadata(&self);
    // Add other methods as needed.
}

// Module-specific metadata types.
pub mod module1 {
    pub struct Metadata1 {
        // Define fields specific to this metadata.
        pub clean: bool,
        // Add other fields as needed.
    }

    impl super::Metadata for Metadata1 {
        fn print_metadata(&self) {
            println!("Metadata1: Clean - {}", self.clean);
        }
    }
}

pub mod module2 {
    pub struct Metadata2 {
        // Define fields specific to this metadata.
        pub status: String,
        // Add other fields as needed.
    }

    impl super::Metadata for Metadata2 {
        fn print_metadata(&self) {
            println!("Metadata2: Status - {}", self.status);
        }
    }
}

// Modify the Expression enum to hold a trait object for metadata.
#[derive(Clone, Debug)]
pub enum Expression {
    // Existing enum variants here...
    WithMetadata(Box<Expression>, Option<Box<dyn Metadata>>),
}

impl Expression {
    // Create a new expression with metadata.
    pub fn with_metadata(expr: Expression, metadata: Option<Box<dyn Metadata>>) -> Expression {
        Expression::WithMetadata(Box::new(expr), metadata)
    }

    // Extract the expression and metadata.
    pub fn extract_metadata(self) -> (Expression, Option<Box<dyn Metadata>>) {
        match self {
            Expression::WithMetadata(expr, metadata) => (*expr, metadata),
            _ => (self, None),
        }
    }

    // Set metadata for an expression.
    pub fn set_metadata(&mut self, metadata: Option<Box<dyn Metadata>>) {
        if let Expression::WithMetadata(_, ref mut existing_metadata) = *self {
            *existing_metadata = metadata;
        }
    }

    // Get metadata for an expression.
    pub fn get_metadata(&self) -> Option<&dyn Metadata> {
        if let Expression::WithMetadata(_, Some(metadata)) = self {
            Some(metadata.as_ref())
        } else {
            None
        }
    }

    // Existing methods here...
}

```

### Why we didn't do it
This is nice in the sense that there is less "ugliness" when creating expressions as there is no need to have metadata in every enum variant. However, this severely effects the way that the rewriter traverses the AST as there is now WithMetadata objects sprinkled throughout the AST which would require some way to traverse up the tree while saving the context of nodes below the metadata object. This image shows the differences in a basic AST:

<img src="https://github.com/conjure-cp/conjure-oxide/assets/27870413/09726bf0-c2ef-4056-b7c3-e011f148774b" width="300" height="600">

### What we did do
Due to this issue we decided that a simpler implementation where metadata is explicitly required every time but could be set to some empty metadata object was more practical. More details on this implementation can found in [Near Future PR](https://github.com/conjure-cp/conjure-oxide/pull/233)

</details>

---

*This section had been adapted from the 'What we didn't do' page of the conjure-oxide wiki*
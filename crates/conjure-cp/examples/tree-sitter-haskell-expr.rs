use tree_sitter::Parser;
use tree_sitter_haskell::LANGUAGE;

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Expr {
    Var(String),
    Int(i32),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

// Helper function to iterate over named children
fn named_children<'a>(
    node: &'a tree_sitter::Node<'a>,
) -> impl Iterator<Item = tree_sitter::Node<'a>> + 'a {
    (0..node.named_child_count()).filter_map(|i| node.named_child(i))
}

// Function to parse and convert Tree-sitter nodes to Expr
fn parse_expr(node: tree_sitter::Node, source_code: &str) -> Option<Expr> {
    // println!("EXPR {:?}", node.to_sexp());
    match node.kind() {
        "literal" => {
            let value_text = node.utf8_text(source_code.as_bytes()).ok()?.trim();
            let value = value_text.parse().ok()?;
            Some(Expr::Int(value))
        }
        "variable" => {
            let name = node.utf8_text(source_code.as_bytes()).ok()?.to_string();
            Some(Expr::Var(name))
        }
        "parens" => {
            let inner = node.named_child(0)?;
            parse_expr(inner, source_code)
        }
        "infix" => {
            let left = parse_expr(node.child_by_field_name("left_operand")?, source_code)?;
            let op_node = node.child_by_field_name("operator")?;
            let op = op_node.utf8_text(source_code.as_bytes()).ok()?;
            let right = parse_expr(node.child_by_field_name("right_operand")?, source_code)?;

            match op.trim() {
                "+" => Some(Expr::Add(Box::new(left), Box::new(right))),
                "*" => Some(Expr::Mul(Box::new(left), Box::new(right))),
                _ => None,
            }
        }
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_code = "example1 = (x + 2) * (y + 3)";
    // let source_code = "example2 = (x + 2 + 3) * (2 * y + 3)";

    let mut parser = Parser::new();
    let _ = parser.set_language(&LANGUAGE.into());

    let tree = parser
        .parse(source_code, None)
        .ok_or("Error parsing code")?;

    // println!("{:?}", tree);

    let root_node = tree.root_node();
    // println!("{:?}", root_node.to_sexp());

    for child in named_children(&root_node) {
        if child.kind() == "declarations" {
            for decl in named_children(&child) {
                if decl.kind() == "bind" {
                    // Get the "name" field
                    let name_node = decl
                        .child_by_field_name("name")
                        .ok_or("Missing name in bind")?;
                    let name_text = name_node.utf8_text(source_code.as_bytes())?;

                    // Get the "match" field
                    let match_node = decl
                        .child_by_field_name("match")
                        .ok_or("Missing match in bind")?;

                    // Within "match", get the "expression" field
                    let expr_node = match_node
                        .child_by_field_name("expression")
                        .ok_or("Missing expression in match")?;

                    // println!("1111 {:?}", expr_node.to_sexp());
                    // println!("2222 {:?}", source_code);

                    // Parse expr_node to our Expr type
                    let expr = parse_expr(expr_node, source_code);
                    println!("{name_text} is parsed as {expr:?}");
                }
            }
        }
    }

    Ok(())
}

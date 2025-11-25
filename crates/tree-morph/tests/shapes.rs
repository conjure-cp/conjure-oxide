// Example case from the 05/02/2025 Conjure VIP meeting

use tree_morph::{helpers::select_panic, prelude::*};
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Shape {
    Circle(Box<Shape>),
    Square,
    Triangle,
}

// O(/\) ~> /\
fn circ_tri_to_tri(_: &mut Commands<Shape, ()>, expr: &Shape, _: &()) -> Option<Shape> {
    if let Shape::Circle(inner) = expr
        && let Shape::Triangle = **inner
    {
        return Some(Shape::Triangle);
    }
    None
}

// O(O(/\)) ~> []
fn circ_circ_tri_to_sqr(_: &mut Commands<Shape, ()>, expr: &Shape, _: &()) -> Option<Shape> {
    if let Shape::Circle(inner) = expr
        && let Shape::Circle(inner_inner) = inner.as_ref()
        && let Shape::Triangle = **inner_inner
    {
        return Some(Shape::Square);
    }
    None
}

#[test]
fn circ_tri() {
    // O(/\)
    let expr = Shape::Circle(Box::new(Shape::Triangle));

    let engine = EngineBuilder::new()
        .add_rule(circ_tri_to_tri as RuleFn<_, _>)
        .add_rule(circ_circ_tri_to_sqr as RuleFn<_, _>)
        .build();
    let (result, _) = engine.morph(expr, ());

    assert_eq!(result, Shape::Triangle);
}

#[test]
fn circ_circ_tri() {
    // O(O(/\))
    let expr = Shape::Circle(Box::new(Shape::Circle(Box::new(Shape::Triangle))));

    // Same priority group - 2nd rule applies first as it applies higher in the tree
    let engine = EngineBuilder::new()
        .add_rule_group(rule_fns![circ_tri_to_tri, circ_circ_tri_to_sqr])
        .build();
    let (result, _) = engine.morph(expr, ());

    assert_eq!(result, Shape::Square);
}

#[test]
fn shape_higher_priority() {
    // O(O(/\))
    let expr = Shape::Circle(Box::new(Shape::Circle(Box::new(Shape::Triangle))));

    // Higher priority group - 1st rule applies first even though it applies lower in the tree
    let engine = EngineBuilder::new()
        .add_rule(circ_tri_to_tri as RuleFn<_, _>)
        .add_rule(circ_circ_tri_to_sqr as RuleFn<_, _>)
        .build();
    let (result, _) = engine.morph(expr, ());

    // O(O(/\)) ~> O(/\) ~> /\
    assert_eq!(result, Shape::Triangle);
}

#[should_panic]
#[test]
fn shape_multiple_rules_panic() {
    // O(O(/\))
    let expr = Shape::Circle(Box::new(Shape::Circle(Box::new(Shape::Triangle))));

    // Same rule twice, applicable at the same time
    let engine = EngineBuilder::new()
        .set_selector(select_panic)
        .add_rule_group(rule_fns![circ_tri_to_tri, circ_tri_to_tri])
        .build();
    engine.morph(expr, ());
}

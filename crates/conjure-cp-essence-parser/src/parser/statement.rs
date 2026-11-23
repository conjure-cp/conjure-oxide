use std::collections::BTreeMap;

use conjure_cp_core::ast::{DeclarationPtr, DomainPtr, Expression, Model, Name};
use tree_sitter::Node;

use crate::{
    FatalParseError, ParseContext,
    diagnostics::diagnostics_api::SymbolKind,
    domain::parse_domain,
    expression::parse_expression,
    field,
    parser::{parse_ident, parse_name},
    util::named_children,
};

#[derive(Debug, Clone)]
pub enum Statement {
    Language(String),
    Letting(Vec<LettingDeclaration>),
    Given(Vec<VariableDeclaration>),
    Where(Vec<Expression>),
    SuchThat(Vec<Expression>),
    Find(Vec<VariableDeclaration>),
    Objective(),
    Dominance(),
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub idents: Vec<Name>,
    pub domain: VariableDeclarationDomain,
}

#[derive(Debug, Clone)]
pub enum VariableDeclarationDomain {
    Domain(DomainPtr),
    Enum,
}

#[derive(Debug, Clone)]
pub struct LettingDeclaration {
    pub idents: Vec<Name>,
    pub value: LettingDeclarationValue,
}

#[derive(Debug, Clone)]
pub enum LettingDeclarationValue {
    Expr(Expression),
    Domain(DomainPtr),
    Enum { variants: Vec<Name> },
    Unnamed { size: Expression },
}

pub fn parse_program(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Vec<Statement>, FatalParseError> {
    node.named_children(&mut node.walk())
        .map(|n| parse_statement(ctx, &n))
        .collect()
}

pub fn parse_statement(ctx: &mut ParseContext, node: &Node) -> Result<Statement, FatalParseError> {
    let stmt = match node.kind() {
        "language_statement" => {
            let lang = parse_ident(ctx, &field!(node, "language"));
            Statement::Language(lang)
        }
        "find_statement" => {
            let decls = node
                .named_children(&mut node.walk())
                .map(|n| parse_variable_declaration(ctx, &n))
                .collect::<Result<_, FatalParseError>>()?;

            Statement::Find(decls)
        }
        "given_statement" => {
            let decls = node
                .named_children(&mut node.walk())
                .map(|n| parse_variable_declaration(ctx, &n))
                .collect::<Result<_, FatalParseError>>()?;

            Statement::Given(decls)
        }
        "such_that_statement" => {
            let constraints = node
                .named_children(&mut node.walk())
                .map(|n| parse_expression(ctx, n))
                .collect::<Result<_, FatalParseError>>()?;
            Statement::SuchThat(constraints)
        }
        "where_statement" => {
            let constraints = node
                .named_children(&mut node.walk())
                .map(|n| parse_expression(ctx, n))
                .collect::<Result<_, FatalParseError>>()?;
            Statement::Where(constraints)
        }
        "letting_statement" => {
            let decls = node
                .named_children(&mut node.walk())
                .map(|n| parse_letting_declaration(ctx, &n))
                .collect::<Result<_, FatalParseError>>()?;
            Statement::Letting(decls)
        }
        "objective_statement" => {
            let dir = match parse_ident(ctx, &field!(node, "direction")).as_ref() {
                "minimising" => todo!(),
                "maximising" => todo!(),
                _ => unreachable!(),
            };
            todo!()
        }
        "dominance_relation" => {
            todo!()
        }
        _ => unreachable!(),
    };
    Ok(stmt)
}

fn parse_variable_declaration(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<VariableDeclaration, FatalParseError> {
    let idents = node
        .children_by_field_name("variables", &mut node.walk())
        .map(|v| parse_name(ctx, &v))
        .collect::<Result<_, FatalParseError>>()?;

    let domain = if let Some(domain_node) = node.child_by_field_name("domain") {
        VariableDeclarationDomain::Domain(parse_domain(ctx, domain_node)?)
    } else if node.child_by_field_name("enum").is_some() {
        VariableDeclarationDomain::Enum
    } else {
        unreachable!()
    };

    Ok(VariableDeclaration { idents, domain })
}

fn parse_letting_declaration(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<LettingDeclaration, FatalParseError> {
    let idents = node
        .children_by_field_name("variables", &mut node.walk())
        .map(|v| parse_name(ctx, &v))
        .collect::<Result<_, FatalParseError>>()?;

    let value = if let Some(expr_node) = node.child_by_field_name("expression") {
        LettingDeclarationValue::Expr(parse_expression(ctx, expr_node)?)
    } else if let Some(domain_node) = node.child_by_field_name("domain") {
        LettingDeclarationValue::Domain(parse_domain(ctx, domain_node)?)
    } else if let Some(variants_node) = node.child_by_field_name("enum") {
        let variants = variants_node
            .named_children(&mut variants_node.walk())
            .map(|v| parse_name(ctx, &v))
            .collect::<Result<_, _>>()?;
        LettingDeclarationValue::Enum { variants }
    } else if let Some(size_node) = node.child_by_field_name("unnamed") {
        LettingDeclarationValue::Unnamed {
            size: parse_expression(ctx, size_node)?,
        }
    } else {
        unreachable!()
    };

    Ok(LettingDeclaration { idents, value })
}

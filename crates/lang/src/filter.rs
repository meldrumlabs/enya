//! Filter query parsing and AST representation.

use crate::error::Error;
use crate::lexer::{self, tokenize_filter_query};
use std::collections::VecDeque;

/// A tag key-value pair.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Tag<'a> {
    /// The tag key.
    pub key: &'a str,
    /// The tag value.
    pub value: &'a str,
}

/// AST node for filter queries.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Node<'a> {
    /// AND of multiple conditions.
    And(Vec<Self>),
    /// OR of multiple conditions.
    Or(Vec<Self>),
    /// Exact tag match.
    Eq(Tag<'a>),
    /// Wildcard (prefix) match.
    Wildcard(Tag<'a>),
    /// NOT condition.
    Not(Box<Self>),
    /// Match all series for the metric.
    AllStar,
}

impl std::fmt::Display for Node<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Eq(leaf) => write!(f, "{}:{}", leaf.key, leaf.value),
            Node::Wildcard(leaf) => write!(f, "{}:{}*", leaf.key, leaf.value),
            Node::And(nodes) => write!(
                f,
                "({})",
                nodes
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" AND ")
            ),
            Node::Or(nodes) => write!(
                f,
                "({})",
                nodes
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" OR ")
            ),
            Node::AllStar => write!(f, "*"),
            Node::Not(node) => write!(f, "!({node})"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum Item<'a> {
    Wildcard((&'a str, &'a str)),
    Identifier((&'a str, &'a str)),
    And,
    Or,
    Not,
    ParenOpen,
    ParenClose,
}

fn split_identifier(value: &str) -> Result<(&str, &str), Error> {
    value.split_once(':').ok_or(Error::InvalidQuery)
}

fn push_identifier<'a>(output_queue: &mut VecDeque<Item<'a>>, id: &'a str) -> Result<(), Error> {
    let (key, value) = split_identifier(id)?;
    output_queue.push_back(Item::Identifier((key, value)));
    Ok(())
}

fn push_wildcard<'a>(output_queue: &mut VecDeque<Item<'a>>, id: &'a str) -> Result<(), Error> {
    let (key, value) = split_identifier(id)?;
    output_queue.push_back(Item::Wildcard((key, value.trim_end_matches('*'))));
    Ok(())
}

fn drain_ops<'a, F>(
    output_queue: &mut VecDeque<Item<'a>>,
    op_stack: &mut VecDeque<Item<'a>>,
    predicate: F,
) -> crate::Result<()>
where
    F: Fn(&Item<'a>) -> bool,
{
    loop {
        let should_pop = matches!(op_stack.back(), Some(top_op) if predicate(top_op));
        if !should_pop {
            break;
        }
        let op = op_stack.pop_back().ok_or(Error::InvalidQuery)?;
        output_queue.push_back(op);
    }
    Ok(())
}

fn handle_paren_close<'a>(
    output_queue: &mut VecDeque<Item<'a>>,
    op_stack: &mut VecDeque<Item<'a>>,
) -> crate::Result<()> {
    loop {
        let Some(top_op) = op_stack.back() else {
            return Err(Error::InvalidQuery);
        };

        if matches!(top_op, Item::ParenOpen) {
            break;
        }

        let op = op_stack.pop_back().ok_or(Error::InvalidQuery)?;
        output_queue.push_back(op);
    }

    let open = op_stack.pop_back().ok_or(Error::InvalidQuery)?;
    if !matches!(open, Item::ParenOpen) {
        return Err(Error::InvalidQuery);
    }

    Ok(())
}

fn pop_node<'a>(buf: &mut Vec<Node<'a>>) -> crate::Result<Node<'a>> {
    buf.pop().ok_or(Error::InvalidQuery)
}

fn build_ast<'a>(output_queue: VecDeque<Item<'a>>) -> crate::Result<Node<'a>> {
    let mut buf: Vec<Node<'a>> = Vec::new();

    for item in output_queue {
        match item {
            Item::Identifier((key, value)) => {
                buf.push(Node::Eq(Tag { key, value }));
            }
            Item::Wildcard((key, value)) => {
                buf.push(Node::Wildcard(Tag { key, value }));
            }
            Item::And => {
                let right = pop_node(&mut buf)?;
                let left = pop_node(&mut buf)?;
                buf.push(Node::And(vec![left, right]));
            }
            Item::Or => {
                let right = pop_node(&mut buf)?;
                let left = pop_node(&mut buf)?;
                buf.push(Node::Or(vec![left, right]));
            }
            Item::Not => {
                let node = pop_node(&mut buf)?;
                buf.push(Node::Not(Box::new(node)));
            }
            Item::ParenOpen | Item::ParenClose => return Err(Error::InvalidQuery),
        }
    }

    match buf.len() {
        1 => pop_node(&mut buf),
        _ => Err(Error::InvalidQuery),
    }
}

/// Parse a filter query expression into an AST.
///
/// # Errors
///
/// Returns an error if the query syntax is invalid.
pub fn parse_filter_query(s: &str) -> Result<Node, Error> {
    if s.trim() == "*" {
        return Ok(Node::AllStar);
    }

    let mut output_queue = VecDeque::new();
    let mut op_stack = VecDeque::new();

    for tok in tokenize_filter_query(s) {
        let tok = tok.map_err(|()| Error::InvalidQuery)?;

        match tok {
            lexer::Token::Identifier(id) => push_identifier(&mut output_queue, id)?,
            lexer::Token::Wildcard(id) => push_wildcard(&mut output_queue, id)?,
            lexer::Token::And => {
                drain_ops(&mut output_queue, &mut op_stack, |item| {
                    matches!(item, Item::And | Item::Not)
                })?;
                op_stack.push_back(Item::And);
            }
            lexer::Token::Or => {
                drain_ops(&mut output_queue, &mut op_stack, |item| {
                    matches!(item, Item::And | Item::Not)
                })?;
                op_stack.push_back(Item::Or);
            }
            lexer::Token::Not => {
                op_stack.push_back(Item::Not);
            }
            lexer::Token::ParenOpen => {
                op_stack.push_back(Item::ParenOpen);
            }
            lexer::Token::ParenClose => {
                handle_paren_close(&mut output_queue, &mut op_stack)?;
            }
        }
    }

    while let Some(top_op) = op_stack.pop_back() {
        if matches!(top_op, Item::ParenOpen) {
            return Err(Error::InvalidQuery);
        }
        output_queue.push_back(top_op);
    }

    build_ast(output_queue)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filter_query_1() {
        assert_eq!(
            Node::Eq(Tag {
                key: "hello",
                value: "world"
            }),
            parse_filter_query("hello:world").unwrap()
        );
    }

    #[test]
    fn test_parse_filter_query_2() {
        assert_eq!(
            Node::Not(Box::new(Node::Eq(Tag {
                key: "hello",
                value: "world"
            }))),
            parse_filter_query("!hello:world").unwrap()
        );
    }

    #[test]
    fn test_parse_filter_query_3() {
        assert_eq!(
            Node::Not(Box::new(Node::Or(vec![
                Node::Eq(Tag {
                    key: "hello",
                    value: "world"
                }),
                Node::Eq(Tag {
                    key: "hallo",
                    value: "welt"
                }),
            ]))),
            parse_filter_query("!(hello:world OR hallo:welt)").unwrap()
        );
    }

    #[test]
    fn test_parse_filter_query_wildcard() {
        assert_eq!(
            Node::Wildcard(Tag {
                key: "service",
                value: "db-"
            }),
            parse_filter_query("service:db-*").unwrap()
        );
    }
}

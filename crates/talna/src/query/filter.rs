use crate::query::lexer::{self, tokenize_filter_query};
use crate::smap::SeriesMapping;
use crate::{tag_index::TagIndex, SeriesId};
use std::collections::VecDeque;

#[derive(Debug, Eq, PartialEq)]
pub struct Tag<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Node<'a> {
    And(Vec<Self>),
    Or(Vec<Self>),
    Eq(Tag<'a>),
    Wildcard(Tag<'a>),
    Not(Box<Self>),
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
            Node::Not(node) => write!(f, "!({node})",),
        }
    }
}

pub fn intersection(vecs: &[Vec<SeriesId>]) -> Vec<SeriesId> {
    if vecs.is_empty() || vecs.iter().any(Vec::is_empty) {
        return vec![];
    }

    let Some(first_vec) = vecs.first() else {
        return vec![];
    };
    let mut result = Vec::new();

    'outer: for &elem in first_vec {
        if vecs.iter().skip(1).any(|vec| !vec.contains(&elem)) {
            continue 'outer;
        }

        result.push(elem);
    }

    result
}

#[must_use]
pub fn union(vecs: &[Vec<SeriesId>]) -> Vec<SeriesId> {
    let mut result = vec![];

    for vec in vecs {
        result.extend(vec);
    }

    result.sort_unstable();
    result.dedup();

    result
}

impl Node<'_> {
    // TODO: 1.0.0 unit test and add benchmark case
    pub fn evaluate(
        &self,
        smap: &SeriesMapping,
        tag_index: &TagIndex,
        metric_name: &str,
    ) -> crate::Result<Vec<SeriesId>> {
        match self {
            Node::AllStar => tag_index.query_eq(metric_name),
            Node::Eq(leaf) => {
                tag_index.query_eq(&TagIndex::format_key(metric_name, leaf.key, leaf.value))
            }
            Node::Wildcard(leaf) => {
                tag_index.query_prefix(&TagIndex::format_key(metric_name, leaf.key, leaf.value))
            }
            Node::And(children) => {
                // TODO: evaluate lazily...
                let ids = children
                    .iter()
                    .map(|c| Self::evaluate(c, smap, tag_index, metric_name))
                    .collect::<crate::Result<Vec<_>>>()?;

                Ok(intersection(&ids))
            }
            Node::Or(children) => {
                // TODO: evaluate lazily...
                let ids = children
                    .iter()
                    .map(|c| Self::evaluate(c, smap, tag_index, metric_name))
                    .collect::<crate::Result<Vec<_>>>()?;

                Ok(union(&ids))
            }
            Node::Not(node) => {
                let mut ids = smap.list_all()?;

                for id in node.evaluate(smap, tag_index, metric_name)? {
                    ids.remove(&id);
                }

                let mut ids = ids.into_iter().collect::<Vec<_>>();
                ids.sort_unstable();

                Ok(ids)
            }
        }
    }
}

#[derive(Debug)]
pub enum Item<'a> {
    Wildcard((&'a str, &'a str)),
    Identifier((&'a str, &'a str)),
    And,
    Or,
    Not,
    ParanOpen,
    ParanClose,
}

fn split_identifier(value: &str) -> Result<(&str, &str), crate::Error> {
    value.split_once(':').ok_or(crate::Error::InvalidQuery)
}

fn push_identifier<'a>(
    output_queue: &mut VecDeque<Item<'a>>,
    id: &'a str,
) -> Result<(), crate::Error> {
    let (key, value) = split_identifier(id)?;
    output_queue.push_back(Item::Identifier((key, value)));
    Ok(())
}

fn push_wildcard<'a>(
    output_queue: &mut VecDeque<Item<'a>>,
    id: &'a str,
) -> Result<(), crate::Error> {
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
        let op = op_stack.pop_back().ok_or(crate::Error::InvalidQuery)?;
        output_queue.push_back(op);
    }
    Ok(())
}

fn handle_paran_close<'a>(
    output_queue: &mut VecDeque<Item<'a>>,
    op_stack: &mut VecDeque<Item<'a>>,
) -> crate::Result<()> {
    loop {
        let Some(top_op) = op_stack.back() else {
            return Err(crate::Error::InvalidQuery);
        };

        if matches!(top_op, Item::ParanOpen) {
            break;
        }

        let op = op_stack.pop_back().ok_or(crate::Error::InvalidQuery)?;
        output_queue.push_back(op);
    }

    let open = op_stack.pop_back().ok_or(crate::Error::InvalidQuery)?;
    if !matches!(open, Item::ParanOpen) {
        return Err(crate::Error::InvalidQuery);
    }

    Ok(())
}

fn pop_node<'a>(buf: &mut Vec<Node<'a>>) -> crate::Result<Node<'a>> {
    buf.pop().ok_or(crate::Error::InvalidQuery)
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
            Item::ParanOpen | Item::ParanClose => return Err(crate::Error::InvalidQuery),
        }
    }

    match buf.len() {
        1 => pop_node(&mut buf),
        _ => Err(crate::Error::InvalidQuery),
    }
}

#[doc(hidden)]
pub fn parse_filter_query(s: &str) -> Result<Node, crate::Error> {
    if s.trim() == "*" {
        return Ok(Node::AllStar);
    }

    let mut output_queue = VecDeque::new();
    let mut op_stack = VecDeque::new();

    for tok in tokenize_filter_query(s) {
        let tok = tok.map_err(|()| crate::Error::InvalidQuery)?;

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
            lexer::Token::ParanOpen => {
                op_stack.push_back(Item::ParanOpen);
            }
            lexer::Token::ParanClose => {
                handle_paran_close(&mut output_queue, &mut op_stack)?;
            }
        }
    }

    while let Some(top_op) = op_stack.pop_back() {
        if matches!(top_op, Item::ParanOpen) {
            return Err(crate::Error::InvalidQuery);
        }
        output_queue.push_back(top_op);
    }

    build_ast(output_queue)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test_log::test]
    fn test_parse_filter_query_1() {
        assert_eq!(
            Node::Eq(Tag {
                key: "hello",
                value: "world"
            }),
            parse_filter_query("hello:world").unwrap()
        );
    }

    #[test_log::test]
    fn test_parse_filter_query_2() {
        assert_eq!(
            Node::Not(Box::new(Node::Eq(Tag {
                key: "hello",
                value: "world"
            }))),
            parse_filter_query("!hello:world").unwrap()
        );
    }

    #[test_log::test]
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

    #[test_log::test]
    fn test_parse_filter_query_wildcard_1() {
        assert_eq!(
            Node::Wildcard(Tag {
                key: "service",
                value: "db-"
            }),
            parse_filter_query("service:db-*").unwrap()
        );
    }

    #[test_log::test]
    fn test_intersection() {
        assert_eq!(
            [1, 3],
            *intersection(&[vec![1, 2, 3, 4, 5], vec![1, 3, 5], vec![1, 3]]),
        );
    }

    #[test_log::test]
    fn test_union() {
        assert_eq!(
            [1, 2, 4, 8],
            *union(&[vec![1, 8], vec![1, 2], vec![1, 2, 4], vec![2, 4, 8]]),
        );
    }
}

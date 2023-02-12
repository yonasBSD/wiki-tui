use std::collections::HashMap;

use anyhow::Result;
use cursive::theme::{Effect, Style};
use select::{document::Document, node::Node, predicate::Class};

use crate::config;

use super::article::{Element, ElementType};

const SHOW_UNSUPPORTED: bool = false;

pub struct Parser {
    elements: Vec<Element>,
    current_effects: Vec<Effect>,
}

impl Parser {
    pub fn parse_document<'a>(document: &'a str, title: &'a str) -> Result<Vec<Element>> {
        let document = Document::from(document);

        let mut parser = Parser {
            elements: Vec::new(),
            current_effects: Vec::new(),
        };

        parser.elements.push(Element::new(
            parser.next_id(),
            ElementType::Header,
            title.to_string(),
            config::CONFIG.theme.title,
            HashMap::new(),
        ));
        parser.push_newline();
        parser.push_newline();

        let _ = &document
            .find(Class("mw-parser-output"))
            .into_selection()
            .children()
            .into_iter()
            .map(|x| parser.parse_node(x))
            .count();

        Ok(parser.elements)
    }

    fn parse_node(&mut self, node: Node) {
        let name = node.name().unwrap_or_default();
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.parse_header(node),
            "p" => self.parse_paragraph(node),
            "a" => self.parse_link(node),
            "b" => self.parse_effect(node, Effect::Bold),
            "i" => self.parse_effect(node, Effect::Italic),
            "ul" => self.parse_list(node),
            "" => return,
            _ if SHOW_UNSUPPORTED => {
                self.elements.push(Element::new(
                    self.next_id(),
                    ElementType::Unsupported,
                    format!("<Unsupported Element '{}'>", name),
                    Effect::Italic,
                    HashMap::new(),
                ));
            }
            _ => return,
        }
    }

    fn next_id(&self) -> usize {
        self.elements.len().saturating_sub(1)
    }

    fn combine_effects(&self, mut style: Style) -> Style {
        self.current_effects.iter().for_each(|effect| {
            style = style.combine(*effect);
        });
        style
    }

    fn parse_header(&mut self, node: Node) {
        if let Some(headline_node) = node.find(Class("mw-headline")).into_selection().first() {
            let mut attributes = HashMap::new();

            if let Some(anchor) = headline_node.attr("id") {
                attributes.insert("anchor".to_string(), anchor.to_string());
            }

            self.push_newline();
            self.elements.push(Element::new(
                self.next_id(),
                ElementType::Header,
                headline_node.text(),
                Style::from(config::CONFIG.theme.title).combine(Effect::Bold),
                attributes,
            ));
            self.push_newline();
            self.push_newline();
        }
    }

    fn parse_paragraph(&mut self, node: Node) {
        self.parse_text(node);
        self.push_newline();
        self.push_newline();
    }

    fn parse_text(&mut self, node: Node) {
        for child in node.children() {
            if child.name().is_some() {
                info!("parsing node {:?} {}", child.name(), child.text());
                self.parse_node(child);
                continue;
            }

            info!("pushing text {}", child.text());
            self.elements.push(Element::new(
                self.next_id(),
                ElementType::Text,
                child.text(),
                self.combine_effects(Style::from(config::CONFIG.theme.text)),
                HashMap::new(),
            ))
        }
    }

    fn parse_link(&mut self, node: Node) {
        let target = node.attr("href");

        if target.is_none() {
            return;
        }

        let mut attributes = HashMap::new();
        attributes.insert("target".to_string(), target.unwrap().to_string());

        if target.unwrap().starts_with("https://") || target.unwrap().starts_with("http://") {
            attributes.insert("external".to_string(), String::new());
        }

        self.elements.push(Element::new(
            self.next_id(),
            ElementType::Link,
            node.text(),
            self.combine_effects(Style::from(config::CONFIG.theme.text).combine(Effect::Underline)),
            attributes,
        ));
    }

    fn parse_effect(&mut self, node: Node, effect: Effect) {
        self.current_effects.push(effect);
        self.parse_text(node);
        self.current_effects.pop();
    }

    fn parse_list(&mut self, node: Node) {
        for child in node
            .children()
            .filter(|x| x.name().unwrap_or_default() == "li")
        {
            self.push_newline();
            self.elements.push(Element::new(
                self.next_id(),
                ElementType::Text,
                "\t-".to_string(),
                self.combine_effects(Style::from(config::CONFIG.theme.text)),
                HashMap::new(),
            ));
            self.parse_text(child)
        }
        self.push_newline();
        self.push_newline();
    }

    fn push_newline(&mut self) {
        self.elements.push(Element::new(
            self.next_id(),
            ElementType::Newline,
            "",
            Style::none(),
            HashMap::new(),
        ));
    }
}

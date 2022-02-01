use crate::wiki::article::{element::ArticleElement, toc::TableOfContents};

/// A fully parsed article with an optional table of contents
#[derive(PartialEq, Debug)]
pub struct Article {
    elements: Vec<ArticleElement>,
    toc: Option<TableOfContents>,
}

impl Article {
    /// Creates a new article from given elements and a given table of contents. This should not be
    /// used directly, instead use the one the ArticleBuilder gives you
    pub fn new(elements: Vec<ArticleElement>, toc: Option<TableOfContents>) -> Self {
        Self { elements, toc }
    }

    /// Iterate over all of the elements contained in this article
    pub fn elements(&self) -> impl Iterator<Item = &ArticleElement> {
        self.elements.iter()
    }

    /// The optional table of contents
    pub fn toc(&self) -> Option<&TableOfContents> {
        self.toc.as_ref()
    }
}

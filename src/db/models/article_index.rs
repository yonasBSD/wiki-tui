use crate::db::schema::article_index;
use diesel::prelude::*;
use chrono::NaiveDateTime;

#[derive(Queryable, Associations, Identifiable, Debug)]
#[table_name="article_index"]
pub struct ArticleIndex {
    pub id: i32,
    pub page_id: i32,
    pub article_id: i32,
    pub namespace: i32,
    pub title: String,

    pub updated_at: NaiveDateTime
}

#[derive(Insertable)]
#[table_name="article_index"]
pub struct NewArticleIndex<'a> {
    pub page_id: &'a i32,
    pub article_id: &'a i32,
    pub namespace: &'a i32,
    pub title: &'a str,

    pub updated_at: &'a NaiveDateTime
}

type AllColumns = (
    article_index::id,
    article_index::page_id,
    article_index::article_id,
    article_index::namespace,
    article_index::title,
    article_index::updated_at
);

const ALL_COLUMNS: AllColumns = (
    article_index::id,
    article_index::page_id,
    article_index::article_id,
    article_index::namespace,
    article_index::title,
    article_index::updated_at
);

type All = diesel::dsl::Select<article_index::table, AllColumns>;
type WithTitle<'a> = diesel::dsl::Eq<article_index::title, &'a str>;
type WithId<'a> = diesel::dsl::Eq<article_index::article_id, &'a i32>;
type ByTitle<'a> = diesel::dsl::Filter<All, WithTitle<'a>>;
type ById<'a> = diesel::dsl::Filter<All, WithId<'a>>;

fn with_title(title: &str) -> WithTitle { article_index::title.eq(title) }
fn with_id(article_id: &i32) -> WithId { article_index::article_id.eq(article_id) }

impl ArticleIndex {
    pub fn all() -> All { article_index::table.select(ALL_COLUMNS) }
    pub fn by_id(id: &i32) -> ById { Self::all().filter(with_id(id)) }
    pub fn by_title(title: &str) -> ByTitle { Self::all().filter(with_title(title)) }
}
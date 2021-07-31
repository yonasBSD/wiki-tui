extern crate anyhow;
extern crate ini;
extern crate lazy_static;
extern crate log;

use anyhow::*;
use cursive::align::HAlign;
use cursive::theme::*;
use cursive::traits::*;
use cursive::utils::*;
use cursive::view::{Resizable, Scrollable};
use cursive::views::*;
use cursive::Cursive;

pub mod config;
pub mod logging;
pub mod ui;
pub mod wiki;

pub const LOGO: &str = "
  _      __   (_)   / /__   (_)         / /_  __  __   (_)
| | /| / /  / /   / //_/  / /  ______ / __/ / / / /  / /
| |/ |/ /  / /   / ,<    / /  /_____// /_  / /_/ /  / /
|__/|__/  /_/   /_/|_|  /_/          \\__/  \\__,_/  /_/ 
";

fn main() {
    // Initialize the logging module
    logging::Logger::initialize();

    // Create the wiki struct, used for interaction with the wikipedia website/api
    let wiki = wiki::WikiApi::new();

    let mut siv = cursive::default();
    siv.add_global_callback('q', Cursive::quit);
    siv.set_user_data(wiki);

    // get and apply the color theme
    let theme = Theme {
        palette: get_color_palette(),
        ..Default::default()
    };
    siv.set_theme(theme);

    // Create the views
    let search_bar = EditView::new()
        .on_submit(|s, q| ui::search::on_search(s, q.to_string()))
        .with_name("search_bar")
        .full_width();

    let search_layout = Dialog::around(LinearLayout::horizontal().child(search_bar))
        .title("Search")
        .title_position(cursive::align::HAlign::Left);

    let logo_view = TextView::new(LOGO)
        .h_align(HAlign::Center)
        .with_name("logo_view")
        .full_screen();

    let article_layout = LinearLayout::horizontal()
        .child(Dialog::around(logo_view))
        .with_name("article_layout");

    // Add a fullscreen layer, containing the search bar and the article view
    siv.add_fullscreen_layer(
        Dialog::around(
            LinearLayout::vertical()
                .child(search_layout)
                .child(article_layout),
        )
        .title("wiki-tui")
        .button("Quit", Cursive::quit)
        .full_screen(),
    );

    // Start the application
    siv.run();
}

fn get_color_palette() -> Palette {
    let mut custom_palette = Palette::default();

    custom_palette.set_color("View", config::CONFIG.theme.background);
    custom_palette.set_color("Primary", config::CONFIG.theme.text);
    custom_palette.set_color("TitlePrimary", config::CONFIG.theme.title);
    custom_palette.set_color("Highlight", config::CONFIG.theme.highlight);
    custom_palette.set_color("HighlightInactive", config::CONFIG.theme.highlight_inactive);
    custom_palette.set_color("HighlightText", config::CONFIG.theme.highlight_text);

    custom_palette
}

fn remove_view_from_article_layout(siv: &mut Cursive, view_name: &str) {
    siv.call_on_name("article_layout", |view: &mut LinearLayout| {
        if let Some(i) = view.find_child_from_name(view_name) {
            log::info!("Removing the {} from the article_layout", view_name);
            view.remove_child(i);
        } else {
            log::warn!("Couldn't find the {}", view_name);
        }
    });
}

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{List, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use tracing::{debug, info, warn};
use wiki_api::{
    document::{Data, Node},
    page::{Link, Page, Section},
};

use crate::{
    action::{Action, ActionPacket, ActionResult, PageAction},
    components::Component,
    config::Theme,
    has_modifier,
    renderer::{default_renderer::render_document, RenderedDocument},
    terminal::Frame,
    ui::padded_rect,
};

#[cfg(debug_assertions)]
use crate::renderer::test_renderer::{render_nodes_raw, render_tree_data, render_tree_raw};

const SCROLLBAR: bool = true;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Renderer {
    #[default]
    Default,

    #[cfg(debug_assertions)]
    TestRendererTreeData,
    #[cfg(debug_assertions)]
    TestRendererTreeRaw,
    #[cfg(debug_assertions)]
    TestRendererNodeRaw,
}

impl Renderer {
    pub fn next(&self) -> Self {
        match self {
            #[cfg(not(debug_assertions))]
            &Renderer::Default => Renderer::Default,

            #[cfg(debug_assertions)]
            &Renderer::Default => Renderer::TestRendererTreeData,
            #[cfg(debug_assertions)]
            &Renderer::TestRendererTreeData => Renderer::TestRendererTreeRaw,
            #[cfg(debug_assertions)]
            &Renderer::TestRendererTreeRaw => Renderer::TestRendererNodeRaw,
            #[cfg(debug_assertions)]
            &Renderer::TestRendererNodeRaw => Renderer::Default,
        }
    }
}

#[derive(Default)]
struct PageContentsState {
    list_state: ListState,
    max_idx_section: u8,
}

macro_rules! rendered_page {
    ($self: ident, $width: expr) => {
        match $self.rendered_page($width) {
            Some(page) => page,
            None => {
                $self.render_page($width);
                $self.rendered_page($width).unwrap()
            }
        }
    };
}

pub struct PageComponent {
    pub page: Page,
    renderer: Renderer,
    render_cache: HashMap<u16, RenderedDocument>,
    viewport: Rect,
    selected: (usize, usize),

    theme: Theme,

    is_contents: bool,
    contents_state: PageContentsState,
}

impl PageComponent {
    pub fn new(page: Page, theme: Theme) -> Self {
        let contents_state = PageContentsState {
            list_state: ListState::default().with_selected(Some(0)),
            max_idx_section: page.sections().map(|x| x.len() as u8).unwrap_or_default(),
        };
        Self {
            page,
            renderer: Renderer::default(),
            render_cache: HashMap::new(),
            viewport: Rect::default(),
            selected: (0, 0),

            theme,

            is_contents: false,
            contents_state,
        }
    }

    fn render_page(&mut self, width: u16) {
        let page = match self.renderer {
            Renderer::Default => render_document(&self.page.content, width),
            #[cfg(debug_assertions)]
            Renderer::TestRendererTreeData => render_tree_data(&self.page.content),
            #[cfg(debug_assertions)]
            Renderer::TestRendererTreeRaw => render_tree_raw(&self.page.content),
            #[cfg(debug_assertions)]
            Renderer::TestRendererNodeRaw => render_nodes_raw(&self.page.content),
        };

        self.render_cache.insert(width, page);
    }

    fn rendered_page(&self, width: u16) -> Option<&RenderedDocument> {
        self.render_cache.get(&width)
    }

    fn render_contents(&mut self, f: &mut Frame<'_>, area: Rect) {
        let sections = self.page.sections.as_ref();
        let mut block = self.theme.default_block().title("Contents");
        if self.is_contents {
            block = block.border_style(
                Style::default()
                    .fg(self.theme.border_highlight_fg)
                    .bg(self.theme.border_highlight_bg),
            );
        }

        if sections.is_none() {
            f.render_widget(
                self.theme
                    .default_paragraph("No Contents available")
                    .block(block),
                area,
            );
            return;
        }

        let sections = sections.unwrap();
        let list = List::new(
            sections
                .iter()
                .map(|x| format!("{} {}", x.number, x.text).fg(self.theme.fg)),
        )
        .block(block)
        .highlight_style(
            Style::default()
                .fg(self.theme.selected_fg)
                .bg(self.theme.selected_bg)
                .add_modifier(Modifier::ITALIC),
        );
        f.render_stateful_widget(list, area, &mut self.contents_state.list_state);
    }

    fn switch_renderer(&mut self, renderer: Renderer) {
        self.renderer = renderer;

        debug!("flushing '{}' cached renders", self.render_cache.len());
        self.render_cache.clear();
        self.selected = (0, 0);
    }

    fn select_header(&mut self, anchor: String) {
        // HACK: do not hardcode this
        if &anchor == "Content_Top" {
            info!("special case: jumping to top");
            self.viewport.y = 0;
            return;
        }

        let header_node = self
            .page
            .content
            .nth(0)
            .unwrap()
            .descendants()
            .filter(|node| {
                if let Data::Header { id, .. } = node.data() {
                    id == &anchor
                } else {
                    false
                }
            })
            .last();

        if header_node.is_none() {
            warn!("no header with the anchor '{}' could be found", anchor);
            return;
        }

        let header_node = header_node.unwrap();
        self.scroll_to_node(header_node.index());
    }

    fn selected_header(&self) -> Option<&Section> {
        let sections = self.page.sections()?;
        let section_idx = self.contents_state.list_state.selected()?;
        assert!(section_idx < self.contents_state.max_idx_section as usize);

        Some(&sections[section_idx])
    }

    /// Returns the y-Position of the selected element
    fn selected_y(&self) -> usize {
        let page = match self.render_cache.get(&self.viewport.width) {
            Some(page) => page,
            None => return 0,
        };

        for (y, line) in page.lines.iter().enumerate() {
            if line
                .iter()
                .any(|word| self.selected.0 <= word.index && self.selected.1 >= word.index)
            {
                return y;
            }
        }

        0
    }

    fn select_node(&mut self, idx: usize) {
        let node = match Node::new(&self.page.content, idx) {
            Some(node) => node,
            None => return,
        };

        let first_index = node.index();
        let last_index = node.last_child().map(|x| x.index()).unwrap_or(first_index);

        self.selected = (first_index, last_index);
        self.check_and_update_scrolling();
    }

    fn selected_node(&self) -> Option<Node> {
        self.page.content.nth(self.selected.0)
    }

    fn select_first(&mut self) {
        if self.page.content.nth(0).is_none() {
            return;
        }

        let selectable_node = self
            .page
            .content
            .nth(0)
            .unwrap()
            .descendants()
            .find(|node| matches!(node.data(), &Data::Link(_)));

        if let Some(node) = selectable_node {
            self.select_node(node.index())
        }
    }

    fn select_last(&mut self) {
        if self.page.content.nth(0).is_none() {
            return;
        }

        let selectable_node = self
            .page
            .content
            .nth(0)
            .unwrap()
            .descendants()
            .filter(|node| matches!(node.data(), &Data::Link(_)) && node.index() > self.selected.1)
            .last();

        if let Some(node) = selectable_node {
            self.select_node(node.index())
        }
    }

    fn select_next(&mut self) {
        if self.page.content.nth(0).is_none() {
            return;
        }

        let selectable_node = self
            .page
            .content
            .nth(0)
            .unwrap()
            .descendants()
            .find(|node| matches!(node.data(), &Data::Link(_)) && self.selected.1 < node.index());

        if let Some(node) = selectable_node {
            self.select_node(node.index())
        }
    }

    fn select_prev(&mut self) {
        if self.page.content.nth(0).is_none() {
            return;
        }

        let selectable_node = self
            .page
            .content
            .nth(0)
            .unwrap()
            .descendants()
            .filter(|node| matches!(node.data(), &Data::Link(_)) && node.index() < self.selected.0)
            .last();

        if let Some(node) = selectable_node {
            self.select_node(node.index())
        }
    }

    /// Checks if the current link is out of the viewport and moves the selection accordingly. If
    /// no links could be found in the current viewport, the selection stays as it was
    fn check_and_update_selection(&mut self) {
        let page = rendered_page!(self, self.viewport.width);

        let selected_y = self.selected_y() as u16;
        let selected_node = match self.selected_node() {
            Some(node) => node,
            None => return,
        };

        if self.viewport.contains((0_u16, selected_y).into()) {
            return;
        }

        if selected_y < self.viewport.top() {
            let (_, idx) = page
                .links
                .iter()
                .find(|(y, _)| self.viewport.contains((0, *y as u16).into()))
                .map(|x| x.to_owned())
                .unwrap_or((selected_y as usize, selected_node.index()));

            self.select_node(idx);
            return;
        }

        if selected_y > self.viewport.bottom() {
            let (_, idx) = page
                .links
                .iter()
                .rev()
                .find(|(y, _)| self.viewport.contains((0, *y as u16).into()))
                .map(|x| x.to_owned())
                .unwrap_or((selected_y as usize, selected_node.index()));

            self.select_node(idx)
        }
    }

    fn scroll_up(&mut self, amount: u16) {
        if self.is_contents {
            let i = match self.contents_state.list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.contents_state.max_idx_section as usize - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };

            self.contents_state.list_state.select(Some(i));
            return;
        }

        self.scroll_to_y(self.viewport.y.saturating_sub(amount));
    }

    fn scroll_down(&mut self, amount: u16) {
        if self.is_contents {
            let i = match self.contents_state.list_state.selected() {
                Some(i) => {
                    if i >= self.contents_state.max_idx_section as usize - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };

            self.contents_state.list_state.select(Some(i));
            return;
        }

        self.scroll_to_y(self.viewport.y + amount);
    }

    fn scroll_to_bottom(&mut self) {
        let page = rendered_page!(self, self.viewport.width);
        self.scroll_to_y(page.lines.len() as u16);
    }

    fn scroll_to_y(&mut self, y: u16) {
        let page = rendered_page!(self, self.viewport.width);
        let n_lines = page.lines.len() as u16;
        self.viewport.y = y;

        if self.viewport.bottom() >= n_lines {
            self.viewport.y = n_lines.saturating_sub(self.viewport.height);
        }

        self.check_and_update_selection();
    }

    fn scroll_to_node(&mut self, idx: usize) {
        let page = rendered_page!(self, self.viewport.width);
        let node = match Node::new(&self.page.content, idx) {
            Some(node) => node,
            None => return,
        };
        let first_index = idx;
        let last_index = node.last_child().map(|x| x.index()).unwrap_or(first_index);
        let y = page.lines.iter().enumerate().find_map(|(y, line)| {
            line.iter()
                .find(|word| {
                    if let Some(node) = word.node(&self.page.content) {
                        first_index <= node.index() && node.index() <= last_index
                    } else {
                        false
                    }
                })
                .map(|_| y)
        });

        if let Some(y) = y {
            self.scroll_to_y(y as u16);
        }
    }

    /// Checks if the current viewport shows the selected link and if not, moves the viewport so
    /// the link is visible
    fn check_and_update_scrolling(&mut self) {
        let selection_y = self.selected_y() as u16;

        if selection_y < self.viewport.top() {
            self.scroll_to_y(selection_y);
            return;
        }

        if selection_y >= self.viewport.bottom() {
            self.scroll_to_y(selection_y.saturating_sub(self.viewport.height) + 1);
        }
    }

    fn open_link(&self) -> ActionResult {
        let index = self.selected.0;
        let node = Node::new(&self.page.content, index).unwrap();
        let data = node.data().to_owned();

        let link = match data {
            Data::Link(link) => link,
            _ => {
                warn!("tried to open an element that is not a link");
                return ActionResult::Ignored;
            }
        };

        match link {
            Link::Internal(_) | Link::Anchor(_) => (),
            Link::External(link_data) => return Action::PopupMessage(
                "Warning".to_string(), 
                format!("This link doesn't point to another page. \nInstead, it leads to the following external webpage: \n\n{}", link_data.url.as_str())
            ).into(),
            Link::RedLink(link_data) => return Action::PopupMessage(
                "Information".to_string(), 
                format!("The page '{}' doesn't exist yet", link_data.title)
            ).into(),
            Link::MediaLink(_) | Link::ExternalToInternal(_) => {
                info!("tried to open an unsupported link '{:?}'", link);
                return Action::PopupMessage(
                    "Information".to_string(), 
                    "This type of link is not supported yet".to_string()
                ).into()
            }
        }

        Action::PopupDialog(
            "Information".to_string(),
            format!(
                "Do you want to open the page '{}'",
                link.title().unwrap_or("UNKNOWN")
            ),
            Box::<ActionPacket>::new(Action::LoadLink(link).into()),
        )
        .into()
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.viewport.width = width;
        self.viewport.height = height;
    }
}

impl Component for PageComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> ActionResult {
        if self.is_contents {
            return match key.code {
                KeyCode::Tab | KeyCode::BackTab => Action::Page(PageAction::ToggleContents).into(),
                KeyCode::Enter if self.contents_state.list_state.selected().is_some() => {
                    let header = self.selected_header();
                    if header.is_none() {
                        info!("no header selected");
                        return ActionResult::Ignored;
                    }
                    ActionPacket::single(Action::Page(PageAction::GoToHeader(
                        header.unwrap().anchor.to_string(),
                    )))
                    .action(Action::Page(PageAction::ToggleContents))
                    .into()
                }
                _ => ActionResult::Ignored,
            };
        }

        match key.code {
            KeyCode::Char('r') if has_modifier!(key, Modifier::CONTROL) => {
                Action::Page(PageAction::SwitchRenderer(self.renderer.next())).into()
            }
            KeyCode::Tab | KeyCode::BackTab => Action::Page(PageAction::ToggleContents).into(),
            KeyCode::Left if has_modifier!(key, Modifier::SHIFT) => {
                Action::Page(PageAction::SelectFirstLink).into()
            }
            KeyCode::Right if has_modifier!(key, Modifier::SHIFT) => {
                Action::Page(PageAction::SelectLastLink).into()
            }
            KeyCode::Up if has_modifier!(key, Modifier::SHIFT) => {
                Action::Page(PageAction::SelectTopLink).into()
            }
            KeyCode::Down if has_modifier!(key, Modifier::SHIFT) => {
                Action::Page(PageAction::SelectBottomLink).into()
            }
            KeyCode::Left => Action::Page(PageAction::SelectPrevLink).into(),
            KeyCode::Right => Action::Page(PageAction::SelectNextLink).into(),
            KeyCode::Enter => self.open_link(),
            _ => ActionResult::Ignored,
        }
    }

    fn update(&mut self, action: Action) -> ActionResult {
        match action {
            Action::Page(page_action) => match page_action {
                PageAction::SwitchRenderer(renderer) => self.switch_renderer(renderer),
                PageAction::ToggleContents => self.is_contents = !self.is_contents,

                PageAction::SelectFirstLink => self.select_first(),
                PageAction::SelectLastLink => self.select_last(),

                PageAction::SelectTopLink | PageAction::SelectBottomLink => todo!(),

                PageAction::SelectPrevLink => self.select_prev(),
                PageAction::SelectNextLink => self.select_next(),

                PageAction::GoToHeader(anchor) => self.select_header(anchor),
            },
            Action::ScrollUp(amount) => self.scroll_up(amount),
            Action::ScrollDown(amount) => self.scroll_down(amount),

            Action::ScrollHalfUp => self.scroll_up(self.viewport.height / 2),
            Action::ScrollHalfDown => self.scroll_down(self.viewport.height / 2),

            Action::ScrollToTop => self.scroll_to_y(0),
            Action::ScrollToBottom => self.scroll_to_bottom(),

            Action::Resize(width, heigth) => self.resize(width, heigth),
            _ => return ActionResult::Ignored,
        }
        ActionResult::consumed()
    }

    fn render(&mut self, f: &mut Frame, area: Rect) {
        let (area, status_area) = {
            let splits = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(100), Constraint::Min(1)])
                .split(padded_rect(area, 1, 1));
            (splits[0], splits[1])
        };

        let status_msg = format!(
            " wiki-tui | Page '{}' | Language '{}' | '{}' other languages available",
            self.page.title,
            self.page.language.name(),
            self.page.available_languages().unwrap_or_default()
        );
        f.render_widget(self.theme.default_paragraph(status_msg), status_area);

        let area = {
            let splits = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(area);

            self.render_contents(f, splits[1]);
            splits[0]
        };

        let page_area = if SCROLLBAR {
            area.inner(&Margin {
                vertical: 0,
                horizontal: 2, // for the scrollbar
            })
        } else {
            area
        };

        self.viewport.width = page_area.width;
        self.viewport.height = page_area.height;

        let rendered_page = rendered_page!(self, page_area.width);
        let mut lines: Vec<Line> = rendered_page
            .lines
            .iter()
            .skip(self.viewport.top() as usize)
            .take(self.viewport.bottom() as usize)
            .map(|line| {
                let mut spans: Vec<Span> = Vec::new();
                line.iter()
                    .map(|word| {
                        let mut span = Span::styled(
                            format!(
                                "{}{}",
                                word.content,
                                " ".repeat(word.whitespace_width as usize)
                            ),
                            word.style,
                        );

                        if let Some(node) = word.node(&self.page.content) {
                            let index = node.index();
                            if self.selected.0 <= index && index <= self.selected.1 {
                                span = span
                                    .patch_style(Style::new().add_modifier(Modifier::UNDERLINED))
                            }
                        }

                        spans.push(span);
                    })
                    .count();
                Line {
                    spans,
                    ..Default::default()
                }
            })
            .collect();

        if self.viewport.y == 0 {
            let title_line =
                Line::raw(&self.page.title).patch_style(Style::default().fg(Color::Red).bold());

            lines.insert(0, title_line);
            lines.pop();
        }

        if SCROLLBAR {
            let scrollbar = Scrollbar::default()
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some(" "))
                .track_style(
                    Style::new()
                        .fg(self.theme.scrollbar_track_fg)
                        .bg(self.theme.scrollbar_track_fg),
                )
                .thumb_style(Style::new().fg(self.theme.scrollbar_thumb_fg))
                .orientation(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state = ScrollbarState::new(
                rendered_page
                    .lines
                    .len()
                    .saturating_sub(self.viewport.height as usize),
            )
            .position(self.viewport.top() as usize);
            f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }

        f.render_widget(Paragraph::new(lines), page_area);
    }
}

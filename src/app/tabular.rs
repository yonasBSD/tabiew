use itertools::{izip, Itertools};
use polars::frame::DataFrame;
use rand::Rng;
use ratatui::{
    layout::{Alignment, Constraint, Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

use crate::{
    theme::Styler,
    utils::{data_frame_widths, line_count, Scroll, TableValues},
};

use super::AppResult;

#[derive(Debug)]
pub struct Tabular {
    offset: usize,
    select: usize,
    rendered_rows: u16,
    widths: Vec<usize>,
    headers: Vec<String>,
    table_values: TableValues,
    data_frame: DataFrame,
    scroll: Option<Scroll>,
}

impl Tabular {
    /// Constructs a new instance of [`App`].
    pub fn new(data_frame: DataFrame) -> Self {
        Self {
            offset: 0,
            select: 0,
            rendered_rows: 0,
            widths: data_frame_widths(&data_frame),
            headers: data_frame
                .get_column_names()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            table_values: TableValues::from_dataframe(&data_frame),
            data_frame,
            scroll: None,
        }
    }

    /// Handles the tick event of the terminal.
    pub fn tick(&mut self) {}

    pub fn select_up(&mut self, len: usize) -> AppResult<()> {
        self.select(self.select.saturating_sub(len))
    }

    pub fn select_down(&mut self, len: usize) -> AppResult<()> {
        self.select(self.select + len)
    }

    pub fn select_first(&mut self) -> AppResult<()> {
        self.select(usize::MIN)
    }

    pub fn select_last(&mut self) -> AppResult<()> {
        self.select(usize::MAX)
    }

    pub fn select_random(&mut self) -> AppResult<()> {
        let mut rng = rand::thread_rng();
        self.select(rng.gen_range(0..self.table_values.height()))
    }

    pub fn select(&mut self, select: usize) -> AppResult<()> {
        self.select = select.min(self.table_values.height().saturating_sub(1));
        Ok(())
    }

    pub fn scroll_up(&mut self) -> AppResult<()> {
        if let Some(scroll) = &mut self.scroll {
            scroll.up();
            Ok(())
        } else {
            Err("Not in detail view".into())
        }
    }

    pub fn scroll_down(&mut self) -> AppResult<()> {
        if let Some(scroll) = &mut self.scroll {
            scroll.down();
            Ok(())
        } else {
            Err("Not in detail view".into())
        }
    }

    pub fn page_len(&self) -> usize {
        self.rendered_rows.into()
    }

    pub fn adjust_offset(&mut self) {
        self.offset = self.offset.clamp(
            self.select
                .saturating_sub(self.rendered_rows.saturating_sub(1).into()),
            self.select,
        );
    }

    pub fn switch_view(&mut self) -> AppResult<()> {
        if self.scroll.is_none() {
            self.detail_view()
        } else {
            self.table_view()
        }
    }

    pub fn detail_view(&mut self) -> AppResult<()> {
        self.scroll = Scroll::default().into();
        Ok(())
    }

    pub fn table_view(&mut self) -> AppResult<()> {
        self.scroll = None;
        Ok(())
    }

    pub fn set_data_frame(&mut self, data_frame: DataFrame) -> AppResult<()> {
        self.widths = data_frame_widths(&data_frame);
        self.offset = 0;
        self.select = 0;
        self.headers = data_frame
            .get_column_names()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect();
        self.table_values.replace_dataframe(&data_frame);
        self.data_frame = data_frame;
        Ok(())
    }

    pub fn data_frame(&self) -> &DataFrame {
        &self.data_frame
    }

    pub fn scroll(&self) -> &Option<Scroll> {
        &self.scroll
    }

    pub fn selected(&self) -> usize {
        self.select
    }

    pub fn table_values(&self) -> &TableValues {
        &self.table_values
    }

    pub fn render<Theme: Styler>(&mut self, frame: &mut Frame, layout: Rect) -> AppResult<()> {
        if let Some(scroll) = &mut self.scroll {
            // Set visible rows = 0
            self.rendered_rows = 0;
            let space = layout.inner(Margin::new(1, 1));
            let title = format!(" {} ", self.select + 1);

            let values = self.table_values.get_row(self.select);

            let (paragraph, line_count) =
                paragraph_from_headers_values::<Theme>(&title, &self.headers, &values, space.width);

            scroll.adjust(line_count, space.height as usize);
            frame.render_widget(paragraph.scroll((scroll.to_u16(), 0)), layout);
        } else {
            // Set visible rows = table height - 1 (if header)
            self.rendered_rows = layout.height.saturating_sub(1);
            self.adjust_offset();

            let mut local_st = TableState::new()
                .with_offset(0)
                .with_selected(self.select.saturating_sub(self.offset));

            frame.render_stateful_widget(
                tabulate::<Theme>(
                    &self.table_values,
                    &self.widths,
                    &self.headers,
                    self.offset,
                    self.rendered_rows as usize,
                ),
                layout,
                &mut local_st,
            );
        }
        Ok(())
    }
}

fn paragraph_from_headers_values<'a, Theme: Styler>(
    title: &'a str,
    headers: &'a [String],
    values: &'a [&str],
    width: u16,
) -> (Paragraph<'a>, usize) {
    let lines = izip!(headers, values.iter())
        .enumerate()
        .flat_map(|(idx, (header, value))| lines_from_header_value::<Theme>(idx, header, value))
        .collect_vec();
    let lc = lines
        .iter()
        .map(|line| line_count(&line.to_string(), width as usize))
        .sum();
    let prgr = Paragraph::new(lines)
        .block(Block::new().title(title).borders(Borders::ALL))
        .style(Theme::item_block())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    (prgr, lc)
}

fn lines_from_header_value<'a, Theme: Styler>(
    idx: usize,
    header: &'a str,
    value: &'a str,
) -> Vec<Line<'a>> {
    let header_line = std::iter::once(Line::from(Span::styled(
        header,
        Theme::table_header_cell(idx),
    )));
    let value_lines = value
        .lines()
        .map(|line| Line::from(Span::styled(line, Theme::table_cell(idx, 0))));
    header_line
        .chain(value_lines)
        .chain(std::iter::once(Line::default()))
        .collect_vec()
}

pub fn tabulate<'a, Theme: Styler>(
    value_pool: &'a TableValues,
    widths: &'a [usize],
    headers: &'a [String],
    offset: usize,
    length: usize,
) -> Table<'a> {
    Table::new(
        (offset..offset + length)
            .map(|row_idx| {
                Row::new(value_pool.get_row(row_idx).into_iter().map(Cell::new))
                    .style(Theme::table_row(row_idx))
            })
            .collect_vec(),
        widths
            .iter()
            .copied()
            .map(|w| Constraint::Length(w as u16))
            .collect::<Vec<_>>(),
    )
    .header(header_row::<Theme>(headers))
    .highlight_style(Theme::table_highlight())
}

fn header_row<Theme: Styler>(df: &[String]) -> Row {
    Row::new(
        df.iter()
            .enumerate()
            .map(|(col_idx, name)| {
                Cell::new(name.as_str()).style(Theme::table_header_cell(col_idx))
            })
            .collect::<Vec<_>>(),
    )
    .style(Theme::table_header())
}

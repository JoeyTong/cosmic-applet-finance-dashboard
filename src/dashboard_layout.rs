// SPDX-License-Identifier: GPL-3.0-only

//! Overview size for the current clocks, quotes, and calendar events.
//!
//! The overview stays at the readability floor while it fits the supplied
//! budget. A second clock row or forex column is used only when the floor no
//! longer fits. Past that, the same arrangement shrinks until it does.

/// Gap between the dashboard cards.
pub const GAP: f32 = 14.0;

const CARD_PAD: f32 = 12.0;
const CLOCK_CHROME: f32 = 56.0;
const CLOCK_CELL_W: f32 = 132.0;
const CLOCK_CELL_H: f32 = 134.0;
const CLOCK_FACE: f32 = 100.0;
const CLOCK_GAP_X: f32 = 8.0;
const CLOCK_GAP_Y: f32 = 4.0;
const FOREX_ROW_H: f32 = 32.0;
const FOREX_GAP_X: f32 = 10.0;
const FOREX_FIXED: f32 = 110.0;
const MARKET_ERROR_H: f32 = 18.0;
const DAY_W: f32 = 108.0;
const DAY_GAP: f32 = 5.0;
const CAL_CHROME: f32 = 132.0;
const DATE_H: f32 = 36.0;
const EVENT_H: f32 = 21.0;
/// Date-only day box was `DATE_H + 8`. The floor is three times that height.
const DAY_MIN_H: f32 = (DATE_H + 8.0) * 3.0;
const LEGEND_ROW: f32 = 18.0;
const AGENDA_CHROME: f32 = 48.0;
const AGENDA_ROW: f32 = 24.0;
const PROMPT_H: f32 = 44.0;
const WORD_H: f32 = 112.0;

/// Usable overview area after the panel, header, and screen margin.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub width: f32,
    pub height: f32,
}

/// Counts that change the overview size.
#[derive(Clone, Copy, Debug)]
pub struct Load {
    pub clocks: usize,
    pub forex: usize,
    /// Longest pair or commodity label, in characters.
    pub description_chars: usize,
    pub week_events: [usize; 2],
    pub agenda_events: usize,
    pub legend_rows: usize,
    pub market_error: bool,
    pub show_calendar_prompt: bool,
}

/// Card sizes for one overview. `scale` is 1 at the readability floor.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub scale: f32,
    pub overview_width: f32,
    pub overview_height: f32,
    pub content_width: f32,
    pub left_width: f32,
    pub right_width: f32,
    pub top_height: f32,
    pub lower_height: f32,
    pub calendar_height: f32,
    pub word_height: f32,
    pub slack: f32,
    pub clock_rows: usize,
    pub clock_columns: usize,
    pub clock_face: f32,
    pub clock_cell_width: f32,
    pub clock_cell_height: f32,
    pub forex_columns: usize,
    pub forex_rows: usize,
    pub forex_row_height: f32,
    pub forex_column_width: f32,
    pub week_heights: [f32; 2],
    pub event_line: f32,
    pub agenda_height: f32,
    pub commodity_height: f32,
}

/// One forex column wide enough for the flags, the label, the price, and the change.
pub fn markets_column_width(description_chars: usize, scale: f32) -> f32 {
    let title = (description_chars as f32 * 6.2).clamp(96.0, 240.0);
    let pair = 42.0 + title + 86.0 + 76.0 + 24.0;
    let tile = (description_chars as f32 * 5.6).clamp(120.0, 220.0) + 16.0;
    let commodities = tile * 2.0 + 10.0;
    pair.max(commodities) * scale
}

/// Largest readable overview that fits `budget`.
pub fn measure(load: &Load, budget: Budget) -> Frame {
    let mut scale = 1.0_f32;
    loop {
        if let Some(frame) = arrange(load, budget, scale) {
            return frame;
        }
        if scale <= 0.15 {
            return build(load, scale, 1, load.forex.max(1));
        }
        scale = (scale - 0.025).max(0.15);
    }
}

fn arrange(load: &Load, budget: Budget, scale: f32) -> Option<Frame> {
    let max_clock_rows = load.clocks.max(1);
    let max_forex_cols = load.forex.max(1);
    for forex_cols in 1..=max_forex_cols {
        for clock_rows in 1..=max_clock_rows {
            let frame = build(load, scale, clock_rows, forex_cols);
            if frame.overview_width <= budget.width + 0.5
                && frame.overview_height <= budget.height + 0.5
            {
                return Some(frame);
            }
        }
    }
    None
}

fn build(load: &Load, scale: f32, clock_rows: usize, forex_cols: usize) -> Frame {
    let clock_rows = if load.clocks == 0 {
        1
    } else {
        clock_rows.clamp(1, load.clocks)
    };
    let clock_columns = if load.clocks == 0 {
        1
    } else {
        load.clocks.div_ceil(clock_rows)
    };
    let forex_columns = if load.forex == 0 {
        1
    } else {
        forex_cols.clamp(1, load.forex)
    };
    let forex_rows = if load.forex == 0 {
        0
    } else {
        load.forex.div_ceil(forex_columns)
    };

    let clock_cell_width = CLOCK_CELL_W * scale;
    let clock_cell_height = CLOCK_CELL_H * scale;
    let clocks_inner_w = if load.clocks == 0 {
        180.0
    } else {
        clock_columns as f32 * clock_cell_width + (clock_columns - 1) as f32 * CLOCK_GAP_X
    };
    let clocks_inner_h = CLOCK_CHROME
        + if load.clocks == 0 {
            24.0
        } else {
            clock_rows as f32 * clock_cell_height + (clock_rows - 1) as f32 * CLOCK_GAP_Y
        };

    let day_w = DAY_W * scale;
    let calendar_inner_w = 7.0 * day_w + 6.0 * DAY_GAP;
    let event_line = EVENT_H * scale;
    let day_floor = DAY_MIN_H * scale;
    let week_height = |events: usize| (DATE_H + 8.0 + events as f32 * event_line).max(day_floor);
    let week_heights = [
        week_height(load.week_events[0]),
        week_height(load.week_events[1]),
    ];
    let mut agenda_height = if load.show_calendar_prompt {
        0.0
    } else {
        AGENDA_CHROME + load.agenda_events as f32 * AGENDA_ROW * scale
    };
    let calendar_inner_h = if load.show_calendar_prompt {
        CAL_CHROME + week_heights[0] + week_heights[1] + PROMPT_H
    } else {
        CAL_CHROME
            + week_heights[0]
            + week_heights[1]
            + load.legend_rows as f32 * LEGEND_ROW
            + agenda_height
    };

    let forex_column_width = markets_column_width(load.description_chars, scale);
    let mut forex_row_height = FOREX_ROW_H * scale;
    let markets_inner_w = forex_columns as f32 * forex_column_width
        + (forex_columns.saturating_sub(1)) as f32 * FOREX_GAP_X;
    let quotes_h = if forex_rows == 0 {
        24.0
    } else {
        forex_rows as f32 * forex_row_height + (forex_rows - 1) as f32
    };
    let mut commodity_height = 78.0 * scale;
    let markets_inner_h = FOREX_FIXED
        + quotes_h
        + if load.market_error {
            MARKET_ERROR_H
        } else {
            0.0
        }
        + commodity_height;

    let left_inner = clocks_inner_w.max(calendar_inner_w);
    let left_width = left_inner + CARD_PAD * 2.0;
    let right_width = markets_inner_w + CARD_PAD * 2.0;
    // The clock stack sets the top-row height. Public IP stays inside that height.
    let top_height = clocks_inner_h + CARD_PAD * 2.0;
    let mut calendar_height = calendar_inner_h + CARD_PAD * 2.0;
    let word_height = (WORD_H * scale).round();
    let markets_height = markets_inner_h + CARD_PAD * 2.0;
    let lower_height = (calendar_height + GAP + word_height)
        .max(markets_height)
        .round();
    if calendar_height + GAP + word_height + 0.5 < lower_height {
        agenda_height += lower_height - (calendar_height + GAP + word_height);
    }
    calendar_height = lower_height - GAP - word_height;
    if markets_height + 0.5 < lower_height {
        let extra = lower_height - markets_height;
        let quotes = if forex_rows == 0 {
            0.0
        } else {
            forex_rows as f32 * forex_row_height
        };
        let flex = (quotes + commodity_height).max(1.0);
        if forex_rows > 0 {
            forex_row_height += extra * quotes / flex / forex_rows as f32;
        }
        commodity_height += extra * commodity_height / flex;
    }
    let content_width = left_width + GAP + right_width;
    let overview_height = top_height + GAP + lower_height;

    Frame {
        scale,
        overview_width: content_width.round(),
        overview_height: overview_height.round(),
        content_width: content_width.round(),
        left_width: left_width.round(),
        right_width: right_width.round(),
        top_height: top_height.round(),
        lower_height: lower_height.round(),
        calendar_height,
        word_height,
        slack: 0.0,
        clock_rows,
        clock_columns,
        clock_face: CLOCK_FACE * scale,
        clock_cell_width,
        clock_cell_height,
        forex_columns,
        forex_rows,
        forex_row_height,
        forex_column_width,
        week_heights,
        event_line,
        agenda_height,
        commodity_height,
    }
}

#[cfg(test)]
mod tests {
    use super::{Budget, Load, measure};

    fn load(clocks: usize, forex: usize, week_events: [usize; 2]) -> Load {
        Load {
            clocks,
            forex,
            description_chars: 34,
            week_events,
            agenda_events: week_events[0].min(week_events[1]),
            legend_rows: 1,
            market_error: false,
            show_calendar_prompt: false,
        }
    }

    fn wide() -> Budget {
        Budget {
            width: 1800.0,
            height: 1200.0,
        }
    }

    #[test]
    fn modest_content_keeps_the_readability_floor() {
        let frame = measure(&load(4, 11, [0, 0]), wide());
        assert!((frame.scale - 1.0).abs() < 0.001);
        assert_eq!(frame.clock_rows, 1);
        assert_eq!(frame.clock_columns, 4);
        assert_eq!(frame.forex_columns, 1);
        assert!((frame.clock_face - 100.0).abs() < 0.001);
        assert!((frame.forex_row_height - 32.0).abs() < 0.001 || frame.forex_row_height > 32.0);
        assert!(frame.forex_column_width < 500.0);
        assert!(
            (frame.calendar_height + super::GAP + frame.word_height - frame.lower_height).abs()
                < 1.5
        );
    }

    #[test]
    fn shorter_labels_make_a_narrower_markets_column() {
        let mut narrow = load(4, 4, [0, 0]);
        narrow.description_chars = 12;
        let mut wide_labels = load(4, 4, [0, 0]);
        wide_labels.description_chars = 34;
        let narrow = measure(&narrow, wide());
        let wide_labels = measure(&wide_labels, wide());
        assert!(narrow.forex_column_width < wide_labels.forex_column_width);
    }

    #[test]
    fn empty_day_boxes_are_three_times_the_previous_floor() {
        let frame = measure(&load(4, 2, [0, 0]), wide());
        assert!((frame.week_heights[0] - 132.0).abs() < 0.5);
        assert!((frame.week_heights[1] - 132.0).abs() < 0.5);
        let busy = measure(&load(4, 2, [6, 1]), wide());
        assert!(busy.week_heights[0] > 132.0);
        assert!(busy.week_heights[1] >= 132.0 - 0.5);
    }

    #[test]
    fn more_events_make_a_taller_calendar_once_they_pass_the_floor() {
        let empty = measure(&load(4, 2, [0, 0]), wide());
        let busy = measure(&load(4, 2, [8, 8]), wide());
        assert!(busy.week_heights[0] > empty.week_heights[0]);
        assert!(busy.overview_height > empty.overview_height);
        for frame in [empty, busy] {
            assert!(
                (frame.calendar_height + super::GAP + frame.word_height - frame.lower_height).abs()
                    < 1.5
            );
        }
    }

    #[test]
    fn a_narrow_screen_wraps_before_it_shrinks_and_still_fits() {
        let wide_frame = measure(&load(8, 24, [0, 0]), wide());
        let tight = measure(
            &load(8, 24, [0, 0]),
            Budget {
                width: 1100.0,
                height: 700.0,
            },
        );
        assert!(tight.overview_width <= 1100.5);
        assert!(tight.overview_height <= 700.5);
        assert!(tight.clock_rows > 1 || tight.forex_columns > 1 || tight.scale < 1.0);
        assert!(tight.overview_width <= wide_frame.overview_width + 0.5 || tight.scale < 1.0);
    }
}

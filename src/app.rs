use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike, Utc, Weekday};
use cosmic::{
    Element, Task, app,
    cctk::wayland_client::protocol::wl_output::WlOutput,
    iced::{
        self, Alignment, Color, Length, Rectangle, Subscription,
        advanced::layout::Limits,
        event::{
            PlatformSpecific,
            wayland::{Event as WaylandEvent, OutputEvent},
        },
        platform_specific::shell::wayland::commands::{
            layer_surface::{Anchor, KeyboardInteractivity, Layer, destroy_layer_surface},
            popup::destroy_popup,
        },
        runtime::platform_specific::wayland::layer_surface::{
            IcedOutput, SctkLayerSurfaceSettings,
        },
        widget::{column, row, stack},
        window,
    },
    widget::{
        self, button, container, divider, icon, rectangle_tracker::*, scrollable, text, text_input,
        tooltip,
    },
};
use cosmic_applet_finance_dashboard::{
    calendar::{self, CalendarEvent, CalendarSnapshot},
    config::{self, Clock, PanelClock, Settings},
    login,
    market::{self, MarketSnapshot, Quote},
    network::{self, PublicIp, TimeSync},
    oauth,
};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::LazyLock,
    time::{Duration, Instant},
};

pub const WIDTH: f32 = 1600.0;
pub const HEIGHT: f32 = 940.0;
static PANEL_AUTOSIZE: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("finance-dashboard-panel"));
static BODY_SCROLL: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("finance-dashboard-body"));
static CLOCK_EDIT_LABEL: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("finance-dashboard-clock-edit-label"));

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    SurfacePressed(window::Id),
    Output(WlOutput, String),
    OutputRemoved(WlOutput),
    TogglePopup,
    Close,
    Closed(window::Id),
    Rectangle(RectangleUpdate<u32>),
    Settings(bool),
    Refresh,
    Time(Result<TimeSync, String>),
    Markets(Result<MarketSnapshot, String>),
    Ip(Result<PublicIp, String>),
    Calendar(u64, Result<CalendarSnapshot, String>),
    BannerInput(String),
    SaveBanner,
    CancelBanner,
    ClockLabel(String),
    ClockZone(String),
    AddClock,
    RemoveClock(usize),
    EditClock(usize),
    EditClockLabel(String),
    EditClockZone(String),
    SaveClock,
    CancelClockEdit,
    PairInput(String),
    AddPair,
    RemovePair(usize),
    ClientPath(String),
    BrowseClient,
    ClientChosen(Result<Option<String>, String>),
    Email(String),
    Connect,
    Connected(Result<String, String>),
    Disconnect,
    Disconnected(Result<(), String>),
    ToggleCalendar(String, bool),
    SelectDay(NaiveDate),
    Open(String),
    Opened(Result<(), String>),
    ClearNotice,
}

struct ClockEdit {
    index: usize,
    label: String,
    timezone: String,
}

pub struct FinanceDashboard {
    core: app::Core,
    preview: bool,
    popup: Option<window::Id>,
    backdrop: Option<window::Id>,
    outputs: Vec<(WlOutput, String)>,
    login_popup_pending: bool,
    rectangle: Rectangle,
    rectangle_tracker: Option<RectangleTracker<u32>>,
    settings: Settings,
    settings_open: bool,
    panel_config: PanelClock,
    locale: icu::locale::Locale,
    panel_now: jiff::Zoned,
    world_now: DateTime<Utc>,
    os_sample: Instant,
    os_time: DateTime<Utc>,
    client: reqwest::Client,
    time: Option<TimeSync>,
    time_error: Option<String>,
    time_busy: bool,
    time_attempt: Option<Instant>,
    quotes: BTreeMap<String, Quote>,
    unavailable: Vec<String>,
    market_error: Option<String>,
    market_busy: bool,
    market_attempt: Option<Instant>,
    ip: Option<PublicIp>,
    ip_error: Option<String>,
    ip_busy: bool,
    ip_attempt: Option<Instant>,
    calendar: CalendarSnapshot,
    calendar_error: Option<String>,
    calendar_busy: bool,
    calendar_attempt: Option<Instant>,
    calendar_generation: u64,
    calendar_start: NaiveDate,
    calendar_day: NaiveDate,
    selected_day: NaiveDate,
    banner_input: String,
    clock_label: String,
    clock_zone: String,
    clock_edit: Option<ClockEdit>,
    pair_input: String,
    client_path: String,
    email: String,
    auth_busy: bool,
    choosing_client: bool,
    notice: Option<String>,
}

fn job<T: Send + 'static>(
    future: impl Future<Output = anyhow::Result<T>> + Send + 'static,
    message: impl Fn(Result<T, String>) -> Message + Send + 'static,
) -> app::Task<Message> {
    Task::perform(
        async move { future.await.map_err(|e| format!("{e:#}")) },
        move |result| cosmic::Action::App(message(result)),
    )
}

fn due(last: Option<Instant>, seconds: u64) -> bool {
    last.is_none_or(|instant| instant.elapsed() >= Duration::from_secs(seconds))
}

fn input_event(event: iced::Event, _: iced::event::Status, id: window::Id) -> Option<Message> {
    // Popup lifetime follows explicit clicks and dismissal, not pointer motion
    // or keyboard focus (including COSMIC's focus-follows-cursor).
    match event {
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
        | iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
            Some(Message::SurfacePressed(id))
        }
        iced::Event::PlatformSpecific(PlatformSpecific::Wayland(event)) => match event {
            WaylandEvent::Output(
                OutputEvent::Created(Some(info)) | OutputEvent::InfoUpdate(info),
                output,
            ) => Some(Message::Output(output, info.name.unwrap_or_default())),
            WaylandEvent::Output(OutputEvent::Removed, output) => {
                Some(Message::OutputRemoved(output))
            }
            _ => None,
        },
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Close),
        _ => None,
    }
}

impl FinanceDashboard {
    fn refresh_due(&mut self) -> app::Task<Message> {
        let mut tasks = Vec::new();
        if !self.time_busy && due(self.time_attempt, 3600) {
            self.time_busy = true;
            self.time_attempt = Some(Instant::now());
            let client = self.client.clone();
            tasks.push(job(
                async move { network::sync_time(&client).await },
                Message::Time,
            ));
        }
        if !self.market_busy && due(self.market_attempt, 600) {
            self.market_busy = true;
            self.market_attempt = Some(Instant::now());
            let client = self.client.clone();
            let pairs = self.settings.forex.clone();
            tasks.push(job(
                async move { market::fetch(&client, &pairs).await },
                Message::Markets,
            ));
        }
        if !self.ip_busy && due(self.ip_attempt, 60) {
            self.ip_busy = true;
            self.ip_attempt = Some(Instant::now());
            let client = self.client.clone();
            tasks.push(job(
                async move { network::public_ip(&client).await },
                Message::Ip,
            ));
        }
        if !self.calendar_busy
            && !self.auth_busy
            && !self.settings.google_email.is_empty()
            && due(self.calendar_attempt, 600)
        {
            self.calendar_busy = true;
            self.calendar_attempt = Some(Instant::now());
            let client = self.client.clone();
            let settings = self.settings.clone();
            let today = Local::now().date_naive();
            let first_day = self.panel_config.first_day_of_week;
            let generation = self.calendar_generation;
            tasks.push(job(
                async move { calendar::fetch(&client, &settings, today, first_day).await },
                move |result| Message::Calendar(generation, result),
            ));
        }
        Task::batch(tasks)
    }

    fn invalidate_calendar(&mut self) {
        self.calendar_generation += 1;
        self.calendar_attempt = None;
        self.calendar_busy = false;
    }

    fn save(&mut self, settings: Settings) -> bool {
        match settings.save() {
            Ok(()) => {
                self.settings = settings;
                self.notice = None;
                true
            }
            Err(error) => {
                self.notice = Some(format!("Could not save settings: {error:#}"));
                false
            }
        }
    }

    fn open_popup(&mut self) -> app::Task<Message> {
        if self.preview || self.popup.is_some() {
            return Task::none();
        }
        let id = window::Id::unique();
        self.popup = Some(id);
        let open = cosmic::surface::surface_task(cosmic::surface::action::app_popup(
            |_| Default::default(),
            move |app: &mut Self| {
                let mut settings = app.core.applet.get_popup_settings(
                    app.core.main_window_id().unwrap(),
                    id,
                    Some((WIDTH as u32, HEIGHT as u32)),
                    None,
                    None,
                );
                // The same serial-free popup works for clicks and login;
                // its companion layer receives outside-click dismissal.
                settings.grab = false;
                settings.positioner.anchor_rect = Rectangle {
                    x: app.rectangle.x.max(1.0) as i32,
                    y: app.rectangle.y.max(1.0) as i32,
                    width: app.rectangle.width.max(1.0) as i32,
                    height: app.rectangle.height.max(1.0) as i32,
                };
                settings.positioner.size_limits = Limits::NONE
                    .min_width(480.0)
                    .max_width(WIDTH)
                    .min_height(300.0)
                    .max_height(HEIGHT);
                settings
            },
            None,
        ));
        open.chain(self.open_backdrop())
    }

    fn open_backdrop(&mut self) -> app::Task<Message> {
        if self.backdrop.is_some() {
            return Task::none();
        }
        let id = window::Id::unique();
        self.backdrop = Some(id);
        let output = self
            .outputs
            .iter()
            .find(|(_, name)| *name == self.core.applet.output_name)
            .or_else(|| (self.outputs.len() == 1).then(|| &self.outputs[0]))
            .map_or(IcedOutput::Active, |(output, _)| {
                IcedOutput::Output(output.clone())
            });
        cosmic::surface::surface_task(cosmic::surface::action::app_layer_shell(
            |_| cosmic::surface::action::LiveSettings {
                blur: Some(false),
                corners: Some(Default::default()),
                padding: Some(Default::default()),
            },
            move |_: &mut Self| SctkLayerSurfaceSettings {
                id,
                // COSMIC stacks panel popups above Top-layer surfaces. A neutral
                // exclusive zone respects the panel's reserved space, so its clock
                // remains clickable. The backdrop itself is entirely transparent.
                layer: Layer::Top,
                anchor: Anchor::all(),
                size: Some((None, None)),
                size_limits: Limits::NONE,
                exclusive_zone: 0,
                keyboard_interactivity: KeyboardInteractivity::OnDemand,
                output: output.clone(),
                namespace: format!("{}.dismiss", cosmic_applet_finance_dashboard::APP_ID),
                ..Default::default()
            },
            Some(Box::new(|_: &Self| {
                widget::space()
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            })),
        ))
    }

    fn consume_login_popup(&mut self) -> bool {
        if !std::mem::take(&mut self.login_popup_pending) {
            return false;
        }
        match login::claim_current_session() {
            Ok(first_open) => first_open,
            Err(error) => {
                tracing::warn!("Could not track login dashboard opening: {error:#}");
                false
            }
        }
    }

    fn open_login_popup_if_ready(&mut self) -> app::Task<Message> {
        // Wait for the panel to map and lay out the applet before anchoring a popup.
        if self.preview
            || self.core.main_window_id().is_none()
            || self.rectangle.width <= 0.0
            || self.rectangle.height <= 0.0
            || !self.consume_login_popup()
        {
            return Task::none();
        }
        self.settings_open = false;
        // Neither surface needs an input serial, including at login.
        self.open_popup()
    }

    fn close_popup(&mut self) -> app::Task<Message> {
        let mut tasks = Vec::new();
        if let Some(id) = self.popup.take() {
            tasks.push(destroy_popup(id));
        }
        if let Some(id) = self.backdrop.take() {
            tasks.push(destroy_layer_surface(id));
        }
        Task::batch(tasks)
    }

    fn header(&self) -> Element<'_, Message> {
        let mut controls = row![].align_y(Alignment::Center).spacing(8);
        if self.settings_open {
            controls = controls
                .push(button::standard("Back to dashboard").on_press(Message::Settings(false)));
        } else {
            controls = controls.push(tooltip(
                button::icon(icon::from_name("view-refresh-symbolic")).on_press(Message::Refresh),
                text("Refresh all modules"),
                tooltip::Position::Bottom,
            ));
            controls = controls.push(tooltip(
                button::icon(icon::from_name("emblem-system-symbolic"))
                    .on_press(Message::Settings(true)),
                text("Settings"),
                tooltip::Position::Bottom,
            ));
        }
        if self.settings_open {
            row![text::title3("Settings").width(Length::Fill), controls]
                .align_y(Alignment::Center)
                .spacing(8)
                .padding([10, 16])
                .into()
        } else {
            // Equal clearances keep the message centered on the dashboard,
            // independent of the right-hand controls.
            container(stack![
                container(banner(&self.settings.banner_message))
                    .padding([0, 100])
                    .width(Length::Fill),
                container(controls).align_right(Length::Fill).center_y(52),
            ])
            .padding([16, 20])
            .width(Length::Fill)
            .into()
        }
    }

    fn dashboard(&self) -> Element<'_, Message> {
        let mut content = column![self.header(), divider::horizontal::default()].spacing(0);
        if let Some(notice) = &self.notice {
            content = content.push(
                container(
                    row![
                        text::caption(notice).width(Length::Fill),
                        button::text("Dismiss").on_press(Message::ClearNotice)
                    ]
                    .align_y(Alignment::Center)
                    .spacing(8),
                )
                .padding([8, 16])
                .class(cosmic::theme::Container::Card),
            );
        }
        let body: Element<_> = if self.settings_open {
            scrollable(
                container(self.settings_view())
                    .padding(16)
                    .width(Length::Fill),
            )
            .id(BODY_SCROLL.clone())
            .height(Length::Fill)
            .into()
        } else {
            container(self.overview())
                .padding(14)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };
        content = content.push(body);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn overview(&self) -> Element<'_, Message> {
        widget::responsive(move |size| {
            let gap = 14.0;
            let top_height = (size.height * 0.245).clamp(80.0, 208.0);
            let lower_height = (size.height - top_height - gap).max(80.0);
            let available_width = (size.width - gap).max(1.0);
            let top = row![
                container(self.clocks_view(available_width * 0.64 - 24.0, top_height - 24.0))
                    .padding(12)
                    .width(Length::FillPortion(16))
                    .height(Length::Fill)
                    .class(cosmic::theme::Container::Card),
                container(self.ip_view(top_height - 24.0))
                    .padding(12)
                    .width(Length::FillPortion(9))
                    .height(Length::Fill)
                    .class(cosmic::theme::Container::Card),
            ]
            .spacing(gap)
            .height(top_height);
            let lower = row![
                container(self.calendar_view(available_width * 0.52 - 24.0, lower_height - 24.0))
                    .padding(12)
                    .width(Length::FillPortion(13))
                    .height(Length::Fill)
                    .class(cosmic::theme::Container::Card),
                container(self.markets_view(available_width * 0.48 - 24.0, lower_height - 24.0))
                    .padding(12)
                    .width(Length::FillPortion(12))
                    .height(Length::Fill)
                    .class(cosmic::theme::Container::Card),
            ]
            .spacing(gap)
            .height(lower_height);
            column![top, lower]
                .spacing(gap)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        })
        .into()
    }

    fn clocks_view(&self, width: f32, height: f32) -> Element<'_, Message> {
        let status = if let Some(error) = &self.time_error {
            error.clone()
        } else if let Some(sync) = &self.time {
            format!(
                "Synced {} · {} · OS sampled every 30s",
                local_time(sync.fetched_at),
                sync.source
            )
        } else {
            "OS time · synchronizing with public time server…".into()
        };
        let mut clocks = column![
            row![
                text::title4("World clocks"),
                widget::space::horizontal(),
                text::caption(if self.time_busy {
                    "Syncing…"
                } else {
                    "Hourly sync"
                })
            ]
            .align_y(Alignment::Center)
        ]
        .spacing(4);
        let columns = self.settings.clocks.len().clamp(1, 8);
        let rows = self.settings.clocks.len().div_ceil(columns).max(1);
        let cell_height = ((height - 48.0 - 4.0 * (rows - 1) as f32) / rows as f32).max(8.0);
        let cell_width = ((width - 8.0 * (columns - 1) as f32) / columns as f32).max(8.0);
        let scale = (cell_height / 142.0)
            .min(cell_width / 160.0)
            .clamp(0.03, 1.0);
        let diameter = (cell_height - 31.0 * scale)
            .min(cell_width - 4.0)
            .clamp(4.0, 112.0);
        for group in self.settings.clocks.chunks(columns) {
            let mut clock_row = widget::Row::new().spacing(8);
            for clock in group {
                if let Ok(zone) = clock.timezone.parse::<chrono_tz::Tz>() {
                    let time = self.world_now.with_timezone(&zone);
                    let clock_face = widget::canvas(crate::analog::AnalogClock {
                        hour: time.hour(),
                        minute: time.minute(),
                    })
                    .width(diameter)
                    .height(diameter);
                    let view = column![
                        text(clock.label.clone())
                            .size(14.0 * scale)
                            .line_height(1.0),
                        clock_face,
                        text(time.format("%a %d %b · %Z · %p").to_string())
                            .size(11.0 * scale)
                            .line_height(1.0),
                    ]
                    .align_x(Alignment::Center)
                    .spacing(2.0 * scale)
                    .height(cell_height)
                    .width(Length::Fill);
                    clock_row = clock_row.push(tooltip(
                        view,
                        text(clock.timezone.clone()),
                        tooltip::Position::Bottom,
                    ));
                }
            }
            for _ in group.len()..columns {
                clock_row = clock_row.push(widget::space::horizontal());
            }
            clocks = clocks.push(clock_row);
        }
        if self.settings.clocks.is_empty() {
            clocks = clocks.push(text::caption("Add a world clock in Settings."));
        }
        clocks
            .push(status_line(status, (width / 6.0).max(12.0) as usize))
            .into()
    }

    fn event_chip<'a>(
        &self,
        event: &'a CalendarEvent,
        compact: bool,
        width: f32,
        height: f32,
    ) -> Element<'a, Message> {
        let prefix = if event.holiday { "◇" } else { "●" };
        let font = (height * 0.7)
            .min(if compact { 11.0 } else { 14.0 })
            .max(0.6);
        let characters = ((width - 6.0).max(1.0) / (font * 0.6)).max(4.0) as usize;
        let label = if compact {
            format!("{prefix} {}", event.title)
        } else {
            format!("{prefix} {}  {}", event.short_time(), event.title)
        };
        let color = parse_color(&event.color);
        let chip = button::custom(
            text(shorten(&label, characters))
                .size(font)
                .line_height(1.0)
                .class(cosmic::theme::Text::Color(color)),
        )
        .class(cosmic::theme::Button::Text)
        .padding([0, 2])
        .height(height)
        .width(Length::Fill)
        .on_press(Message::Open(event.url.clone()));
        tooltip(
            chip,
            container(text(event.details()).size(13))
                .max_width(420)
                .padding(6),
            tooltip::Position::FollowCursor,
        )
        .into()
    }

    fn calendar_view(&self, width: f32, height: f32) -> Element<'_, Message> {
        let today = Local::now().date_naive();
        let start = calendar::week_start(today, self.panel_config.first_day_of_week);
        let end = start + chrono::Duration::days(13);
        let cell_height = (height * 0.235).clamp(30.0, 148.0);
        let cell_width = ((width - 30.0) / 7.0).max(10.0);
        let mut view = column![
            row![
                column![
                    text::title4("Calendar"),
                    text::caption(format!(
                        "{} – {} · This week & next week",
                        start.format("%d %b"),
                        end.format("%d %b %Y")
                    ))
                ]
                .spacing(3),
                widget::space::horizontal(),
                button::text("Google Calendar ↗")
                    .padding([4, 6])
                    .on_press(Message::Open(
                        "https://calendar.google.com/calendar/u/0/r".into()
                    )),
            ]
            .align_y(Alignment::Center)
        ]
        .spacing(6);
        let mut weekdays = widget::Row::new().spacing(5);
        for index in 0..7 {
            weekdays = weekdays.push(
                container(text::caption(
                    (start + chrono::Duration::days(index))
                        .format("%a")
                        .to_string(),
                ))
                .center_x(Length::Fill),
            );
        }
        view = view.push(weekdays);
        for week in 0..2 {
            let mut days = widget::Row::new().spacing(5);
            for weekday in 0..7 {
                let day = start + chrono::Duration::days(week * 7 + weekday);
                let events: Vec<_> = self
                    .calendar
                    .events
                    .iter()
                    .filter(|e| {
                        e.on_day(day) && !self.settings.disabled_calendars.contains(&e.calendar_id)
                    })
                    .collect();
                let holiday = events.iter().any(|event| event.holiday);
                let weekend = matches!(day.weekday(), Weekday::Sat | Weekday::Sun);
                let label = if day.day() == 1 || day == start {
                    day.format("%d %b").to_string()
                } else {
                    day.day().to_string()
                };
                let date_button = button::custom(text(label).size(13))
                    .padding([3, 8])
                    .class(if day == today {
                        cosmic::theme::Button::Suggested
                    } else if day == self.selected_day {
                        cosmic::theme::Button::Standard
                    } else {
                        cosmic::theme::Button::Text
                    })
                    .on_press(Message::SelectDay(day));
                let mut cell = column![date_button].spacing(0).width(Length::Fill);
                let event_height =
                    ((cell_height - 38.0).max(1.0) / events.len().max(1) as f32).min(21.0);
                for event in events {
                    cell = cell.push(self.event_chip(event, true, cell_width - 8.0, event_height));
                }
                days = days.push(
                    container(cell)
                        .padding(4)
                        .width(Length::Fill)
                        .height(cell_height)
                        .class(if holiday || weekend {
                            cosmic::theme::Container::custom(highlighted_day_style)
                        } else {
                            cosmic::theme::Container::Card
                        }),
                );
            }
            view = view.push(days);
        }
        if self.settings.google_email.is_empty() {
            view = view.push(
                row![
                    text::caption(
                        "Connect Google to show your events and subscribed holiday calendars."
                    )
                    .width(Length::Fill),
                    button::suggested("Connect in Settings").on_press(Message::Settings(true))
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );
        } else {
            let mut status = if self.calendar_busy {
                "Fetching Google Calendar…".into()
            } else if let Some(error) = &self.calendar_error {
                format!("Calendar refresh failed · {error}")
            } else if let Some(updated) = self.calendar.fetched_at {
                format!(
                    "{} · Updated {} · Refreshes every 10 minutes · Times are local",
                    self.settings.google_email,
                    local_time(updated)
                )
            } else {
                "Waiting for Google Calendar…".into()
            };
            if !self.calendar.warnings.is_empty() {
                status = format!(
                    "Partial data · {} refresh warnings\n{status}\n{}",
                    self.calendar.warnings.len(),
                    self.calendar.warnings.join("\n")
                );
            }
            view = view.push(status_line(status, (width / 6.0).max(12.0) as usize));
            let sources: Vec<_> = self
                .calendar
                .calendars
                .iter()
                .filter(|source| !self.settings.disabled_calendars.contains(&source.id))
                .collect();
            let legend_rows = sources.len().div_ceil(3).max(1);
            let legend_font = (30.0 / legend_rows as f32).clamp(0.1, 11.0);
            let mut legends = widget::Column::new().spacing(0);
            for sources in sources.chunks(3) {
                let mut legend = widget::Row::new().spacing(12);
                for source in sources {
                    legend = legend.push(tooltip(
                        text(format!(
                            "{} {}",
                            if source.holiday { "◇" } else { "●" },
                            shorten(
                                &source.name,
                                ((width / 3.0 - 16.0) / (legend_font * 0.6)).max(4.0) as usize
                            )
                        ))
                        .size(legend_font)
                        .line_height(1.0)
                        .class(cosmic::theme::Text::Color(parse_color(&source.color)))
                        .width(Length::Fill),
                        text(source.name.clone()),
                        tooltip::Position::Top,
                    ));
                }
                legends = legends.push(legend);
            }
            view = view.push(legends);
            view = view.push(
                container(self.agenda_view())
                    .padding(8)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .class(cosmic::theme::Container::Primary),
            );
        }
        view.height(Length::Fill).into()
    }

    fn agenda_view(&self) -> Element<'_, Message> {
        widget::responsive(move |size| {
            let events: Vec<_> = self
                .calendar
                .events
                .iter()
                .filter(|event| {
                    event.on_day(self.selected_day)
                        && !self
                            .settings
                            .disabled_calendars
                            .contains(&event.calendar_id)
                })
                .collect();
            let slot_height = (size.height - 26.0).max(1.0) / events.len().max(1) as f32;
            let gap = (slot_height * 0.08).min(2.0);
            let mut agenda = column![text::body(format!(
                "{} · {} events",
                self.selected_day.format("%A %d %B"),
                events.len()
            ))]
            .spacing(gap);
            if events.is_empty() {
                agenda = agenda.push(text::caption(if self.calendar_busy {
                    "Loading events…"
                } else if self.calendar_error.is_some() || !self.calendar.warnings.is_empty() {
                    "No events in the currently available data."
                } else {
                    "No events for this day."
                }));
            }
            let row_height = (slot_height - gap).clamp(0.1, 30.0);
            for event in events {
                agenda = agenda.push(self.event_chip(event, false, size.width, row_height));
            }
            agenda.into()
        })
        .into()
    }

    fn quote_row<'a>(&'a self, symbol: &'a str, width: f32, height: f32) -> Element<'a, Message> {
        let scale = (height / 40.0).min(width / 680.0).clamp(0.025, 1.0);
        let (flags, description): (Element<'_, Message>, String) = match symbol {
            "BRN1" => (
                text("◈").size(19.0 * scale).into(),
                "Brent crude · Front-month futures · USD/barrel".into(),
            ),
            "GOLD" => (
                text("◆").size(19.0 * scale).into(),
                "Spot gold · XAU/USD · USD/troy ounce".into(),
            ),
            _ if symbol.len() == 6 && symbol.is_ascii() => {
                let (base, base_flag) = market::currency(&symbol[..3]);
                let (quote, quote_flag) = market::currency(&symbol[3..]);
                (
                    paired_flags(base_flag, quote_flag, scale),
                    format!("{base} / {quote}"),
                )
            }
            _ => (crate::flags::view("", 26.0 * scale), symbol.to_owned()),
        };
        let title = column![
            text(symbol).size(14.0 * scale).line_height(1.0),
            text(description).size(11.0 * scale).line_height(1.0)
        ]
        .spacing(2.0 * scale)
        .width(Length::Fill);
        let mut line = row![container(flags).width(44.0 * scale).center_y(height), title]
            .align_y(Alignment::Center)
            .spacing(8.0 * scale);
        if let Some(quote) = self.quotes.get(symbol) {
            let price = format!("{:.*}", quote.decimals, quote.price);
            let price_view: Element<_> = if quote.decimals >= 5 {
                let (main, fractional) = price.split_at(price.len() - 1);
                row![
                    text(main.to_owned()).size(18.0 * scale).line_height(1.0),
                    text(fractional.to_owned())
                        .size(11.0 * scale)
                        .line_height(1.0)
                ]
                .align_y(Alignment::Start)
                .into()
            } else {
                text(price).size(18.0 * scale).line_height(1.0).into()
            };
            let change = match (quote.change, quote.percent) {
                (Some(change), Some(percent)) => {
                    format!("{change:+.*}\n{percent:+.2}%", quote.decimals)
                }
                _ => "Change\nunavailable".into(),
            };
            let delta = quote.change.unwrap_or(0.0);
            let change = text(change).size(12.0 * scale).line_height(1.15).class(
                cosmic::theme::Text::Color(if delta > 0.0 {
                    self.core.system_theme().cosmic().success.base.into()
                } else if delta < 0.0 {
                    self.core.system_theme().cosmic().destructive.base.into()
                } else {
                    self.core
                        .system_theme()
                        .cosmic()
                        .background(false)
                        .on
                        .into()
                }),
            );
            let unavailable =
                self.unavailable.iter().any(|s| s == symbol) || self.market_error.is_some();
            let status = if unavailable || quote.stale(Utc::now()) {
                "Stale · last known".into()
            } else if quote.old_bar(Utc::now()) {
                "Closed / old quote".into()
            } else {
                quote.latency()
            };
            let details = format!(
                "TradingView · {}\n{}\nRetrieved: {}\n{}\nDaily changes are against the provider's previous session close.",
                market::ticker(symbol),
                quote.latency(),
                quote
                    .fetched_at
                    .with_timezone(&Local)
                    .format("%d %b %H:%M:%S %Z"),
                quote
                    .bar_time
                    .map_or("Quote timestamp unavailable".into(), |time| format!(
                        "Last one-minute bar: {} (not a trade timestamp)",
                        time.with_timezone(&Local).format("%d %b %H:%M %Z")
                    ))
            );
            line = line
                .push(tooltip(
                    column![price_view, text(status).size(10.0 * scale).line_height(1.0)]
                        .align_x(Alignment::End)
                        .spacing(2.0 * scale)
                        .width(126.0 * scale),
                    container(text(details).size(12)).max_width(420),
                    tooltip::Position::Left,
                ))
                .push(container(change).align_right(96.0 * scale));
        } else {
            line = line
                .push(
                    column![
                        text("—").size(18.0 * scale),
                        text(if self.market_busy {
                            "Loading…"
                        } else {
                            "Unavailable"
                        })
                        .size(10.0 * scale)
                    ]
                    .align_x(Alignment::End)
                    .width(126.0 * scale),
                )
                .push(container(text("—").size(12.0 * scale)).align_right(96.0 * scale));
        }
        container(line)
            .padding([0, 2])
            .height(height)
            .width(Length::Fill)
            .into()
    }

    fn markets_view(&self, width: f32, height: f32) -> Element<'_, Message> {
        let mut view = column![
            row![
                text::title4("Forex"),
                widget::space::horizontal(),
                text::caption(if self.market_busy {
                    "Refreshing…"
                } else {
                    "Price · Daily Δ / %"
                })
            ]
            .align_y(Alignment::Center)
        ]
        .spacing(5);
        if let Some(error) = &self.market_error {
            view = view.push(status_line(
                format!("Refresh failed · {error}"),
                (width / 6.0).max(12.0) as usize,
            ));
        }
        let columns = self.settings.forex.len().div_ceil(18).clamp(1, 3);
        let rows = self.settings.forex.len().div_ceil(columns).max(1);
        let grid_height = (height
            - 166.0
            - if self.market_error.is_some() {
                20.0
            } else {
                0.0
            })
        .max(10.0);
        let row_height =
            ((grid_height - rows.saturating_sub(1) as f32) / rows as f32).clamp(0.8, 44.0);
        let column_width = (width - 10.0 * (columns - 1) as f32) / columns as f32;
        let mut quotes = widget::Row::new().spacing(10).height(grid_height);
        for group in self.settings.forex.chunks(rows) {
            let mut list = widget::Column::new().width(Length::Fill);
            for (index, pair) in group.iter().enumerate() {
                if index > 0 {
                    list = list.push(divider::horizontal::light());
                }
                list = list.push(self.quote_row(pair, column_width, row_height));
            }
            quotes = quotes.push(list);
        }
        if self.settings.forex.is_empty() {
            quotes = quotes.push(text::caption("Add currency pairs in Settings."));
        }
        view = view
            .push(quotes)
            .push(
                text("TradingView · 10-minute refresh · Hover prices for source and timing")
                    .size(11)
                    .line_height(1.0),
            )
            .push(divider::horizontal::default())
            .push(text::title4("Commodities"))
            .push(
                row![self.commodity_tile("BRN1"), self.commodity_tile("GOLD")]
                    .spacing(10)
                    .height(78),
            );
        view.height(Length::Fill).into()
    }

    fn commodity_tile(&self, symbol: &'static str) -> Element<'_, Message> {
        let description = if symbol == "BRN1" {
            "Brent front month · USD/barrel"
        } else {
            "Spot gold · USD/troy ounce"
        };
        let mut content = column![].spacing(3);
        if let Some(quote) = self.quotes.get(symbol) {
            let price = format!("{:.*}", quote.decimals, quote.price);
            let changes = match (quote.change, quote.percent) {
                (Some(change), Some(percent)) => {
                    format!("{change:+.*}  {percent:+.2}%", quote.decimals)
                }
                _ => "Change unavailable".into(),
            };
            let color = if quote.change.unwrap_or_default() < 0.0 {
                self.core.system_theme().cosmic().destructive.base
            } else if quote.change.unwrap_or_default() > 0.0 {
                self.core.system_theme().cosmic().success.base
            } else {
                self.core.system_theme().cosmic().background(false).on
            };
            let status = if self.market_error.is_some()
                || self.unavailable.iter().any(|s| s == symbol)
                || quote.stale(Utc::now())
            {
                "Stale · last known".into()
            } else if quote.old_bar(Utc::now()) {
                "Closed / old quote".into()
            } else {
                quote.latency()
            };
            content = content
                .push(
                    row![
                        text(symbol).size(14),
                        widget::space::horizontal(),
                        text(price).size(21).line_height(1.0)
                    ]
                    .align_y(Alignment::Center),
                )
                .push(text(description).size(10).line_height(1.0))
                .push(
                    row![
                        text(status).size(10),
                        widget::space::horizontal(),
                        text(changes)
                            .size(12)
                            .class(cosmic::theme::Text::Color(color.into()))
                    ]
                    .align_y(Alignment::Center),
                );
            let details = format!(
                "TradingView · {}\n{}\n{}\nRetrieved: {}\n{}",
                market::ticker(symbol),
                description,
                quote.latency(),
                quote
                    .fetched_at
                    .with_timezone(&Local)
                    .format("%d %b %H:%M:%S %Z"),
                quote
                    .bar_time
                    .map_or("Quote timestamp unavailable".into(), |time| format!(
                        "Last one-minute bar: {}",
                        time.with_timezone(&Local).format("%d %b %H:%M %Z")
                    ))
            );
            tooltip(
                container(content)
                    .padding(8)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .class(cosmic::theme::Container::Primary),
                container(text(details).size(12)).max_width(420),
                tooltip::Position::Top,
            )
            .into()
        } else {
            content = content
                .push(row![
                    text(symbol).size(14),
                    widget::space::horizontal(),
                    text("—").size(21)
                ])
                .push(text(description).size(10))
                .push(
                    text(if self.market_busy {
                        "Loading…"
                    } else {
                        "Unavailable"
                    })
                    .size(11),
                );
            container(content)
                .padding(8)
                .width(Length::Fill)
                .height(Length::Fill)
                .class(cosmic::theme::Container::Primary)
                .into()
        }
    }

    fn ip_view(&self, height: f32) -> Element<'_, Message> {
        let scale = (height / 184.0).clamp(0.3, 1.0);
        let mut view = column![
            row![
                text::title4("Public IP"),
                widget::space::horizontal(),
                text::caption(if self.ip_busy {
                    "Checking…"
                } else {
                    "Checks every minute"
                })
            ]
            .align_y(Alignment::Center)
        ]
        .spacing(6);
        if let Some(ip) = &self.ip {
            view = view.push(
                container(
                    row![
                        crate::flags::view(&ip.country_code, 96.0 * scale),
                        column![
                            text(&ip.ip)
                                .size(if ip.ip.len() > 20 {
                                    24.0 * scale
                                } else {
                                    34.0 * scale
                                })
                                .line_height(1.05),
                            text(format!("{}, {}", ip.city, ip.country))
                                .size(18.0 * scale)
                                .line_height(1.1)
                        ]
                        .spacing(6)
                        .width(Length::Fill),
                    ]
                    .spacing(18)
                    .align_y(Alignment::Center),
                )
                .center_y(Length::Fill),
            );
            view = view.push(
                text(format!(
                    "{} · Updated {}{}",
                    ip.source,
                    local_time(ip.fetched_at),
                    if self.ip_error.is_some() || (Utc::now() - ip.fetched_at).num_seconds() > 120 {
                        " · Stale"
                    } else {
                        ""
                    }
                ))
                .size(11)
                .line_height(1.0),
            );
        } else {
            view = view.push(
                container(
                    text(if self.ip_busy {
                        "Looking up this computer’s public internet address…"
                    } else {
                        "Public IP unavailable"
                    })
                    .size(18.0 * scale),
                )
                .center_y(Length::Fill),
            );
        }
        if let Some(error) = &self.ip_error {
            view = view.push(status_line(error.clone(), 70));
        }
        view.push(tooltip(text("Internet egress · Includes VPN / proxy routing").size(11).line_height(1.0),
            container(text("Shows the public address seen by the service, including any VPN or configured proxy. Country and city come from IP geolocation.")).max_width(420), tooltip::Position::Bottom))
        .height(Length::Fill)
        .into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let banner_changed = self.banner_input != self.settings.banner_message;
        let mut view = column![
            text::title4("Dashboard banner"),
            text::caption("Your message across the top of the dashboard. Save or press Enter to apply; Cancel restores the saved message."),
            row![
                text_input(config::DEFAULT_BANNER_MESSAGE, &self.banner_input)
                    .on_input(Message::BannerInput)
                    .on_submit(|_| Message::SaveBanner)
                    .width(Length::Fill),
                button::suggested("Save")
                    .on_press_maybe(banner_changed.then_some(Message::SaveBanner)),
                button::standard("Cancel")
                    .on_press_maybe(banner_changed.then_some(Message::CancelBanner)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text::caption(format!(
                "{} / {} characters · One line · Longer messages shrink to fit",
                self.banner_input.chars().count(),
                config::MAX_BANNER_CHARACTERS,
            )),
            divider::horizontal::default(),
            text::title4("World clocks"),
            text::caption("Analog hour and minute hands. IANA time zones automatically follow daylight saving. Edit a clock's name or time zone, then Save or Cancel."),
        ]
        .spacing(10);
        for (index, clock) in self.settings.clocks.iter().enumerate() {
            if let Some(edit) = self.clock_edit.as_ref().filter(|edit| edit.index == index) {
                view = view.push(
                    row![
                        text_input("Clock name", &edit.label)
                            .id(CLOCK_EDIT_LABEL.clone())
                            .on_input(Message::EditClockLabel)
                            .on_submit(|_| Message::SaveClock)
                            .width(Length::FillPortion(1)),
                        text_input("IANA time zone, e.g. Europe/London", &edit.timezone)
                            .on_input(Message::EditClockZone)
                            .on_submit(|_| Message::SaveClock)
                            .width(Length::FillPortion(2)),
                        button::suggested("Save").on_press(Message::SaveClock),
                        button::standard("Cancel").on_press(Message::CancelClockEdit),
                    ]
                    .align_y(Alignment::Center)
                    .spacing(8),
                );
                continue;
            }
            view = view.push(
                row![
                    text::body(&clock.label).width(150),
                    text::caption(&clock.timezone).width(Length::Fill),
                    button::standard("Edit").on_press_maybe(
                        self.clock_edit
                            .is_none()
                            .then_some(Message::EditClock(index))
                    ),
                    button::text("Remove").on_press_maybe(
                        self.clock_edit
                            .is_none()
                            .then_some(Message::RemoveClock(index))
                    )
                ]
                .align_y(Alignment::Center)
                .spacing(12),
            );
        }
        view = view.push(
            row![
                text_input("Label, e.g. London", &self.clock_label)
                    .on_input(Message::ClockLabel)
                    .width(Length::FillPortion(1)),
                text_input("Time zone, e.g. Europe/London", &self.clock_zone)
                    .on_input(Message::ClockZone)
                    .width(Length::FillPortion(2)),
                button::suggested("Add clock").on_press(Message::AddClock)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
        view = view
            .push(divider::horizontal::default())
            .push(text::title4("Forex watchlist"));
        view = view.push(text::caption("Pairs use base/quote order. CNH is offshore yuan; CNY is onshore yuan. Unsupported pairs are shown as unavailable."));
        for (index, pair) in self.settings.forex.iter().enumerate() {
            view = view.push(
                row![
                    text(pair).width(Length::Fill),
                    button::text("Remove").on_press(Message::RemovePair(index))
                ]
                .align_y(Alignment::Center),
            );
        }
        view = view.push(
            row![
                text_input("Add a pair, e.g. EUR/USD", &self.pair_input)
                    .on_input(Message::PairInput)
                    .on_submit(|_| Message::AddPair),
                button::suggested("Add pair").on_press(Message::AddPair)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
        view = view
            .push(divider::horizontal::default())
            .push(text::title4("Google Calendar"));
        view = view.push(text::caption("One-time setup: 1. Enable Google Calendar API in Google Cloud. 2. Create an OAuth client of type Desktop app and download its JSON. 3. Choose that JSON below, then sign in. Calendar access is read-only; your refresh token is stored in the desktop keyring."));
        view = view.push(
            button::standard("1. Set up Google OAuth in browser ↗").on_press(Message::Open(
                "https://console.cloud.google.com/apis/credentials".into(),
            )),
        );
        view = view.push(
            row![
                text_input("Path to downloaded OAuth client JSON", &self.client_path)
                    .on_input(Message::ClientPath),
                button::standard(if self.choosing_client {
                    "Choosing…"
                } else {
                    "2. Choose JSON…"
                })
                .on_press_maybe(
                    (!self.choosing_client && !self.auth_busy).then_some(Message::BrowseClient)
                )
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
        if self.client_path.trim().is_empty() {
            view = view.push(text::caption("Choose the downloaded Desktop app client JSON to enable browser sign-in. A Google email alone cannot authorize Calendar access."));
        }
        view = view.push(
            text_input("Google account email (optional sign-in hint)", &self.email)
                .on_input(Message::Email),
        );
        let mut connect = row![
            button::suggested(if self.auth_busy {
                "Waiting for browser / keyring…"
            } else {
                "3. Sign in with Google"
            })
            .on_press_maybe(
                (!self.auth_busy && !self.choosing_client && !self.client_path.trim().is_empty())
                    .then_some(Message::Connect)
            )
        ]
        .spacing(8);
        if !self.settings.google_email.is_empty() {
            connect = connect.push(
                button::standard("Disconnect")
                    .on_press_maybe((!self.auth_busy).then_some(Message::Disconnect)),
            );
            view = view.push(text::body(format!(
                "Connected: {}",
                self.settings.google_email
            )));
        }
        view = view.push(connect);
        view = view.push(text::caption("For an OAuth app in Testing, add your email as a test user. Google may expire its refresh token after seven days; reconnect when requested."));
        if !self.calendar.calendars.is_empty() {
            view = view.push(text::title4("Visible calendars"));
            for source in &self.calendar.calendars {
                let id = source.id.clone();
                view = view.push(
                    widget::checkbox(!self.settings.disabled_calendars.contains(&source.id))
                        .label(format!(
                            "{} {}",
                            if source.holiday {
                                "◇ Holiday ·"
                            } else {
                                "●"
                            },
                            source.name
                        ))
                        .on_toggle(move |enabled| Message::ToggleCalendar(id.clone(), enabled)),
                );
            }
        }
        view = view.push(text::caption("Subscribe to country holiday calendars in Google Calendar → Settings → Add calendar → Browse calendars of interest. Each appears here after the next refresh."));
        view = view
            .push(divider::horizontal::default())
            .push(text::title4("Panel & refresh"));
        view = view.push(text::caption("The panel label follows COSMIC Date, Time & Calendar settings and the OS clock. The two-week calendar follows COSMIC’s first day of week. Click the panel clock to open or close the dashboard. Click outside or press Escape to dismiss it. The dashboard opens automatically once per login, after the panel is ready. Public time syncs hourly; Google Calendar and markets refresh every 10 minutes; public IP every minute."));
        view.push(text::caption(format!(
            "Settings: {}",
            config::config_dir().join("settings.json").display()
        )))
        .into()
    }
}

impl cosmic::Application for FinanceDashboard {
    type Executor = cosmic::executor::Default;
    type Flags = (Settings, bool);
    type Message = Message;
    const APP_ID: &'static str = cosmic_applet_finance_dashboard::APP_ID;

    fn core(&self) -> &app::Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn init(mut core: app::Core, (settings, preview): Self::Flags) -> (Self, app::Task<Message>) {
        core.window.show_headerbar = preview;
        let today = Local::now().date_naive();
        let panel_config = PanelClock::load();
        let mut app = Self {
            core,
            preview,
            popup: None,
            backdrop: None,
            outputs: Vec::new(),
            login_popup_pending: !preview,
            rectangle: Rectangle::default(),
            rectangle_tracker: None,
            client_path: settings.google_client_path.clone(),
            email: settings.google_email.clone(),
            banner_input: settings.banner_message.clone(),
            settings,
            settings_open: false,
            calendar_start: calendar::week_start(today, panel_config.first_day_of_week),
            panel_config,
            locale: crate::panel_time::locale(),
            panel_now: jiff::Zoned::now(),
            world_now: Utc::now(),
            os_sample: Instant::now(),
            os_time: Utc::now(),
            client: network::client(),
            time: None,
            time_error: None,
            time_busy: false,
            time_attempt: None,
            quotes: BTreeMap::new(),
            unavailable: Vec::new(),
            market_error: None,
            market_busy: false,
            market_attempt: None,
            ip: None,
            ip_error: None,
            ip_busy: false,
            ip_attempt: None,
            calendar: CalendarSnapshot::default(),
            calendar_error: None,
            calendar_busy: false,
            calendar_attempt: None,
            calendar_generation: 0,
            calendar_day: today,
            selected_day: today,
            clock_label: String::new(),
            clock_zone: String::new(),
            clock_edit: None,
            pair_input: String::new(),
            auth_busy: false,
            choosing_client: false,
            notice: None,
        };
        let tasks = app.refresh_due();
        (app, tasks)
    }

    fn style(&self) -> Option<iced::theme::Style> {
        if self.preview {
            None
        } else {
            Some(cosmic::applet::style())
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            rectangle_tracker_subscription(0).map(|event| Message::Rectangle(event.1)),
            iced::event::listen_with(input_event),
        ])
    }

    fn update(&mut self, message: Message) -> app::Task<Message> {
        // Respect any manual opening/dismissal before the startup tick. Mark this
        // session handled so a later panel restart won't reopen the dashboard.
        if matches!(
            &message,
            Message::TogglePopup | Message::Close | Message::Settings(_)
        ) {
            self.consume_login_popup();
        }
        match message {
            Message::Tick => {
                self.panel_now = jiff::Zoned::now();
                let today = Local::now().date_naive();
                if self.os_sample.elapsed() >= Duration::from_secs(30) {
                    let now = Utc::now();
                    let elapsed = self.os_sample.elapsed().as_millis() as i64;
                    if ((now - self.os_time).num_milliseconds() - elapsed).abs() > 2000 {
                        self.time = None;
                        self.time_attempt = None;
                    }
                    self.world_now = now
                        + chrono::Duration::milliseconds(
                            self.time.as_ref().map_or(0, |s| s.offset_ms),
                        );
                    self.os_time = now;
                    self.os_sample = Instant::now();
                    let config = PanelClock::load();
                    if self.panel_config.first_day_of_week != config.first_day_of_week {
                        self.invalidate_calendar();
                    }
                    self.panel_config = config;
                }
                let start = calendar::week_start(today, self.panel_config.first_day_of_week);
                if today != self.calendar_day || start != self.calendar_start {
                    self.calendar_start = start;
                    self.calendar_day = today;
                    self.selected_day = today;
                    self.invalidate_calendar();
                }
                return Task::batch([self.open_login_popup_if_ready(), self.refresh_due()]);
            }
            Message::SurfacePressed(id) => {
                if self.backdrop == Some(id) {
                    return self.close_popup();
                }
            }
            Message::Output(output, name) => {
                self.outputs.retain(|(old, _)| *old != output);
                self.outputs.push((output, name));
            }
            Message::OutputRemoved(output) => {
                self.outputs.retain(|(old, _)| *old != output);
                return self.close_popup();
            }
            Message::TogglePopup => {
                if self.popup.is_some() {
                    return self.close_popup();
                }
                return self.open_popup();
            }
            Message::Close => {
                if self.preview {
                    self.settings_open = false;
                } else {
                    return self.close_popup();
                }
            }
            Message::Closed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                    return self.close_popup();
                }
                if self.backdrop == Some(id) {
                    self.backdrop = None;
                    return self.close_popup();
                }
            }
            Message::Rectangle(update) => match update {
                RectangleUpdate::Rectangle((_, rectangle)) => self.rectangle = rectangle,
                RectangleUpdate::Init(tracker) => self.rectangle_tracker = Some(tracker),
            },
            Message::Settings(open) => {
                self.settings_open = open;
                return Task::batch([
                    self.open_popup(),
                    iced::widget::operation::snap_to(
                        BODY_SCROLL.clone(),
                        iced::widget::operation::RelativeOffset::START,
                    ),
                ]);
            }
            Message::Refresh => {
                self.time_attempt = None;
                self.market_attempt = None;
                self.ip_attempt = None;
                self.calendar_attempt = None;
                return self.refresh_due();
            }
            Message::Time(result) => {
                self.time_busy = false;
                match result {
                    Ok(sync) => {
                        self.world_now =
                            Utc::now() + chrono::Duration::milliseconds(sync.offset_ms);
                        self.time = Some(sync);
                        self.time_error = None;
                    }
                    Err(error) => {
                        self.time = None;
                        self.world_now = Utc::now();
                        self.time_error = Some(error);
                    }
                }
            }
            Message::Markets(result) => {
                self.market_busy = false;
                match result {
                    Ok(snapshot) => {
                        self.quotes.extend(snapshot.quotes);
                        self.unavailable = snapshot.unavailable;
                        self.market_error = None;
                    }
                    Err(error) => self.market_error = Some(error),
                }
            }
            Message::Ip(result) => {
                self.ip_busy = false;
                match result {
                    Ok(ip) => {
                        self.ip = Some(ip);
                        self.ip_error = None;
                    }
                    Err(error) => self.ip_error = Some(error),
                }
            }
            Message::Calendar(generation, result) if generation == self.calendar_generation => {
                self.calendar_busy = false;
                match result {
                    Ok(mut snapshot) => {
                        snapshot.events.extend(
                            self.calendar
                                .events
                                .iter()
                                .filter(|e| snapshot.failed_calendars.contains(&e.calendar_id))
                                .cloned(),
                        );
                        self.calendar = snapshot;
                        self.calendar_error = None;
                    }
                    Err(error) => self.calendar_error = Some(error),
                }
            }
            Message::Calendar(_, _) => {}
            Message::BannerInput(value) => {
                self.banner_input = value;
                self.notice = None;
            }
            Message::SaveBanner => {
                let mut settings = self.settings.clone();
                match settings.update_banner(&self.banner_input) {
                    Ok(()) => {
                        if self.save(settings) {
                            self.banner_input.clone_from(&self.settings.banner_message);
                        }
                    }
                    Err(error) => self.notice = Some(error.to_string()),
                }
            }
            Message::CancelBanner => {
                self.banner_input.clone_from(&self.settings.banner_message);
                self.notice = None;
            }
            Message::ClockLabel(value) => self.clock_label = value,
            Message::ClockZone(value) => self.clock_zone = value,
            Message::AddClock => {
                if let Err(error) =
                    config::validate_clock(self.clock_label.trim(), self.clock_zone.trim())
                {
                    self.notice = Some(error.to_string());
                } else {
                    let mut settings = self.settings.clone();
                    settings.clocks.push(Clock {
                        label: self.clock_label.trim().into(),
                        timezone: self.clock_zone.trim().into(),
                    });
                    if self.save(settings) {
                        self.clock_label.clear();
                        self.clock_zone.clear();
                    }
                }
            }
            Message::RemoveClock(index) if self.clock_edit.is_none() => {
                let mut settings = self.settings.clone();
                if index < settings.clocks.len() {
                    settings.clocks.remove(index);
                    self.save(settings);
                }
            }
            Message::EditClock(index) if self.clock_edit.is_none() => {
                if let Some(clock) = self.settings.clocks.get(index) {
                    self.clock_edit = Some(ClockEdit {
                        index,
                        label: clock.label.clone(),
                        timezone: clock.timezone.clone(),
                    });
                    self.notice = None;
                    return iced::widget::operation::focus(CLOCK_EDIT_LABEL.clone());
                }
            }
            // Finish the current edit first, so removals cannot shift its index
            // and switching rows cannot silently discard an unsaved draft.
            Message::RemoveClock(_) | Message::EditClock(_) => {}
            Message::EditClockLabel(value) => {
                if let Some(edit) = &mut self.clock_edit {
                    edit.label = value;
                    self.notice = None;
                }
            }
            Message::EditClockZone(value) => {
                if let Some(edit) = &mut self.clock_edit {
                    edit.timezone = value;
                    self.notice = None;
                }
            }
            Message::SaveClock => {
                if let Some(edit) = &self.clock_edit {
                    let mut settings = self.settings.clone();
                    match settings.update_clock(edit.index, &edit.label, &edit.timezone) {
                        Ok(()) => {
                            if self.save(settings) {
                                self.clock_edit = None;
                            }
                        }
                        Err(error) => self.notice = Some(error.to_string()),
                    }
                }
            }
            Message::CancelClockEdit => {
                self.clock_edit = None;
                self.notice = None;
            }
            Message::PairInput(value) => self.pair_input = value,
            Message::AddPair => match config::normalize_pair(&self.pair_input) {
                Ok(pair) => {
                    if self.settings.forex.contains(&pair) {
                        self.notice = Some("This pair is already in your watchlist.".into());
                    } else {
                        let mut settings = self.settings.clone();
                        settings.forex.push(pair);
                        if self.save(settings) {
                            self.pair_input.clear();
                            self.market_attempt = None;
                            return self.refresh_due();
                        }
                    }
                }
                Err(error) => self.notice = Some(error.to_string()),
            },
            Message::RemovePair(index) => {
                let mut settings = self.settings.clone();
                if index < settings.forex.len() {
                    settings.forex.remove(index);
                    self.save(settings);
                }
            }
            Message::ClientPath(value) => {
                self.client_path = value;
                self.notice = None;
            }
            Message::BrowseClient if !self.choosing_client && !self.auth_busy => {
                self.choosing_client = true;
                // Explicitly hand off to external UI instead of relying on
                // keyboard-focus loss, which can also happen on pointer motion.
                return self.close_popup().chain(job(
                    async {
                        use cosmic::dialog::file_chooser;
                        let dialog = file_chooser::open::Dialog::new()
                            .title("Choose Google OAuth Desktop app client JSON")
                            .filter(file_chooser::FileFilter::new("JSON files").glob("*.json"));
                        match dialog.open_file().await {
                            Ok(response) => {
                                let path = response
                                    .url()
                                    .to_file_path()
                                    .map_err(|_| anyhow::anyhow!("Choose a local JSON file"))?;
                                Ok(Some(path.to_string_lossy().into_owned()))
                            }
                            Err(file_chooser::Error::Cancelled) => Ok(None),
                            Err(error) => Err(error.into()),
                        }
                    },
                    Message::ClientChosen,
                ));
            }
            Message::BrowseClient => {}
            Message::ClientChosen(result) => {
                self.choosing_client = false;
                match result {
                    Ok(Some(path)) => {
                        self.client_path = path;
                        self.notice = match oauth::credentials(&self.client_path) {
                            Ok(_) => Some("OAuth client selected. Click Sign in with Google to open your browser.".into()),
                            Err(error) => Some(error.to_string()),
                        };
                    }
                    Ok(None) => {}
                    Err(error) => self.notice = Some(error),
                }
            }
            Message::Email(value) => self.email = value,
            Message::Connect if !self.auth_busy => {
                if let Err(error) = oauth::credentials(&self.client_path) {
                    self.notice = Some(format!("{error:#}"));
                    return Task::none();
                }
                self.auth_busy = true;
                self.notice = Some(
                    "Complete Google sign-in in your browser. This may take up to three minutes."
                        .into(),
                );
                let client = self.client.clone();
                let path = self.client_path.clone();
                let email = self.email.clone();
                return self.close_popup().chain(job(
                    async move { oauth::sign_in(&client, &path, &email).await },
                    Message::Connected,
                ));
            }
            Message::Connected(result) => {
                self.auth_busy = false;
                match result {
                    Ok(email) => {
                        let mut settings = self.settings.clone();
                        settings.google_email = email.clone();
                        settings.google_client_path = config::config_dir()
                            .join("google-client.json")
                            .to_string_lossy()
                            .into_owned();
                        settings.disabled_calendars.clear();
                        if self.save(settings) {
                            self.email = email;
                            self.client_path = self.settings.google_client_path.clone();
                            self.calendar = CalendarSnapshot::default();
                            self.invalidate_calendar();
                            self.notice = Some("Google Calendar connected.".into());
                            return self.refresh_due();
                        }
                    }
                    Err(error) => self.notice = Some(error),
                }
            }
            Message::Disconnect if !self.auth_busy => {
                self.auth_busy = true;
                return job(oauth::disconnect(), Message::Disconnected);
            }
            Message::Disconnected(result) => {
                self.auth_busy = false;
                match result {
                    Ok(()) => {
                        let mut settings = self.settings.clone();
                        settings.google_email.clear();
                        settings.disabled_calendars.clear();
                        if self.save(settings) {
                            self.email.clear();
                            self.invalidate_calendar();
                            self.calendar = CalendarSnapshot::default();
                            self.calendar_error = None;
                            self.notice = Some("Google Calendar disconnected; saved refresh token removed from the keyring.".into());
                        }
                    }
                    Err(error) => self.notice = Some(error),
                }
            }
            Message::Connect | Message::Disconnect => {}
            Message::ToggleCalendar(id, enabled) => {
                let mut settings = self.settings.clone();
                settings.disabled_calendars.retain(|value| value != &id);
                if !enabled {
                    settings.disabled_calendars.push(id);
                }
                if self.save(settings) {
                    self.invalidate_calendar();
                    return self.refresh_due();
                }
            }
            Message::SelectDay(day) => self.selected_day = day,
            Message::Open(url) => {
                return self.close_popup().chain(job(
                    async move { oauth::open_url(&url).await },
                    Message::Opened,
                ));
            }
            Message::Opened(Err(error)) => self.notice = Some(error),
            Message::Opened(Ok(())) => {}
            Message::ClearNotice => self.notice = None,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        if self.preview {
            return self.dashboard();
        }
        let horizontal = self.core.applet.is_horizontal();
        let label: Element<_> = if horizontal {
            row![
                self.core.applet.text(crate::panel_time::horizontal(
                    &self.panel_now,
                    &self.panel_config,
                    &self.locale
                )),
                widget::space::vertical().height(
                    (self.core.applet.suggested_size(true).1
                        + 2 * self.core.applet.suggested_padding(true).1)
                        as f32
                )
            ]
            .align_y(Alignment::Center)
            .into()
        } else {
            let mut lines = widget::Column::new().align_x(Alignment::Center).spacing(4);
            for part in
                crate::panel_time::vertical(&self.panel_now, &self.panel_config, &self.locale)
            {
                if part.is_empty() {
                    lines = lines.push(
                        iced::widget::rule::horizontal(2)
                            .width(self.core.applet.suggested_size(true).0),
                    );
                } else {
                    lines = lines.push(self.core.applet.text(part));
                }
            }
            column![
                lines,
                widget::space::horizontal().width(
                    (self.core.applet.suggested_size(true).0
                        + 2 * self.core.applet.suggested_padding(true).1)
                        as f32
                )
            ]
            .align_x(Alignment::Center)
            .into()
        };
        let button = button::custom(label)
            .class(cosmic::theme::Button::AppletIcon)
            .padding(if horizontal {
                [0, self.core.applet.suggested_padding(true).0]
            } else {
                [self.core.applet.suggested_padding(true).0, 0]
            })
            .on_press_down(Message::TogglePopup);
        let content = button;
        let tracked: Element<_> = if let Some(tracker) = &self.rectangle_tracker {
            tracker.container(0, content).ignore_bounds(true).into()
        } else {
            content.into()
        };
        widget::autosize::autosize(tracked, PANEL_AUTOSIZE.clone()).into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.popup != Some(id) {
            return widget::space()
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }
        let content = self.dashboard();
        self.core
            .applet
            .popup_container(container(content).width(WIDTH).height(HEIGHT))
            .limits(
                Limits::NONE
                    .min_width(480.0)
                    .max_width(WIDTH)
                    .min_height(300.0)
                    .max_height(HEIGHT),
            )
            .into()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::Closed(id))
    }
}

fn local_time(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%d %b %H:%M").to_string()
}

fn highlighted_day_style(theme: &cosmic::Theme) -> container::Style {
    let mut style =
        <cosmic::Theme as container::Catalog>::style(theme, &cosmic::theme::Container::Card);
    if let Some(iced::Background::Color(base)) = style.background {
        let accent: Color = theme.cosmic().accent.base.into();
        let tint = 0.16;
        style.background = Some(
            Color::from_rgba(
                base.r + (accent.r - base.r) * tint,
                base.g + (accent.g - base.g) * tint,
                base.b + (accent.b - base.b) * tint,
                base.a,
            )
            .into(),
        );
    }
    style
}

fn banner(message: &str) -> Element<'_, Message> {
    use iced::advanced::text::{self as native_text, Paragraph};
    type BannerParagraph = <cosmic::Renderer as native_text::Renderer>::Paragraph;

    tooltip(
        widget::responsive(move |size| {
            let paragraph = BannerParagraph::with_text(native_text::Text {
                content: message,
                bounds: iced::Size::new(f32::INFINITY, 52.0),
                size: 35.0.into(),
                line_height: 1.25.into(),
                font: cosmic::font::semibold(),
                align_x: native_text::Alignment::Default,
                align_y: iced::alignment::Vertical::Center,
                shaping: native_text::Shaping::Advanced,
                wrapping: native_text::Wrapping::None,
                ellipsize: native_text::Ellipsize::None,
            });
            let font_size = (35.0 * (size.width - 1.0).max(1.0)
                / paragraph.min_bounds().width.max(1.0))
            .clamp(20.0, 35.0);
            text::title1(message)
                .size(font_size)
                .line_height(1.25)
                .shaping(native_text::Shaping::Advanced)
                .wrapping(native_text::Wrapping::None)
                .ellipsize(native_text::Ellipsize::End(
                    native_text::EllipsizeHeightLimit::Lines(1),
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        })
        .height(52),
        container(text(message)).padding(8).max_width(640),
        tooltip::Position::Bottom,
    )
    .into()
}

fn paired_flags(base: &str, quote: &str, scale: f32) -> Element<'static, Message> {
    let flag = |country: &str| crate::flags::view(country, 26.0 * scale);
    // Explicit positions keep each pair together at every watchlist scale;
    // the quote flag overlaps the lower right of the base flag without wrapping.
    stack![
        container(flag(base))
            .align_left(40.0 * scale)
            .align_top(26.0 * scale),
        container(flag(quote))
            .align_right(40.0 * scale)
            .align_bottom(26.0 * scale),
    ]
    .width(40.0 * scale)
    .height(26.0 * scale)
    .into()
}

fn status_line(value: String, max_characters: usize) -> Element<'static, Message> {
    let first_line = value.lines().next().unwrap_or_default();
    tooltip(
        text(shorten(first_line, max_characters))
            .size(11)
            .line_height(1.0),
        container(text(value).size(12)).max_width(500).padding(4),
        tooltip::Position::Top,
    )
    .into()
}

fn shorten(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        format!("{}…", value.chars().take(max - 1).collect::<String>())
    }
}

fn parse_color(hex: &str) -> Color {
    let value = hex.trim_start_matches('#');
    if value.len() == 6
        && let Ok(rgb) = u32::from_str_radix(value, 16)
    {
        return Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    }
    Color::from_rgb8(122, 162, 247)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::Application;

    fn test_app() -> FinanceDashboard {
        let (mut app, _) =
            FinanceDashboard::init(app::Core::default(), (Settings::default(), false));
        // These tests only exercise update/state; never consume the real login
        // marker or execute the returned networking/Wayland tasks.
        app.login_popup_pending = false;
        app
    }

    #[test]
    fn click_toggles_dashboard_and_settings_retain_the_same_surfaces() {
        let mut app = test_app();
        _ = app.update(Message::TogglePopup);
        let popup = app.popup.unwrap();
        let backdrop = app.backdrop.unwrap();
        for open in [true, false, true] {
            _ = app.update(Message::Settings(open));
            assert_eq!(app.settings_open, open);
            assert_eq!(app.popup, Some(popup));
            assert_eq!(app.backdrop, Some(backdrop));
        }
        _ = app.update(Message::TogglePopup);
        assert!(app.popup.is_none() && app.backdrop.is_none());
        _ = app.update(Message::Tick);
        assert!(app.popup.is_none());
        _ = app.update(Message::TogglePopup);
        assert!(app.popup.is_some() && app.backdrop.is_some());
        _ = app.update(Message::Close);
        assert!(app.popup.is_none() && app.backdrop.is_none());
    }

    #[test]
    fn serial_free_popup_dismisses_outside_and_ignores_stale_surface_events() {
        let mut app = test_app();
        // The login path uses this same operation without a click/input serial.
        _ = app.open_popup();
        let old_popup = app.popup.unwrap();
        let old_backdrop = app.backdrop.unwrap();
        _ = app.update(Message::SurfacePressed(old_popup));
        assert_eq!(app.popup, Some(old_popup));
        _ = app.update(Message::SurfacePressed(old_backdrop));
        assert!(app.popup.is_none() && app.backdrop.is_none());

        _ = app.open_popup();
        let popup = app.popup;
        let backdrop = app.backdrop;
        _ = app.update(Message::SurfacePressed(old_backdrop));
        _ = app.update(Message::Closed(old_popup));
        _ = app.update(Message::Closed(old_backdrop));
        assert_eq!(app.popup, popup);
        assert_eq!(app.backdrop, backdrop);
        _ = app.update(Message::SurfacePressed(backdrop.unwrap()));
        assert!(app.popup.is_none() && app.backdrop.is_none());
    }

    #[test]
    fn compositor_closing_either_surface_cleans_up_the_other() {
        for close_backdrop in [false, true] {
            let mut app = test_app();
            _ = app.open_popup();
            let id = if close_backdrop {
                app.backdrop
            } else {
                app.popup
            }
            .unwrap();
            _ = app.update(Message::Closed(id));
            assert!(app.popup.is_none() && app.backdrop.is_none());
        }
    }

    #[test]
    fn pointer_motion_and_focus_changes_neither_open_nor_close_dashboard() {
        for open in [false, true] {
            let mut app = test_app();
            let panel = window::Id::unique();
            app.core.set_main_window_id(Some(panel));
            if open {
                _ = app.update(Message::TogglePopup);
            }
            let popup = app.popup;
            let backdrop = app.backdrop;
            for id in [
                panel,
                popup.unwrap_or_else(window::Id::unique),
                backdrop.unwrap_or_else(window::Id::unique),
            ] {
                for status in [iced::event::Status::Ignored, iced::event::Status::Captured] {
                    for event in [
                        iced::Event::Mouse(iced::mouse::Event::CursorEntered),
                        iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                            position: iced::Point::new(12.0, 24.0),
                        }),
                        iced::Event::Mouse(iced::mouse::Event::CursorLeft),
                        iced::Event::Window(window::Event::Focused),
                        iced::Event::Window(window::Event::Unfocused),
                    ] {
                        assert!(input_event(event, status, id).is_none());
                    }
                }
            }
            _ = app.update(Message::Tick);
            assert_eq!(app.popup, popup);
            assert_eq!(app.backdrop, backdrop);
        }
    }

    #[test]
    fn login_popup_waits_for_layout_opens_once_and_preview_does_not_claim() {
        const CHILD: &str = "FINANCE_DASHBOARD_LOGIN_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            // Run the app-level login path in its own process so environment
            // isolation cannot race other tests or touch the real login marker.
            let runtime = std::env::temp_dir().join(format!(
                "finance-dashboard-login-ui-{}",
                rand::random::<u64>()
            ));
            std::fs::create_dir(&runtime).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "app::tests::login_popup_waits_for_layout_opens_once_and_preview_does_not_claim", "--nocapture"])
                .env(CHILD, "1")
                .env("XDG_RUNTIME_DIR", &runtime)
                .env("XDG_SESSION_ID", "finance-dashboard-test")
                .output()
                .unwrap();
            std::fs::remove_dir_all(&runtime).unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let runtime = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
        let make_app = |preview| {
            let mut core = app::Core::default();
            core.set_main_window_id(Some(window::Id::unique()));
            FinanceDashboard::init(core, (Settings::default(), preview)).0
        };
        let rectangle = Rectangle {
            x: 10.0,
            y: 0.0,
            width: 120.0,
            height: 32.0,
        };
        let mut preview = make_app(true);
        preview.rectangle = rectangle;
        _ = preview.update(Message::Tick);
        assert!(preview.popup.is_none() && preview.backdrop.is_none());
        assert!(!runtime.join(login::RUNTIME_DIRECTORY).exists());

        let mut app = make_app(false);
        _ = app.update(Message::Tick);
        assert!(app.login_popup_pending && app.popup.is_none());
        assert!(!runtime.join(login::RUNTIME_DIRECTORY).exists());
        app.rectangle = rectangle;
        _ = app.update(Message::Tick);
        assert!(!app.login_popup_pending);
        assert!(app.popup.is_some() && app.backdrop.is_some());
        assert_eq!(
            std::fs::read_dir(runtime.join(login::RUNTIME_DIRECTORY))
                .unwrap()
                .count(),
            1
        );
        _ = app.update(Message::Close);
        _ = app.update(Message::Tick);
        assert!(app.popup.is_none() && app.backdrop.is_none());

        let mut restarted = make_app(false);
        restarted.rectangle = rectangle;
        _ = restarted.update(Message::Tick);
        assert!(!restarted.login_popup_pending);
        assert!(restarted.popup.is_none() && restarted.backdrop.is_none());
        _ = restarted.update(Message::TogglePopup);
        assert!(restarted.popup.is_some());
    }

    #[test]
    fn explicit_external_ui_actions_remove_the_dismissal_layer() {
        for message in [
            Message::Open("https://calendar.google.com/".into()),
            Message::BrowseClient,
        ] {
            let mut app = test_app();
            _ = app.open_popup();
            // Only inspect state; the browser/portal tasks are not executed.
            _ = app.update(message);
            assert!(app.popup.is_none() && app.backdrop.is_none());
        }
    }

    #[test]
    fn banner_draft_survives_navigation_and_cancel_restores_saved_text() {
        let mut app = test_app();
        let original = serde_json::to_value(&app.settings).unwrap();
        _ = app.update(Message::Settings(true));
        let popup = app.popup;
        let backdrop = app.backdrop;
        _ = app.update(Message::BannerInput("A new day".into()));
        _ = app.update(Message::Settings(false));
        _ = app.update(Message::Settings(true));
        assert_eq!(app.banner_input, "A new day");
        assert_eq!(serde_json::to_value(&app.settings).unwrap(), original);
        assert_eq!(app.popup, popup);
        assert_eq!(app.backdrop, backdrop);
        _ = app.update(Message::CancelBanner);
        assert_eq!(app.banner_input, app.settings.banner_message);
        assert_eq!(serde_json::to_value(&app.settings).unwrap(), original);
    }

    #[test]
    fn invalid_banner_save_retains_draft_and_all_saved_preferences() {
        let mut app = test_app();
        let original = serde_json::to_value(&app.settings).unwrap();
        for invalid in [
            "   ".to_owned(),
            "a".repeat(config::MAX_BANNER_CHARACTERS + 1),
        ] {
            _ = app.update(Message::BannerInput(invalid.clone()));
            _ = app.update(Message::SaveBanner);
            assert_eq!(app.banner_input, invalid);
            assert!(app.notice.as_ref().unwrap().contains("banner"));
            assert_eq!(serde_json::to_value(&app.settings).unwrap(), original);
        }
        _ = app.update(Message::BannerInput("Ready for today".into()));
        assert!(app.notice.is_none());
        _ = app.update(Message::CancelBanner);
        assert_eq!(app.banner_input, app.settings.banner_message);
    }

    #[test]
    fn clock_edit_is_a_draft_and_cancel_preserves_existing_clocks() {
        let mut app = test_app();
        let original = app.settings.clocks.clone();
        app.clock_label = "Unsubmitted new clock".into();
        _ = app.update(Message::EditClock(1));
        _ = app.update(Message::EditClockLabel("London".into()));
        _ = app.update(Message::EditClockZone("Europe/London".into()));
        // A different row cannot be removed or selected under an active draft.
        _ = app.update(Message::RemoveClock(0));
        _ = app.update(Message::EditClock(2));
        let edit = app.clock_edit.as_ref().unwrap();
        assert_eq!(edit.index, 1);
        assert_eq!(edit.label, "London");
        assert_eq!(edit.timezone, "Europe/London");
        assert_eq!(app.settings.clocks, original);
        _ = app.update(Message::CancelClockEdit);
        assert!(app.clock_edit.is_none());
        assert_eq!(app.settings.clocks, original);
        assert_eq!(app.clock_label, "Unsubmitted new clock");
    }

    #[test]
    fn invalid_clock_edit_keeps_the_draft_and_reports_validation_error() {
        let mut app = test_app();
        let original = app.settings.clocks.clone();
        _ = app.update(Message::EditClock(1));
        _ = app.update(Message::EditClockLabel("London".into()));
        _ = app.update(Message::EditClockZone("Not/A_Timezone".into()));
        _ = app.update(Message::SaveClock);
        assert_eq!(app.settings.clocks, original);
        assert_eq!(app.clock_edit.as_ref().unwrap().label, "London");
        assert!(app.notice.as_ref().unwrap().contains("IANA"));
        _ = app.update(Message::EditClockZone("Europe/London".into()));
        assert!(app.notice.is_none());
        _ = app.update(Message::CancelClockEdit);
        assert_eq!(app.settings.clocks, original);
    }

    #[test]
    fn outside_click_subscription_sees_captured_and_uncaptured_presses() {
        let id = window::Id::unique();
        for status in [iced::event::Status::Ignored, iced::event::Status::Captured] {
            let message = input_event(
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                status,
                id,
            );
            assert!(matches!(message, Some(Message::SurfacePressed(window)) if window == id));
        }
    }
}

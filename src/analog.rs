use cosmic::{
    Theme,
    iced::{Color, Point, Rectangle, Renderer, mouse},
    widget::canvas::{self, Frame, Geometry, Path, Stroke, Text},
};
use std::f32::consts::TAU;

pub struct AnalogClock {
    pub hour: u32,
    pub minute: u32,
}

impl<Message> canvas::Program<Message, Theme> for AnalogClock {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = frame.center();
        let scale = bounds.width.min(bounds.height) / 116.0;
        let radius = (bounds.width.min(bounds.height) / 2.0 - 5.0 * scale).max(0.5);
        let cosmic = theme.cosmic();
        let ink: Color = cosmic.background(theme.transparent).on.into();
        let accent: Color = cosmic.accent.base.into();
        let muted: Color = cosmic.background(theme.transparent).divider.into();
        frame.fill(
            &Path::circle(center, radius),
            Color::from(cosmic.primary(theme.transparent).base),
        );
        frame.stroke(
            &Path::circle(center, radius),
            Stroke::default().with_color(muted).with_width(1.5 * scale),
        );
        let point = |angle: f32, length: f32| {
            Point::new(
                center.x + angle.sin() * length,
                center.y - angle.cos() * length,
            )
        };
        for tick in 0..60 {
            let major = tick % 5 == 0;
            let angle = tick as f32 * TAU / 60.0;
            frame.stroke(
                &Path::line(
                    point(
                        angle,
                        radius - if major { 10.0 * scale } else { 6.0 * scale },
                    ),
                    point(angle, radius - 3.0 * scale),
                ),
                Stroke::default()
                    .with_color(if major { ink } else { muted })
                    .with_width(if major { 2.0 * scale } else { scale }),
            );
        }
        for (number, angle) in [
            (12, 0.0),
            (3, TAU / 4.0),
            (6, TAU / 2.0),
            (9, 3.0 * TAU / 4.0),
        ] {
            frame.fill_text(Text {
                content: number.to_string(),
                position: point(angle, radius - 21.0 * scale),
                color: ink,
                size: (13.0 * scale).into(),
                align_x: cosmic::iced::alignment::Horizontal::Center.into(),
                align_y: cosmic::iced::alignment::Vertical::Center,
                font: cosmic::font::default(),
                ..Text::default()
            });
        }
        let hour_angle = (self.hour % 12) as f32 * TAU / 12.0 + self.minute as f32 * TAU / 720.0;
        let minute_angle = self.minute as f32 * TAU / 60.0;
        frame.stroke(
            &Path::line(center, point(hour_angle, radius * 0.46)),
            Stroke::default().with_color(ink).with_width(4.0 * scale),
        );
        frame.stroke(
            &Path::line(center, point(minute_angle, radius * 0.72)),
            Stroke::default().with_color(accent).with_width(2.5 * scale),
        );
        frame.fill(&Path::circle(center, 3.5 * scale), accent);
        vec![frame.into_geometry()]
    }
}

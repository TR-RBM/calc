use calc_viz::{Emphasis, KindColour, StyleRole};

use crate::colour::Colour;
use crate::geometry::ScaleFactor;
use crate::mapping::pixel_round;
use crate::theme::Theme;

const CHANNEL_MAXIMUM: f64 = 255.0;
const BAND_ALPHA: u8 = 0x59;
const PATTERN_LINE_LOGICAL: u16 = 1;
const HATCH_SPACING_LOGICAL: u16 = 4;
const STIPPLE_DOT_LOGICAL: u16 = 1;
const STIPPLE_PITCH_LOGICAL: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Role {
    Missing,
    Unresolved,
    MayBeHit,
    Marked,
    Undecided,
    Inside,
    BackFace,
    Provisional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fill {
    Solid(Colour),
    Role(Role),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Patterns {
    origin: (u16, u16),
    line: u16,
    spacing: u16,
    dot: u16,
    pitch: u16,
    theme: Theme,
}

impl Patterns {
    pub(crate) fn new(theme: Theme, scale: ScaleFactor, origin: (u16, u16)) -> Patterns {
        let size = |logical| scale.to_physical_size(logical).unwrap_or(u16::MAX).max(1);
        let width = |logical| scale.to_line_width(logical).unwrap_or(u16::MAX);
        Patterns {
            origin,
            line: width(PATTERN_LINE_LOGICAL),
            spacing: size(HATCH_SPACING_LOGICAL),
            dot: width(STIPPLE_DOT_LOGICAL),
            pitch: size(STIPPLE_PITCH_LOGICAL),
            theme,
        }
    }

    pub(crate) fn ink(&self, role: Role, x: u16, y: u16) -> Option<Colour> {
        let across = i32::from(x) - i32::from(self.origin.0);
        let down = i32::from(y) - i32::from(self.origin.1);
        let on = |value: i32, period: u16, width: u16| {
            value.rem_euclid(i32::from(period)) < i32::from(width)
        };
        let hatch = on(across + down, self.spacing, self.line);
        match role {
            Role::Missing => hatch.then_some(self.theme.text_secondary),
            Role::Unresolved => (on(across, self.pitch, self.dot)
                && on(down, self.pitch, self.dot))
            .then_some(self.theme.text_secondary),
            Role::MayBeHit => None,
            Role::Marked => {
                on(across - down, self.spacing, self.line).then_some(self.theme.text_secondary)
            }
            Role::Undecided => (hatch || on(across - down, self.spacing, self.line))
                .then_some(self.theme.text_tertiary),
            Role::Inside => Some(self.theme.text),
            Role::BackFace => Some(self.theme.text_tertiary),
            Role::Provisional => None,
        }
    }

    pub(crate) fn covering(&self, fill: Fill, x: u16, y: u16) -> Colour {
        match fill {
            Fill::Solid(colour) => colour,
            Fill::Role(role) => self.ink(role, x, y).unwrap_or(self.theme.window),
        }
    }
}

pub(crate) fn kind_colour(theme: &Theme, kind: KindColour) -> Colour {
    match kind {
        KindColour::Exact => theme.kinds.exact,
        KindColour::Symbolic => theme.kinds.symbolic,
        KindColour::Numeric => theme.kinds.numeric,
        KindColour::Sampled => theme.kinds.sampled,
        KindColour::Measured | KindColour::Derived => theme.kinds.measured,
        KindColour::Running => theme.kinds.running,
        KindColour::Differs => theme.kinds.differs,
    }
}

pub(crate) fn provisional(theme: &Theme, colour: Colour) -> Colour {
    let blend = |ink: u8, ground: u8| {
        u8::try_from((u16::from(ink) + u16::from(ground)).div_ceil(2)).unwrap_or(u8::MAX)
    };
    Colour {
        red: blend(colour.red, theme.window.red),
        green: blend(colour.green, theme.window.green),
        blue: blend(colour.blue, theme.window.blue),
        alpha: colour.alpha,
    }
}

pub(crate) fn role_tone(theme: &Theme, role: Role) -> Colour {
    match role {
        Role::Missing | Role::Unresolved | Role::MayBeHit | Role::Marked => theme.text_secondary,
        Role::Undecided | Role::BackFace => theme.text_tertiary,
        Role::Inside => theme.text,
        Role::Provisional => theme.kinds.running,
    }
}

pub(crate) fn emphasised(theme: &Theme, style: &StyleRole, colour: Colour) -> Colour {
    match style.emphasis {
        Emphasis::Normal => colour,
        Emphasis::Provisional => provisional(theme, colour),
        Emphasis::Unresolved => role_tone(theme, Role::Unresolved),
        Emphasis::Inside => role_tone(theme, Role::Inside),
        Emphasis::Undecided => role_tone(theme, Role::Undecided),
    }
}

pub(crate) fn settled(theme: &Theme, style: &StyleRole, colour: Colour) -> Colour {
    if style.emphasis == Emphasis::Provisional {
        provisional(theme, colour)
    } else {
        colour
    }
}

pub(crate) fn layer_colour(theme: &Theme, style: &StyleRole) -> Colour {
    emphasised(theme, style, kind_colour(theme, style.kind))
}

pub(crate) fn over_window(theme: &Theme, colour: Colour) -> Colour {
    let blend = |ink: u8, ground: u8| {
        let (ink, ground, alpha) = (u16::from(ink), u16::from(ground), u16::from(colour.alpha));
        u8::try_from((ink * alpha + ground * (255 - alpha) + 127) / 255).unwrap_or(u8::MAX)
    };
    Colour::opaque(
        blend(colour.red, theme.window.red),
        blend(colour.green, theme.window.green),
        blend(colour.blue, theme.window.blue),
    )
}

pub(crate) fn band_fill(colour: Colour) -> Colour {
    Colour {
        alpha: BAND_ALPHA,
        ..colour
    }
}

pub(crate) fn from_map(colour: calc_viz::Colour) -> Colour {
    let channel = |value: f64| {
        let scaled = value * CHANNEL_MAXIMUM;
        u8::try_from(pixel_round(scaled)).unwrap_or(u8::MAX)
    };
    Colour::opaque(
        channel(colour.red),
        channel(colour.green),
        channel(colour.blue),
    )
}

#[cfg(test)]
mod tests {
    use calc_viz::{LinePattern, Marker};

    use super::*;

    fn style(emphasis: Emphasis) -> StyleRole {
        StyleRole {
            kind: KindColour::Numeric,
            colour_map: None,
            line: LinePattern::Solid,
            marker: Marker::None,
            emphasis,
        }
    }

    fn patterns() -> Patterns {
        Patterns::new(
            Theme::light(),
            ScaleFactor::from_percent(100).unwrap(),
            (10, 20),
        )
    }

    #[test]
    fn derived_kind_takes_the_measured_colour() {
        let theme = Theme::light();

        assert_eq!(
            kind_colour(&theme, KindColour::Derived),
            theme.kinds.measured
        );
    }

    #[test]
    fn normal_layer_takes_its_kind_colour() {
        let theme = Theme::light();

        assert_eq!(
            layer_colour(&theme, &style(Emphasis::Normal)),
            theme.kinds.numeric
        );
    }

    #[test]
    fn provisional_layer_is_halfway_to_the_window() {
        let theme = Theme::light();
        let ink = Colour::opaque(0, 100, 255);
        let window = theme.window;

        let blended = provisional(&theme, ink);

        assert_eq!(
            blended.red,
            u8::try_from(u16::from(window.red).div_ceil(2)).unwrap()
        );
    }

    #[test]
    fn map_colour_channels_round_to_the_nearest_byte() {
        let colour = calc_viz::Colour {
            red: 1.0,
            green: 0.5,
            blue: 0.0,
        };

        assert_eq!(from_map(colour), Colour::opaque(255, 128, 0));
    }

    #[test]
    fn missing_hatch_runs_diagonally_from_the_picture_origin() {
        let patterns = patterns();
        let theme = Theme::light();

        assert_eq!(
            [
                patterns.ink(Role::Missing, 10, 20),
                patterns.ink(Role::Missing, 13, 21),
                patterns.ink(Role::Missing, 11, 20),
            ],
            [Some(theme.text_secondary), Some(theme.text_secondary), None]
        );
    }

    #[test]
    fn unresolved_stipple_sets_one_dot_per_grid_cell() {
        let patterns = patterns();

        let dots = (10..16)
            .flat_map(|x| (20..26).map(move |y| (x, y)))
            .filter(|(x, y)| patterns.ink(Role::Unresolved, *x, *y).is_some())
            .count();

        assert_eq!(dots, 4);
    }

    #[test]
    fn undecided_cross_hatch_holds_both_diagonals() {
        let patterns = patterns();

        assert!(
            patterns.ink(Role::Undecided, 13, 21).is_some()
                && patterns.ink(Role::Undecided, 11, 21).is_some()
        );
    }

    #[test]
    fn pattern_ground_is_the_window() {
        let patterns = patterns();

        assert_eq!(
            patterns.covering(Fill::Role(Role::Missing), 11, 20),
            Theme::light().window
        );
    }

    #[test]
    fn inside_is_solid_text() {
        assert_eq!(
            patterns().ink(Role::Inside, 57, 91),
            Some(Theme::light().text)
        );
    }

    #[test]
    fn back_face_is_solid_tertiary_text() {
        assert_eq!(
            patterns().ink(Role::BackFace, 0, 0),
            Some(Theme::light().text_tertiary)
        );
    }
}

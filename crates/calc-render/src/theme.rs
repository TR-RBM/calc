use crate::colour::Colour;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindColours {
    pub exact: Colour,
    pub symbolic: Colour,
    pub numeric: Colour,
    pub sampled: Colour,
    pub measured: Colour,
    pub running: Colour,
    pub differs: Colour,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub window: Colour,
    pub panel: Colour,
    pub session_bar: Colour,
    pub line: Colour,
    pub text: Colour,
    pub text_secondary: Colour,
    pub text_tertiary: Colour,
    pub selection: Colour,
    pub kinds: KindColours,
}

impl Theme {
    pub const fn light() -> Self {
        Self {
            window: Colour::from_rgb_hex(0xfb_fa_f7),
            panel: Colour::from_rgb_hex(0xf3_f1_ec),
            session_bar: Colour::from_rgb_hex(0xef_ec_e6),
            line: Colour::from_rgb_hex(0xd8_d4_cb),
            text: Colour::from_rgb_hex(0x1c_1e_22),
            text_secondary: Colour::from_rgb_hex(0x43_47_4e),
            text_tertiary: Colour::from_rgb_hex(0x62_66_6c),
            selection: Colour::from_rgb_hex(0xe4_e0_d6),
            kinds: KindColours {
                exact: Colour::from_rgb_hex(0x1c_70_42),
                symbolic: Colour::from_rgb_hex(0x10_6b_76),
                numeric: Colour::from_rgb_hex(0x2b_5c_b3),
                sampled: Colour::from_rgb_hex(0x80_40_a8),
                measured: Colour::from_rgb_hex(0x89_57_07),
                running: Colour::from_rgb_hex(0x5f_62_68),
                differs: Colour::from_rgb_hex(0xb3_26_1e),
            },
        }
    }

    pub const fn dark() -> Self {
        Self {
            window: Colour::from_rgb_hex(0x17_18_1b),
            panel: Colour::from_rgb_hex(0x1c_1e_22),
            session_bar: Colour::from_rgb_hex(0x20_22_26),
            line: Colour::from_rgb_hex(0x30_33_3a),
            text: Colour::from_rgb_hex(0xe7_e5_df),
            text_secondary: Colour::from_rgb_hex(0xb8_bb_c0),
            text_tertiary: Colour::from_rgb_hex(0x92_96_9b),
            selection: Colour::from_rgb_hex(0x2a_2d_33),
            kinds: KindColours {
                exact: Colour::from_rgb_hex(0x5c_c2_8e),
                symbolic: Colour::from_rgb_hex(0x4f_c3_cf),
                numeric: Colour::from_rgb_hex(0x7b_a6_f0),
                sampled: Colour::from_rgb_hex(0xbf_93_e6),
                measured: Colour::from_rgb_hex(0xe1_a8_4e),
                running: Colour::from_rgb_hex(0x9a_9e_a5),
                differs: Colour::from_rgb_hex(0xf0_71_67),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_has_its_own_window_colour() {
        assert_ne!(Theme::dark().window, Theme::light().window);
    }
}

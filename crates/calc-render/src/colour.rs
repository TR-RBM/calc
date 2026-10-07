#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Colour {
    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: u8::MAX,
        }
    }

    pub const fn from_rgb_hex(hex: u32) -> Self {
        let [_, red, green, blue] = hex.to_be_bytes();
        Self::opaque(red, green, blue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hex_splits_into_channels_in_order() {
        let colour = Colour::from_rgb_hex(0x1c_70_42);

        assert_eq!(colour, Colour::opaque(0x1c, 0x70, 0x42));
    }

    #[test]
    fn rgb_hex_colour_is_opaque() {
        let colour = Colour::from_rgb_hex(0x00_00_00);

        assert_eq!(colour.alpha, u8::MAX);
    }
}

use crate::colour::Colour;
use crate::geometry::PhysicalRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawCommand {
    FillRect {
        rect: PhysicalRect,
        colour: Colour,
    },
    FrameRect {
        rect: PhysicalRect,
        line_width: u16,
        colour: Colour,
    },
    FillRoundedRect {
        rect: PhysicalRect,
        radius: u16,
        colour: Colour,
    },
    FrameRoundedRect {
        rect: PhysicalRect,
        radius: u16,
        line_width: u16,
        colour: Colour,
    },
    FillTriangle {
        corners: [(u16, u16); 3],
        colour: Colour,
    },
    FillQuadrilateral {
        corners: [(u16, u16); 4],
        colour: Colour,
    },
    FillEllipse {
        rect: PhysicalRect,
        colour: Colour,
    },
    FrameEllipse {
        rect: PhysicalRect,
        line_width: u16,
        colour: Colour,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrawList {
    commands: Vec<DrawCommand>,
}

impl DrawList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: DrawCommand) {
        self.commands.push(command);
    }

    pub fn append(&mut self, mut other: DrawList) {
        self.commands.append(&mut other.commands);
    }

    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appended_commands_follow_existing_commands() {
        let first = DrawCommand::FillRect {
            rect: PhysicalRect::new(0, 0, 1, 1),
            colour: Colour::opaque(1, 2, 3),
        };
        let second = DrawCommand::FillRect {
            rect: PhysicalRect::new(1, 1, 1, 1),
            colour: Colour::opaque(4, 5, 6),
        };
        let mut list = DrawList::new();
        list.push(first);
        let mut other = DrawList::new();
        other.push(second);

        list.append(other);

        assert_eq!(list.commands(), &[first, second]);
    }
}

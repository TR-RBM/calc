use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Localized {
    text: String,
}

impl Localized {
    pub(crate) fn new(text: String) -> Self {
        Self { text }
    }

    pub fn blank() -> Self {
        Self {
            text: String::new(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for Localized {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

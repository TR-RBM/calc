use calc_numbers::Number;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParameterRole {
    Slider,
    TimeAxis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreeParameter {
    pub name: String,
    pub lower: Number,
    pub upper: Number,
    pub step: Option<Number>,
    pub current: Number,
    pub role: ParameterRole,
}

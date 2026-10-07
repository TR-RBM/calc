use calc_core::Seed;
use calc_exec::{BackendKind, Domain};
use calc_numbers::Number;

use crate::escape_time::{EscapeTimeForm, IterationLimitRule};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResultId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub lower: Number,
    pub upper: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneInput {
    pub expression: String,
    pub sampling_box: Vec<Interval>,
    pub variables: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntegratorMethod {
    ExplicitEuler,
    ClassicalRungeKutta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayeringMethod {
    LongestPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SamplingMethod {
    UniformGrid,
    AdaptiveSubdivision {
        maximum_depth: u32,
        refinement_threshold: Number,
    },
    MonteCarlo {
        sample_count: u64,
    },
    EscapeTime {
        form: EscapeTimeForm,
        limit_rule: IterationLimitRule,
        iterations_used: u32,
    },
    Integrator {
        method: IntegratorMethod,
        step: Number,
    },
    GraphLayout {
        layering: LayeringMethod,
    },
    Direct,
}

impl SamplingMethod {
    pub fn draws_random_numbers(&self) -> bool {
        matches!(self, SamplingMethod::MonteCarlo { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxisResolution {
    pub sample_count: u64,
    pub fineness: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdaptiveSpacing {
    pub finest: Number,
    pub coarsest: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub domain: Domain,
    pub axes: Vec<AxisResolution>,
    pub adaptive: Option<AdaptiveSpacing>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CorpusReference(pub String);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SamplingDiagnostics {
    pub nan_samples: u64,
    pub infinite_samples: u64,
    pub refinement_limit_cells: u64,
    pub escaped_cells: u64,
    pub inside_cells: u64,
    pub undecided_cells: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneRecord {
    pub result: ResultId,
    pub inputs: Vec<SceneInput>,
    pub method: SamplingMethod,
    pub resolution: Resolution,
    pub seed: Option<Seed>,
    pub backend: BackendKind,
    pub references: Vec<CorpusReference>,
    pub diagnostics: SamplingDiagnostics,
}

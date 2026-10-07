use calc_exec::Backend;
use calc_expr::ExprPool;

use crate::sampling::{
    SampleError, SamplePlan, SampleRequest, SampledShape, SamplingRun, plan_sampling,
};
use crate::scene::Scene;

const COARSEST_HALVINGS: u32 = 4;

#[derive(Debug)]
pub enum RefinementStep {
    Pending,
    Level {
        level: usize,
        levels: usize,
        scene: Box<Scene>,
    },
    Cancelled,
}

pub struct RefinementRun {
    levels: Vec<SamplingRun>,
    current: usize,
    is_finished: bool,
}

pub fn level_divisions(divisions: u32) -> Vec<u32> {
    let mut levels: Vec<u32> = Vec::new();
    for halvings in (0..=COARSEST_HALVINGS).rev() {
        let count = divisions.div_ceil(1_u32 << halvings).max(1);
        if levels.last() != Some(&count) {
            levels.push(count);
        }
    }
    levels
}

pub struct RefinementPlan {
    levels: Vec<SamplePlan>,
}

impl RefinementPlan {
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    pub fn build(self) -> RefinementRun {
        RefinementRun {
            levels: self.levels.into_iter().map(SamplePlan::build).collect(),
            current: 0,
            is_finished: false,
        }
    }
}

pub fn prepare_refinement(
    pool: &mut ExprPool,
    request: &SampleRequest,
) -> Result<RefinementRun, SampleError> {
    plan_refinement(pool, request).map(RefinementPlan::build)
}

pub fn plan_refinement(
    pool: &mut ExprPool,
    request: &SampleRequest,
) -> Result<RefinementPlan, SampleError> {
    if request.shape != SampledShape::EscapeTime {
        return Err(SampleError::RefinementNeedsEscapeTime);
    }
    let mut levels = Vec::new();
    let mut previous: Option<Vec<u32>> = None;
    for halvings in (0..=COARSEST_HALVINGS).rev() {
        let divisions: Vec<u32> = request
            .axes
            .iter()
            .map(|axis| axis.divisions.div_ceil(1_u32 << halvings).max(1))
            .collect();
        if previous.as_ref() == Some(&divisions) {
            continue;
        }
        let mut level = request.clone();
        for (axis, count) in level.axes.iter_mut().zip(&divisions) {
            axis.divisions = *count;
        }
        levels.push(plan_sampling(pool, &level)?);
        previous = Some(divisions);
    }
    Ok(RefinementPlan { levels })
}

impl RefinementRun {
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    pub fn step(
        &mut self,
        backends: &[&dyn Backend],
        chunk_length: usize,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<RefinementStep, SampleError> {
        if self.is_finished {
            return Err(SampleError::RunFinished);
        }
        if is_cancelled() {
            self.is_finished = true;
            return Ok(RefinementStep::Cancelled);
        }
        let levels = self.levels.len();
        let Some(run) = self.levels.get_mut(self.current) else {
            self.is_finished = true;
            return Err(SampleError::RunFinished);
        };
        match run.step(backends, chunk_length)? {
            None => Ok(RefinementStep::Pending),
            Some(scene) => {
                let level = self.current;
                self.current += 1;
                self.is_finished = self.current == levels;
                Ok(RefinementStep::Level {
                    level,
                    levels,
                    scene: Box::new(scene),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use calc_exec::{Domain, Preference};
    use calc_exec_cpu::CpuBackend;
    use calc_expr::SymbolKind;
    use calc_numbers::{Integer, Number};

    use super::*;
    use crate::escape_time::{EscapeTimeForm, IterationLimitRule};
    use crate::primitive::{Column, Primitive};
    use crate::record::{Interval, ResultId};
    use crate::sampling::{AxisBounds, EscapeTimeRequest, SampleAxis, sample};
    use crate::style::KindColour;
    use crate::view::{AxisUnit, Dimension};

    fn interval(lower: Number, upper: Number) -> Interval {
        Interval { lower, upper }
    }

    fn request(pool: &mut ExprPool, divisions: [u32; 2]) -> SampleRequest {
        let names = ["x", "y"];
        let bounds = [
            interval(Number::from(-2_i64), Number::from(1_i64)),
            interval(
                Number::fraction(&Integer::from(-3_i64), &Integer::from(2_i64)).unwrap(),
                Number::fraction(&Integer::from(3_i64), &Integer::from(2_i64)).unwrap(),
            ),
        ];
        let axes = names
            .iter()
            .zip(bounds)
            .zip(divisions)
            .map(|((name, bounds), divisions)| SampleAxis {
                symbol: pool.intern_symbol(name, SymbolKind::Variable).unwrap(),
                name: String::from(*name),
                bounds: AxisBounds::Exact(bounds),
                divisions,
                dimension: Dimension::DIMENSIONLESS,
                unit: AxisUnit::dimensionless(),
            })
            .collect();
        SampleRequest {
            result: ResultId(1),
            expression_text: String::from("escape"),
            shape: SampledShape::EscapeTime,
            axes,
            value_range: None,
            value_dimension: Dimension::DIMENSIONLESS,
            value_unit: AxisUnit::dimensionless(),
            value_divisions: 256,
            domain: Domain::F64,
            preference: Preference::Automatic,
            sample_limit: 100_000,
            kind: KindColour::Sampled,
            references: Vec::new(),
            escape_time: Some(EscapeTimeRequest {
                form: EscapeTimeForm::QuadraticParameter,
                limit_rule: IterationLimitRule::Fixed { iterations: 32 },
            }),
        }
    }

    fn scenes(run: &mut RefinementRun, cancel_after: Option<usize>) -> (Vec<Scene>, bool) {
        let backend = CpuBackend::new();
        let delivered = Cell::new(0_usize);
        let mut scenes = Vec::new();
        loop {
            let is_cancelled = || cancel_after.is_some_and(|after| delivered.get() >= after);
            match run.step(&[&backend], 64, &is_cancelled).unwrap() {
                RefinementStep::Pending => {}
                RefinementStep::Level { levels, scene, .. } => {
                    scenes.push(*scene);
                    delivered.set(scenes.len());
                    if scenes.len() == levels {
                        return (scenes, false);
                    }
                }
                RefinementStep::Cancelled => return (scenes, true),
            }
        }
    }

    fn cell_counts(scene: &Scene) -> Vec<u32> {
        match &scene.frames[0].layers[0].primitive {
            Primitive::ScalarGrid(grid) => grid.cells.counts.clone(),
            _ => Vec::new(),
        }
    }

    fn run_all(run: &mut RefinementRun) -> Vec<Scene> {
        scenes(run, None).0
    }

    fn level_request(pool: &mut ExprPool, level: usize) -> SampleRequest {
        let full = request(pool, [32, 16]);
        let halvings = COARSEST_HALVINGS - u32::try_from(level).unwrap();
        let mut level_request = full.clone();
        for axis in &mut level_request.axes {
            axis.divisions = axis.divisions.div_ceil(1 << halvings).max(1);
        }
        level_request
    }

    fn built_level_matches_the_pool_bound_sample(level: usize) {
        let mut pool = ExprPool::new();
        let full = request(&mut pool, [32, 16]);
        let plan = plan_refinement(&mut pool, &full).unwrap();
        let backend = CpuBackend::new();
        let direct_request = level_request(&mut pool, level);
        let direct = sample(&mut pool, &[&backend], &direct_request).unwrap();

        let mut run = plan.build();
        let built = run_all(&mut run);

        assert_eq!(built[level], direct);
    }

    #[test]
    fn plan_can_be_sent_to_a_job() {
        fn is_send<T: Send>(_: &T) -> bool {
            true
        }
        let mut pool = ExprPool::new();
        let full = request(&mut pool, [32, 16]);

        let plan = plan_refinement(&mut pool, &full).unwrap();

        assert!(is_send(&plan));
    }

    #[test]
    fn plan_names_one_level_per_distinct_division() {
        let mut pool = ExprPool::new();
        let full = request(&mut pool, [32, 16]);

        let plan = plan_refinement(&mut pool, &full).unwrap();

        assert_eq!(plan.level_count(), 5);
    }

    #[test]
    fn built_coarsest_level_matches_the_pool_bound_sample() {
        built_level_matches_the_pool_bound_sample(0);
    }

    #[test]
    fn built_second_level_matches_the_pool_bound_sample() {
        built_level_matches_the_pool_bound_sample(1);
    }

    #[test]
    fn built_third_level_matches_the_pool_bound_sample() {
        built_level_matches_the_pool_bound_sample(2);
    }

    #[test]
    fn built_fourth_level_matches_the_pool_bound_sample() {
        built_level_matches_the_pool_bound_sample(3);
    }

    #[test]
    fn built_finest_level_matches_the_pool_bound_sample() {
        built_level_matches_the_pool_bound_sample(4);
    }

    #[test]
    fn built_plan_hands_over_the_scenes_of_the_pool_bound_refinement() {
        let mut pool = ExprPool::new();
        let full = request(&mut pool, [32, 16]);
        let plan = plan_refinement(&mut pool, &full).unwrap();
        let mut pool_bound = prepare_refinement(&mut pool, &full).unwrap();

        let mut built = plan.build();

        assert_eq!(run_all(&mut built), run_all(&mut pool_bound));
    }

    #[test]
    fn plan_of_another_shape_is_rejected() {
        let mut pool = ExprPool::new();
        let mut curve = request(&mut pool, [8, 8]);
        let x = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
        curve.shape = SampledShape::Curve(pool.symbol(x).unwrap());

        let result = plan_refinement(&mut pool, &curve);

        assert!(matches!(
            result,
            Err(SampleError::RefinementNeedsEscapeTime)
        ));
    }

    #[test]
    fn levels_halve_from_one_sixteenth_up_to_the_full_divisions() {
        assert_eq!(level_divisions(1920), vec![120, 240, 480, 960, 1920]);
    }

    #[test]
    fn small_divisions_keep_each_level_once() {
        assert_eq!(level_divisions(5), vec![1, 2, 3, 5]);
    }

    #[test]
    fn run_hands_over_one_scene_per_level_from_coarse_to_fine() {
        let mut pool = ExprPool::new();
        let request = request(&mut pool, [32, 16]);
        let mut run = prepare_refinement(&mut pool, &request).unwrap();

        let (scenes, _) = scenes(&mut run, None);

        assert_eq!(
            scenes.iter().map(cell_counts).collect::<Vec<_>>(),
            vec![
                vec![2, 1],
                vec![4, 2],
                vec![8, 4],
                vec![16, 8],
                vec![32, 16]
            ]
        );
    }

    #[test]
    fn finest_level_is_the_scene_of_the_full_request() {
        let mut pool = ExprPool::new();
        let request = request(&mut pool, [24, 12]);
        let mut run = prepare_refinement(&mut pool, &request).unwrap();
        let backend = CpuBackend::new();
        let direct = sample(&mut pool, &[&backend], &request).unwrap();

        let (scenes, _) = scenes(&mut run, None);

        let (Some(finest), Primitive::ScalarGrid(expected)) =
            (scenes.last(), &direct.frames[0].layers[0].primitive)
        else {
            panic!("expected a grid");
        };
        let Primitive::ScalarGrid(refined) = &finest.frames[0].layers[0].primitive else {
            panic!("expected a grid");
        };
        let bits = |column: &Column| match column {
            Column::F64(values) => values.iter().map(|value| value.to_bits()).collect(),
            Column::F32(_) => Vec::new(),
        };
        assert_eq!(
            (bits(&refined.scalar), &refined.classes),
            (bits(&expected.scalar), &expected.classes)
        );
    }

    #[test]
    fn step_within_a_level_is_pending() {
        let mut pool = ExprPool::new();
        let request = request(&mut pool, [32, 16]);
        let mut run = prepare_refinement(&mut pool, &request).unwrap();
        let backend = CpuBackend::new();

        let _coarsest = run.step(&[&backend], 64, &|| false).unwrap();
        let next = run.step(&[&backend], 1, &|| false).unwrap();

        assert!(matches!(next, RefinementStep::Pending));
    }

    #[test]
    fn cancellation_between_levels_hands_over_no_finer_scene() {
        let mut pool = ExprPool::new();
        let request = request(&mut pool, [32, 16]);
        let mut run = prepare_refinement(&mut pool, &request).unwrap();

        let (scenes, cancelled) = scenes(&mut run, Some(2));

        assert_eq!((scenes.len(), cancelled), (2, true));
    }

    #[test]
    fn cancelled_run_takes_no_further_step() {
        let mut pool = ExprPool::new();
        let request = request(&mut pool, [32, 16]);
        let mut run = prepare_refinement(&mut pool, &request).unwrap();
        let backend = CpuBackend::new();
        run.step(&[&backend], 64, &|| true).unwrap();

        let after = run.step(&[&backend], 64, &|| false);

        assert!(matches!(after, Err(SampleError::RunFinished)));
    }

    #[test]
    fn refinement_of_another_shape_is_rejected() {
        let mut pool = ExprPool::new();
        let mut curve = request(&mut pool, [8, 8]);
        let x = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
        curve.shape = SampledShape::Curve(pool.symbol(x).unwrap());

        let result = prepare_refinement(&mut pool, &curve);

        assert!(matches!(
            result,
            Err(SampleError::RefinementNeedsEscapeTime)
        ));
    }
}

use calc_viz::{
    BackendKind, Domain, Resolution, ResultId, SamplingDiagnostics, SamplingMethod, SceneRecord,
    Seed,
};

#[test]
fn scene_record_is_built_from_calc_viz_paths_alone() {
    let record = SceneRecord {
        result: ResultId(1),
        inputs: Vec::new(),
        method: SamplingMethod::MonteCarlo { sample_count: 4 },
        resolution: Resolution {
            domain: Domain::F32,
            axes: Vec::new(),
            adaptive: None,
        },
        seed: Some(Seed {
            value: 7,
            generator: String::from("test"),
        }),
        backend: BackendKind::Cpu,
        references: Vec::new(),
        diagnostics: SamplingDiagnostics::default(),
    };

    assert_eq!(
        (record.resolution.domain, record.backend),
        (Domain::F32, BackendKind::Cpu)
    );
}

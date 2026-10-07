#[path = "../tests/golden/scenes.rs"]
mod scenes;

use std::path::PathBuf;
use std::process::ExitCode;

use calc_render::geometry::ScaleFactor;
use calc_render::image::encode_pam;
use calc_render::theme::Theme;
use calc_render::{FontSet, RenderRequest, render_scene};

const WIDTH: u16 = 640;
const HEIGHT: u16 = 480;
const TEXT_SIZE: u16 = 14;

fn main() -> ExitCode {
    let Ok(fonts) = FontSet::bundled() else {
        return ExitCode::FAILURE;
    };
    let Ok(scale) = ScaleFactor::from_percent(100) else {
        return ExitCode::FAILURE;
    };
    let text = scenes::picture_text();
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let scenes = [
        ("curve", scenes::curve()),
        ("escape_time", scenes::escape_time()),
        ("figure", scenes::figure()),
        ("surface", scenes::surface()),
        ("diverging_grid", scenes::diverging_grid()),
        ("domain_grid", scenes::domain_grid()),
        ("mesh_missing", scenes::mesh_with_a_missing_vertex()),
    ];
    for (name, scene) in scenes {
        let request = RenderRequest {
            scene: &scene,
            settled: None,
            frame: 0,
            width: WIDTH,
            height: HEIGHT,
            text_size: TEXT_SIZE,
            scale,
            theme: Theme::light(),
            decimal_separator: '.',
            text: &text,
        };
        let Ok(rendered) = render_scene(&request, &fonts) else {
            return ExitCode::FAILURE;
        };
        if std::fs::write(
            golden.join(format!("{name}.pam")),
            encode_pam(&rendered.image),
        )
        .is_err()
        {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

#![cfg(target_arch = "x86_64")]

#[path = "golden/scenes.rs"]
mod scenes;

use calc_render::geometry::ScaleFactor;
use calc_render::image::encode_pam;
use calc_render::theme::Theme;
use calc_render::{FontSet, RenderRequest, render_scene};
use calc_viz::Scene;

const WIDTH: u16 = 640;
const HEIGHT: u16 = 480;

fn rendered_pam(scene: &Scene) -> Vec<u8> {
    let fonts = FontSet::bundled().expect("bundled fonts load");
    let text = scenes::picture_text();
    let request = RenderRequest {
        scene,
        settled: None,
        frame: 0,
        width: WIDTH,
        height: HEIGHT,
        text_size: 14,
        scale: ScaleFactor::from_percent(100).expect("non-zero scale"),
        theme: Theme::light(),
        decimal_separator: '.',
        text: &text,
    };
    encode_pam(
        &render_scene(&request, &fonts)
            .expect("golden scene renders")
            .image,
    )
}

fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .or_else(|| (left.len() != right.len()).then(|| left.len().min(right.len())))
}

#[test]
fn curve_with_a_gap_and_an_enclosure_matches_its_golden_image() {
    let pam = rendered_pam(&scenes::curve());

    assert_eq!(
        first_difference(&pam, include_bytes!("golden/curve.pam")),
        None
    );
}

#[test]
fn escape_time_grid_with_its_classes_matches_its_golden_image() {
    let pam = rendered_pam(&scenes::escape_time());

    assert_eq!(
        first_difference(&pam, include_bytes!("golden/escape_time.pam")),
        None
    );
}

#[test]
fn right_triangle_figure_matches_its_golden_image() {
    let pam = rendered_pam(&scenes::figure());

    assert_eq!(
        first_difference(&pam, include_bytes!("golden/figure.pam")),
        None
    );
}

#[test]
fn ridge_surface_with_back_faces_and_occlusion_matches_its_golden_image() {
    let pam = rendered_pam(&scenes::surface());

    assert_eq!(
        first_difference(&pam, include_bytes!("golden/surface.pam")),
        None
    );
}

#[test]
fn diverging_grid_with_a_missing_cell_matches_its_golden_image() {
    let pam = rendered_pam(&scenes::diverging_grid());

    assert_eq!(
        first_difference(&pam, include_bytes!("golden/diverging_grid.pam")),
        None
    );
}

#[test]
fn domain_coloured_grid_matches_its_golden_image() {
    let pam = rendered_pam(&scenes::domain_grid());

    assert_eq!(
        first_difference(&pam, include_bytes!("golden/domain_grid.pam")),
        None
    );
}

#[test]
fn mesh_with_a_missing_vertex_matches_its_golden_image() {
    let pam = rendered_pam(&scenes::mesh_with_a_missing_vertex());

    assert_eq!(
        first_difference(&pam, include_bytes!("golden/mesh_missing.pam")),
        None
    );
}

use calc_render::geometry::ScaleFactor;
use calc_render::image::{encode_pam, encode_png};
use calc_render::theme::Theme;
use calc_render::{FontSet, PictureText, RenderRequest, Rendered, ViewRequest, render_scene};
use calc_viz::{FigureLayout, Scene};

pub const IMAGE_WIDTH: u32 = 400;
pub const IMAGE_HEIGHT: u32 = 300;
pub const TEXT_SIZE: u16 = 14;
pub const SCALE_PERCENT: u16 = 100;
pub const DECIMAL_SEPARATOR: char = '.';
pub const BYTES_PER_PIXEL: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub plot_area: PixelRect,
    pub figure_layout: Option<FigureLayout>,
}

fn rendered(scene: &Scene, width: u32, height: u32) -> Result<Rendered, String> {
    rendered_with_settled(scene, width, height, None)
}

fn rendered_with_settled(
    scene: &Scene,
    width: u32,
    height: u32,
    settled: Option<&[ViewRequest]>,
) -> Result<Rendered, String> {
    let fonts = FontSet::bundled().map_err(|error| format!("{error:?}"))?;
    let text = PictureText::default();
    let request = RenderRequest {
        scene,
        frame: 0,
        width: u16::try_from(width).map_err(|error| format!("{error:?}"))?,
        height: u16::try_from(height).map_err(|error| format!("{error:?}"))?,
        text_size: TEXT_SIZE,
        scale: ScaleFactor::from_percent(SCALE_PERCENT).map_err(|error| format!("{error:?}"))?,
        theme: Theme::light(),
        decimal_separator: DECIMAL_SEPARATOR,
        settled,
        text: &text,
    };
    render_scene(&request, &fonts).map_err(|error| format!("{error:?}"))
}

pub fn render_sized(scene: &Scene, width: u32, height: u32) -> Result<Picture, String> {
    let rendered = rendered(scene, width, height)?;
    picture_of(&rendered).ok_or_else(|| "the layout has no view".to_string())
}

fn picture_of(rendered: &Rendered) -> Option<Picture> {
    let plot = rendered.layout.views.first()?.plot_area;
    Some(Picture {
        width: u32::from(rendered.image.width()),
        height: u32::from(rendered.image.height()),
        rgba: rendered.image.rgba().to_vec(),
        plot_area: PixelRect {
            left: u32::from(plot.x),
            top: u32::from(plot.y),
            width: u32::from(plot.width),
            height: u32::from(plot.height),
        },
        figure_layout: rendered.figure_layouts.iter().flatten().next().cloned(),
    })
}

pub fn render(scene: &Scene) -> Picture {
    render_sized(scene, IMAGE_WIDTH, IMAGE_HEIGHT).expect("the reference renderer draws the scene")
}

pub fn pam_bytes(scene: &Scene) -> Vec<u8> {
    encode_pam(
        &rendered(scene, IMAGE_WIDTH, IMAGE_HEIGHT)
            .expect("rendered")
            .image,
    )
}

pub fn png_bytes(scene: &Scene) -> Vec<u8> {
    encode_png(
        &rendered(scene, IMAGE_WIDTH, IMAGE_HEIGHT)
            .expect("rendered")
            .image,
    )
    .expect("PNG encodes")
}

pub fn render_settled(scene: &Scene, settled: &[ViewRequest]) -> Picture {
    let rendered = rendered_with_settled(scene, IMAGE_WIDTH, IMAGE_HEIGHT, Some(settled))
        .expect("the reference renderer draws the scene");
    picture_of(&rendered).expect("the layout holds a view")
}

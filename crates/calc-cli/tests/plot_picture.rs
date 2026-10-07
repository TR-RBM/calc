use std::sync::mpsc::channel;

use calc_app::{
    AxisDisplayUnit, AxisState, FixedClock, Job, JobState, PictureEvent, PictureSlot, PlotJob,
    PlotRequest, Session, UtcTimestamp, ViewState, registered_backends,
};
use calc_render::geometry::ScaleFactor;
use calc_render::theme::Theme;
use calc_render::{
    FontSet, Interval, LayoutRequest, Number, PictureText, RenderRequest, Rendered, Scene,
    ViewKind, ViewRequest, plot_layout, render_scene,
};

const WIDTH: u16 = 480;
const HEIGHT: u16 = 360;
const TEXT_SIZE: u16 = 14;

fn scale() -> ScaleFactor {
    ScaleFactor::from_percent(100).expect("non-zero scale")
}

fn interval(lower: i64, upper: i64) -> Interval {
    Interval {
        lower: Number::from(lower),
        upper: Number::from(upper),
    }
}

fn session() -> Session {
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    Session::new(Box::new(clock), registered_backends())
}

fn curve_request(line: calc_app::LineId, fonts: &FontSet) -> PlotRequest {
    let layout = plot_layout(
        &LayoutRequest {
            width: WIDTH,
            height: HEIGHT,
            text_size: TEXT_SIZE,
            scale: scale(),
            views: vec![ViewRequest::of_kind(ViewKind::Plane)],
            legends: Vec::new(),
        },
        fonts,
    )
    .expect("picture has room for a plot area");
    let axis = |lower, upper| AxisState {
        range: Some(interval(lower, upper)),
        unit: AxisDisplayUnit::Coherent,
    };
    PlotRequest {
        line,
        slot: PictureSlot(0),
        iteration_limit: None,
        views: Some(vec![ViewState {
            axes: vec![axis(-4, 4), axis(-2, 2)],
            camera: None,
        }]),
        divisions: vec![layout.views[0].divisions()],
        parameters: None,
    }
}

fn run(job: &mut PlotJob) {
    while job.step() == JobState::Pending {}
}

fn finished_scene(events: impl Iterator<Item = PictureEvent>) -> Option<Scene> {
    events
        .filter_map(|event| match event {
            PictureEvent::Bounds { scene, .. } => Some(*scene),
            PictureEvent::Samples { .. } | PictureEvent::Failed { .. } => None,
        })
        .last()
}

fn render(scene: &Scene, fonts: &FontSet) -> Rendered {
    let text = PictureText::default();
    render_scene(
        &RenderRequest {
            settled: None,
            scene,
            frame: 0,
            width: WIDTH,
            height: HEIGHT,
            text_size: TEXT_SIZE,
            scale: scale(),
            theme: Theme::light(),
            decimal_separator: '.',
            text: &text,
        },
        fonts,
    )
    .expect("sampled scene renders")
}

#[test]
fn sampled_curve_is_drawn_in_the_plot_area_the_layout_gave_its_divisions() {
    let fonts = FontSet::bundled().expect("bundled fonts load");
    let mut session = session();
    let line = session.enter("sin(x)").expect("line is entered");
    let (sender, receiver) = channel();
    let mut job = session
        .plot(&curve_request(line, &fonts), sender)
        .expect("line is plottable");

    run(&mut job);
    let scene = finished_scene(receiver.try_iter()).expect("bounds arrive");
    let rendered = render(&scene, &fonts);

    let area = rendered.layout.views[0].plot_area;
    let ink = Theme::light().kinds.numeric;
    let drawn = (area.y..area.bottom()).any(|y| {
        rendered.image.pixel(area.x + area.width / 2, y)
            == Some([ink.red, ink.green, ink.blue, ink.alpha])
    });
    assert!(drawn);
}

#[test]
fn cancelled_request_sends_nothing_after_the_newer_request() {
    let fonts = FontSet::bundled().expect("bundled fonts load");
    let mut session = session();
    let line = session.enter("x^2").expect("line is entered");
    let (first_sender, first_receiver) = channel();
    let request = curve_request(line, &fonts);
    let mut first = session
        .plot(&request, first_sender)
        .expect("line is plottable");
    let (second_sender, _second_receiver) = channel();
    let _second = session
        .plot(&request, second_sender)
        .expect("line is plottable");

    run(&mut first);

    assert_eq!(first_receiver.try_iter().count(), 0);
}

#[test]
fn newer_request_holds_the_latest_generation() {
    let fonts = FontSet::bundled().expect("bundled fonts load");
    let mut session = session();
    let line = session.enter("x^2").expect("line is entered");
    let request = curve_request(line, &fonts);
    let (first_sender, _first_receiver) = channel();
    let first = session
        .plot(&request, first_sender)
        .expect("line is plottable");
    let (second_sender, _second_receiver) = channel();
    let second = session
        .plot(&request, second_sender)
        .expect("line is plottable");

    let latest = session.latest_picture_generation(line, PictureSlot(0));

    assert_eq!(
        (
            latest == Some(second.generation()),
            latest == Some(first.generation())
        ),
        (true, false)
    );
}

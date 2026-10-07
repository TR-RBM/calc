# calc-viz

## What it does

Turns expressions into pictures without drawing them. The output is a `Scene`, which every renderer consumes.

`Scene` holds a format version, a record, free parameters, views and frames. A frame holds one exact value per free parameter and a list of layers. A layer draws one primitive into one view and names the input it came from. `Primitive` is a closed enum: `Polyline`, `Band`, `Points`, `Arrows`, `ScalarGrid`, `ComplexGrid`, `TriangleMesh`, `Voxels`, `Graph`, `Formula` and `BitLayout`. Sample values are stored as `Column::F32` or `Column::F64` in the domain of the plan that produced them. A scene holds no pool id.

The scene format version is 5. Every view axis carries `divisions`, the number of steps the frontend distinguishes along it. A space view's camera is an exact azimuth in degrees from 0 up to 360 and an exact elevation from −90 to 90, with its projection. A style role names its emphasis: `Normal`, `Provisional` or `Unresolved`. A layer carries `value_bounds`, one `F64` column of rounding bounds per value column of its primitive, where NaN means unknown, an optional `PrecisionLimit` with the axes where the grid is exhausted and the counts of unresolved samples and unknown bounds, and the `readings` it offers. Scenes are regenerated from their requests and never stored, so there is no conversion from version 1. `Column` equality compares bits, so two columns with the same NaN are equal and `+0` differs from `-0`.

`Scene::new` sets the format version and checks the scene, and `Scene::check` checks an existing one. It checks:

- There is at least one frame.
- Every bound in views, sampling boxes, cell grids and colour map ranges is an exact number, and every interval has a lower bound below its upper bound.
- Every input has one variable name per axis of its sampling box.
- The seed is present exactly when the method draws random numbers.
- Every free parameter has exact bounds, a current value inside them and a positive step. Every frame has one exact value per parameter.
- Every layer names a view and an input that exist.
- Every view axis has at least one division, and a camera's angles lie in their ranges.
- A layer has one bounds column per value column of its primitive, each in `F64` and as long as its column. The value columns are the last coordinate of a polyline or points, the lower and upper column of a band, the components of arrows, the scalar of a scalar grid or voxels, the real and imaginary column of a complex grid, and the last vertex column of a triangle mesh. A graph, formula or bit layout has none. A layer with `Provisional` emphasis may carry no bounds yet, because they arrive in a second completion.
- A precision limit names only axes of the layer's view, and a layer offers readings only when its primitive has value columns.
- Every primitive has one coordinate column per view axis and columns of equal length. `Band`, `ComplexGrid` and `BitLayout` need a plane view, and `Voxels` needs a space view. A grid has one value per cell. Triangle and edge indices point at existing vertices. A bit layout has one sign bit, then the exponent, then a non-empty fraction.

`sample` samples an expression over exact intervals and returns a scene with one frame and one layer.

- Each axis names a variable, its bounds and a number of divisions. Bounds are either expressions, which `calc-core` evaluates exactly and which must be rational, so `pi` is rejected, or an exact interval, as the view state of an interactive picture gives them. A machine number in an exact interval is rejected.
- `Curve`, `Points` and `Band` take one axis. They sample the partition points `lower + k·(upper − lower)/n` for `k` from 0 to `n`, so `n` divisions give `n + 1` samples. `ScalarGrid` takes two axes and samples the cell centres `lower + (k + 1/2)·(upper − lower)/n`. Grid values are stored in row-major order with the first axis varying fastest.
- Each position is computed exactly and rounded once to the plan domain with ties to even.
- `calc-exec` lowers each expression to a plan, selects a backend from the ones given, and runs the plan on the batch of positions. The second expression of a band runs on the backend chosen for the first.
- The record holds the expression text given by the caller, the sampling box, the variable names, `UniformGrid`, the domain, per axis the sample count and the fineness of the partition as an exact number, the backend, the references given, and the count of NaN and infinite samples.
- The value axis of a curve, and the colour map range of a grid, is the range the caller gave. Without one it is the range from the smallest to the largest finite sample, converted exactly. When those are equal, the range is widened by the magnitude of the value on each side, and by 1 when the value is zero. When no sample is finite, the range is −1 to 1.
- Non-finite samples stay in the columns as values.
- The request names the largest number of samples it allows. A larger request fails before anything is sampled.

A sampled scene's view axes carry the divisions of the sampled axes and the request's `value_divisions` for the value axis. Its layer has `Normal` emphasis, one `ValueAt` reading over the sampled variables, a rounding bound for every value, and a precision limit when anything is not resolved.

- Bounds come from `calc_core::enclose_sample` at the exact position of every sample, compared with the value the plan returned, so each bound covers the rounding of the positions, the constants and every operation. A complex grid has one bounds column per part. A voxel layer keeps the bounds of its occupied cells. A value without an enclosure has a NaN bound, which means unknown.
- The enclosures need an expression pool, and the run holds none. `prepare_sampling` therefore copies the sampled expressions and variables into a small pool the run owns, and each step encloses its chunk there, so bounds are computed on the worker in the same cancellable steps as the values. An expression with a quantity, or an axis symbol that is not in the pool, cannot be copied and is an error.
- A curve also carries `ColumnEnclosures`, one entry per column between two neighbouring samples:
  - an enclosure of the function over the whole column;
  - the span it is proven to take there, by the intermediate value theorem on the edge enclosures when the expression is continuous where enclosed;
    - widened by the attained range, when the axis variable occurs once in the line: evaluated from the inside out with inward rounding, where a sine or cosine attains 1 or −1 wherever a proven multiple of π/2 lies in its argument;
  - a mark where an edge sample is unresolved.
- Where the line is a quotient of polynomials in the axis variable with rational coefficients, the edges and columns are exact:
  - edge values come from the exact curve;
  - a column without a turning point is enclosed by its edge values;
  - a turning point is narrowed and enclosed tightly;
  - a pole leaves its column without an enclosure;
  - the drawn values are the exact values rounded to the nearest `f64`.
- Otherwise a column is enclosed in `f64` intervals and tightened by the slope from both edges (the mean value form).
- A column whose enclosure exceeds its proven span by more than a value step is subdivided: 4 and then 16 parts when exact, 8 in `f64`.
- The value range is fitted like the default value interval described below. Where the samples vary less than their median bound, the precision limit says so (`varies_below_bounds`), and it counts the marked columns (`marked_columns`).
- The grid limit holds for an axis when the fineness of its partition is below the spacing of the plan domain's machine numbers just above the larger magnitude of its bounds, compared exactly. The axis is then named in `grid_exhausted`.
- The value limit holds for a sample when twice its bound is at least one step of what the value decides: the value axis width over its divisions for a curve, points, a band and a surface height, the width of the matching view axis for an arrow component, the colour range over 256 for a scalar grid, and the same for the larger of the two bounds of a complex cell. A voxel is unresolved when its bound reaches its absolute value. Such samples are counted in `unresolved_samples`, and unknown bounds in `unknown_bounds`.
- The precision limit is `None` when no axis is exhausted and no sample is unresolved or unknown. A surface or voxel view has the camera at azimuth 315 and elevation 30 degrees. Direct pictures carry no bounds, no precision limit and no readings, and their view axes have one division.

Figures. `Primitive::Figure` is a plane geometric figure: two coordinate columns of points, segments with a solid or auxiliary stroke, angles running counterclockwise from their first arm to their second, marks (angle arcs, the two right-angle marks, equality ticks and a direction arrow), labels, and a scale that is a sketch or an exact number of view units per metre. A label is attached to one element, names a vertex, side or angle, carries its name in the curriculum's notation, and has a quantity: named, given with a value, sought with an optional step, or answered. A value is coherent, metres or radians, as a rational, a rational multiple of π or an exact enclosure, with its dimension and its printed text. `Scene::check` rejects a figure outside a plane view, an index outside its list, a segment or angle whose points coincide, a label whose kind differs from its element, a mark on the wrong kind of element, a scale that is not exact and positive, and a label value that is not exact or whose enclosure is reversed.

`check_figure(figure, notation, task)` runs the figure labelling consistency checks and returns typed faults, empty when the figure passes. `FigureNotation` gives the curriculum's name triples such as A, a, α, the symbols for an angle by three points, the vertex order, the right-angle mark and whether equality marks are confirmed. `FigureTask` gives the names in the task text with their kinds, the right angles, designations and equalities.

- C1: vertex labels have distinct names and distinct points. C2: a side named in a triple joins the points named by the other vertices. C3: an angle named in a triple sits at its vertex, and an angle written with ∠ or ∡ and three vertex names sits at the middle one with arms at the other two. C4: with counterclockwise order, the signed area of the first three vertices, computed exactly from the drawn coordinates, is positive.
- C5: every task name is labelled with its kind, and every label is used, given, answered, or a vertex that a used side or angle name corresponds to. C6: right-angle marks stand exactly on the given right angles in the curriculum's mark, and a hypotenuse does not touch a right-angle vertex. C8: no name labels two elements or is used for another kind.
- C7: a given or answered label has printed text, a side's value is a length and an angle's value is dimensionless. In a figure to scale, a length passes when its squared drawn length lies between (49/50·s·g)² and (51/50·s·g)², exactly in rationals. An angle is the counterclockwise angle between the arms from `atan2_f64` at the corners of the enclosed exact cross and dot products, normalized to 0 up to 2π, and passes when its difference from the given value is at most π/180 on outward enclosures. An enclosure whose bounds decide differently, or an angle whose box of cross and dot product reaches the positive real axis, where the normalized angle jumps, gives an undecided fault instead of a pass. A straight angle, whose box lies across the negative real axis, is decided from the hull of its four normalized corners.
- Point 3: equality marks only where the curriculum confirms them, and equality ticks, angle arcs with two or more arcs, and, where the curriculum confirms equality marks, single arcs on unnamed angles only on elements given as equal. A single arc on a named angle, or anywhere equality marks are not confirmed, only marks the angle. Point 7: a direction arrow only on an oriented angle. Point 18: at most one sought label without a step and no repeated step. Point 20: every mark on an element the task uses.

`check_label_layout(figure, layout)` runs the figure layout rules over the `FigureLayout` a renderer reports in device pixels: the text size of 1 em, every label as its four corners so a label parallel to a side can be rotated, every stroke as a polyline with its width and the element it draws, every arc with its vertex, arm ends, radius, arc count and spacing, dot and square, and every tick group with its count, length and spacing, and the marks the key names. It reports:

- a label closer than 0.25 em to the edge of a stroke, or to another label, and a stroke that is not 1 or 2 logical px wide as snapped at the layout's scale factor, a whole number of device pixels, rounded down and at least one;
- an arc radius other than 2 em, further arcs not 0.3 em apart, a right-angle square other than 0.8 em, and a dot that is not 0.3 em wide, at least 2 px, or not on the bisector at half the radius within half a pixel, with every size rounded to whole pixels, ties away from zero, and ticks other than 0.8 em long and 0.3 em apart;
- an arm shorter than the outermost arc radius plus the box of the angle's name plus 0.25 em, an angle name drawn inside its arc that does not lie within the arc's sector less the margin, and a name drawn outside that is not beyond the arcs on the bisector;
- a vertex or side label nearer to another element of its kind than to its own, and a mark that the key does not name.

Escape-time pictures. `SampledShape::EscapeTime` with an `EscapeTimeRequest` samples `z² + c` over a plane of cell centres: `QuadraticParameter` samples `c` from `z = 0`, and `QuadraticInitial` samples `z` for an exact `c`. The plan comes from `calc_exec::escape_time_iteration_plan`, one bounded iteration gate whose body steps `z` and counts, so every backend that runs it gives the counts of the unrolled circuit bit for bit, and `calc-exec-cpu` stops each cell at its escape. A backend without iteration gates is skipped by selection. The iteration limit is `Fixed` or `FollowingDepth`: the depth is the largest number of halvings of the default real width (7/2 for the parameter plane, 4 for Julia sets) that the view's real width fits, and the limit is the base plus the step per halving, at most the cap, and never above 65536. Every cell gets a class in the grid's class column: `INSIDE_CELL` when the exact main cardioid or period-2 disc test holds for `c` (decided on an enclosure first and exactly only when that is undecided), otherwise `ESCAPED_CELL` when the machine orbit escaped, otherwise `UNDECIDED_CELL`. Only escaped cells carry their count; the others are NaN and drawn by their class. The diagnostics count the three classes, the record names the form, the rule and the iterations used, the bounds of every count are unknown and no count is unresolved. `Scene::check` rejects an escape-time layer without a class column, with a class code above 2, with a class column of the wrong length, or with class counts that do not add up to its cells.

Axis display units. Every `ViewAxis` carries an `AxisUnit`: `Coherent`, `Unit` with an exact factor and power of π, or `TemperatureScale` with an exact factor and offset to kelvin, each with the display symbol. Bounds and sample coordinates of an axis are in its unit. `Scene::check` rejects a factor that is not exact and positive, an offset that is not exact, a temperature scale on an axis that is not a temperature or that is logarithmic, a dimensioned axis without a symbol, and a dimensionless coherent axis with one.

A `SampleAxis` names its unit and a `SampleRequest` the unit of its value axis. While preparing, sampling builds one expression in the pool: each input variable of a non-coherent axis is replaced by its shown-to-coherent map, `x · f · π^k` or `f · x + offset`, and for a curve, points, a band or a surface the value is mapped back to the value axis's unit, `v / (f · π^k)` or `(v - offset) / f`. That expression is lowered and enclosed, so the rounding bound covers the conversion. A scalar grid and an occupancy convert their inputs and keep their values coherent. A vector field, a complex grid and an escape-time picture need coherent axes. An axis is also marked in `grid_exhausted` when two neighbouring positions become the same machine value after the plan's own conversion gates.

`sample` is `prepare_sampling` followed by `SamplingRun::step` until the scene is delivered. `prepare_sampling` does everything that needs the expression pool: it evaluates the bounds, checks the limits, lowers each expression to a plan and computes the sample positions. The `SamplingRun` it returns holds no pool id, so it can move to a worker thread. Each `step` runs the plans on at most the given number of positions and returns the scene after the last chunk. The backend is selected once per plan for the whole run. Because a plan's map stage treats every element on its own, a run in chunks gives the same bits as a run in one step, so a caller can stop between steps and a zoomed view samples the same bits as an earlier scene at the positions they share. This is true only for plans without a reduce stage, because chunking a reduction would change its tree shape. `prepare_sampling` rejects a reduced plan, so a run never chunks one. A step after the scene was delivered is an error.

A caller can instead split the run in two stages.
- `step_samples` runs the plans without enclosing, and after the last chunk returns a provisional scene: the same samples, no `value_bounds`, no `precision`, and the `Provisional` emphasis.
- `step_bounds` then encloses the stored samples chunk by chunk, and after the last chunk returns the settled scene, equal to the scene of one `step` run.
- `step_bounds` before the samples are finished is `SamplesPending`.
- `step` after `step_samples`, or the reverse, is `StageOrder`.

The shapes and what they sample:

| Shape | Axes | Positions | Primitive |
|---|---|---|---|
| `Curve` | 1 | partition points | `Polyline` of position and value |
| `Band` | 1 | partition points | `Band` of position, lower and upper expression |
| `Points` | 1 | partition points | `Points` of position and value |
| `ScalarGrid` | 2 | cell centres | `ScalarGrid` with a sequential colour map |
| `VectorField` | 2 | cell centres | `Arrows` based at the centres, components from two expressions |
| `ComplexGrid` | 2 | cell centres | `ComplexGrid` of one complex variable, real part on the first axis and imaginary part on the second, with domain colouring |
| `Surface` | 2 | partition points | `TriangleMesh` with the value as height and as scalar, two triangles per cell |
| `Occupancy` | 3 | cell centres | `Voxels` for the cells whose value is greater than zero, with the value as scalar |

- Both axes of a complex grid must name the same variable. The lowered plan takes it as one complex input with two channels, and without a requested range the domain colouring reaches half lightness at modulus 1.
- A surface may have at most 2^32 vertices, so its triangle indices fit `u32`. A larger request fails before anything is sampled.
- A surface and a voxel scene use a space view with an orthographic camera at azimuth 315 and elevation 30 degrees around the box centre.
- Voxel coordinates are cell indices, and the voxel view is in grid units: each axis runs from −1/2 to n − 1/2, so every cell is the unit cube around its index. The sampling box in the record keeps the physical bounds.

Pictures that are not sampled are built directly and record the method `Direct` or `GraphLayout`, no sampling box and no variables. They name the CPU as backend, because they are computed on the calling thread without a plan.

- `formula_scene` places formula text at an exact anchor in a plane view. The text is given by the caller, rendered through `calc-syntax` in `calc-app`, because `calc-viz` cannot print expressions.
- `bit_layout_scene` takes an `F32` or `F64` number and gives its bits, most significant first, with the exponent starting after the sign bit and the fraction after 8 or 11 exponent bits. An exact number is rejected.
- `graph_scene` places the nodes of a directed graph with the `calc-core` graph module. A node's x coordinate is its layer from the longest path layering, and its y coordinate is minus its slot within the layer. Layer 0 is ordered by node index, and every later layer by the mean slot of each node's predecessors, ties broken by node index. The edges are all arrows or only those of the transitive reduction, as requested. A graph with a cycle is rejected with a node on the cycle.

The colour map reference functions return a `Colour` with red, green and blue in 0 to 1, or nothing for a value that must be drawn as missing. They use only correctly rounded operations and the reference `atan2_f64`, so every renderer can reproduce them bit for bit.

- `sequential_colour` and `diverging_colour` place the value in the exact range, rounded once to `f64`, clamp it to the ends, and blend linearly between five and three fixed stops. The diverging map has its neutral stop at the middle of the range. NaN is missing.
- `categorical_colour` takes the floor of the value modulo 8 as the index into eight fixed colours. A non-finite value is missing.
- `domain_colour` maps the argument of a complex value to hue, with the positive real axis red, and the modulus m to lightness m / (m + r), where r is the width of the range. Zero is black and an infinite modulus white. A NaN part is missing.

`Domain` and `BackendKind` of `calc-exec` are re-exported beside `Seed`, so a caller builds a `SceneRecord` from `calc-viz` paths alone.

`plan_sampling` does only the work that needs the pool: evaluating the axis bounds, converting and detaching the expressions, and lowering the plans. `SamplePlan::build` then builds the sample positions and batches without the pool, so a job can run it; `prepare_sampling` is the two together. `plan_refinement` and `RefinementPlan::build` split a refinement the same way. `prepare_refinement` prepares an escape-time request as a run over levels, from one sixteenth of the divisions on each axis up to the full request, halving the cell size each level and keeping a level once when rounding repeats it. `RefinementRun::step` checks the caller's cancellation before every chunk, and hands over one complete scene per level, coarse to fine. The finest scene is the scene of the full request. A cancelled run hands over nothing more. Other shapes are rejected with `RefinementNeedsEscapeTime`.

`read_orbit` reads one point of an escape-time picture: it takes the form, the limit rule, the real axis of the view read from, the domain, the backend preference and the exact point, runs the plan of that limit for that one point, and gives back the class, the escape count when the point escaped, and the limit used. `escape_time_limit` gives that limit alone, for a caller that has to name it before the reading, such as a committed reading line. `escape_time_default_view` gives the default view of a form.

`shape_legend` gives the colour legend of a sampled shape before sampling, and `layer_legend` the legend of a scene layer, which agree for every shape: Sequential for scalar grids, surfaces and occupancies, EscapeTime for escape-time grids, DomainColouring for complex grids, none for curves, bands, points and vector fields.

A sampled expression with units goes through `calc_core::to_coherent_units` before its axis unit conversions and detaching, so dimensions are checked and quantities become numbers in coherent units. The value axis takes the result's dimension and its coherent unit symbol when the request leaves the value dimensionless and coherent. A request naming another dimension, or a band whose bounds differ in dimension, fails with `ValueDimensionMismatch`, and a dimension fault inside the expression with `Quantity`. Where a sampled axis carries a dimension, the value's dimension is the request's: the expression holds its axis variables as bare symbols, so its own dimension is missing what the axes give them, and `calc-app` has already read the dimension from the expression with each variable replaced by one of its axis's unit. The axis conversions are unchanged, because they are the numbers of the shown values and the dimension is not in them.

An axis with `AxisBounds::Default` is sampled over the default interval: −10 to 10 in its display unit, 0 to 100 for a temperature, −2 to 2 for the complex plane, and the default view of its form for an escape-time picture. A request without a value range gets the default value interval: the finite values, less those marked unresolved against the interval of all values, cut to the quantiles that drop at most two percent on each side, widened by every value within half the central width, and rounded out to at most five steps of one, two or five times a power of ten. Scalar grids and surfaces take that interval as their colour range, and a complex grid takes the rounded median modulus as its reference modulus. Round steps are found by exact comparison within eight decades of a lower bound taken from bit lengths, which holds for every exact value of `f64` origin; an exact range whose exponent runs to hundreds of thousands of bits finds no step, and its interval falls back to −1 to 1.

## How to test

`cargo test -p calc-viz`

Scene tests build one valid scene and change one field for each check, one test per error. Sampling tests run on `CpuBackend` and compare sample bits, one test per shape. Direct picture tests check one picture per kind. Colour map tests check stops, clamping and missing values for each map. A test backend that fails at run time produces the run error.

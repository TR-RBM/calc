# decision_oracle

## What it does

Tests the reference rasterizer of `calc-render` against the documented rules of the scene format, of figures, of colour legends, of the plot area and of picture output, written from those rules alone, without reading the renderer. Each scene is small, and its expected pixels follow from geometry, from the reference colour functions of `calc-viz`, or from the image formats. A test compares a rendered image with the image of the same scene without layers, so axes, ticks and the theme do not enter the expectation.

Scenes are built from the public types of `calc-viz`, including `Domain` and `BackendKind`, which `calc-viz` re-exports because `calc-render` may not depend on `calc-exec`. Every scene is made with `Scene::new`, so it passes `Scene::check`.

`rendering.rs` is the one place that calls `calc-render`, through its public API: `render_scene`, `RenderRequest`, `Rendered` with its image, plot area and figure layouts, and `encode_pam` and `encode_png`.

The readings the tests take where the rules leave a detail open:

| Reading | Where the text is open |
|---|---|
| The view box maps linearly onto the plot area, the lower bound at the left or bottom edge, x to the right and y upward, with image rows counted from the top. Sub-pixel placement is not tested. | The scene format: "Screen orientation is the renderer's concern." |
| A drawn line lies within 4 pixels of its exact position at scale factor 1. | No rule fixes line widths. |
| The scalar at index `row · columns + column` is the cell in column `column` counted along x and row `row` counted from the lower bound of y. | The scene format: "one scalar column in row-major order", read as x varying fastest, row 0 at the lower y bound. |
| A colour channel `c` of a reference colour is drawn as a byte within 1 of `round(255 · clamp(c, 0, 1))`, and at least 90 percent of a cell's inner pixels carry it. | No rule fixes the conversion to bytes; 1 is allowed for the f32 path of tiny-skia. |
| The PAM header fields are separated by newlines. | Picture output names the PAM header fields, not their separator. |
| `FigureLayout` coordinates are image pixels with the origin at the top left. | The figure rules name what the layout holds, not its unit. |

## How to test

`cargo test -p calc-render --test decision_oracle`

Each test follows from one sentence of the rules:

| Test | Rule |
|---|---|
| `horizontal_polyline_is_drawn_on_the_row_of_its_height`, `horizontal_polyline_leaves_rows_away_from_its_height_untouched` | Scene format, Primitive kinds: `Polyline` holds coordinate columns of a curve. |
| `larger_y_is_drawn_higher_in_the_image` | Scene format, Views and coordinates: "In 2D, x points right and y points up." |
| `polyline_is_broken_at_a_nan_coordinate`, `polyline_pieces_beside_a_nan_are_still_drawn` | Scene format, Sample storage: "A NaN in any coordinate of a point removes that point: a polyline is broken there". |
| `polyline_is_broken_at_an_infinite_coordinate` | Scene format, Sample storage: "Infinite coordinates are treated the same way." |
| `f64_line_above_an_f32_grid_point_is_drawn_at_its_offset_from_the_view_origin` | Scene format, Sample storage: "A coordinate is converted to the renderer's working type only after the view origin is subtracted in the column's own domain." |
| `unresolved_curve_is_drawn_as_the_band_of_its_bounds`, `resolved_curve_draws_nothing_where_the_band_would_be` | Enclosures: "A run of unresolved samples on a curve is not drawn as a line. It is drawn as the band between value minus bound and value plus bound". |
| `points_of_an_exhausted_grid_are_drawn_in_another_role_than_ordinary_points` | Enclosures: "A layer with `grid_exhausted` draws its points in the unresolved style role." |
| `band_fills_between_lower_and_upper` | Scene format, Primitive kinds: `Band` holds an abscissa column and lower and upper columns. |
| `single_cell_takes_the_reference_colour_of_its_value` | Scene format, Style: "The mapping from value to colour is a pure reference function in `calc-viz`". |
| the four `…_scalar_of_the_grid_is_the_cell_at_…` tests | Scene format, Primitive kinds: `ScalarGrid` has "one scalar column in row-major order". |
| `later_layer_is_drawn_over_earlier_layer` | Scene format, Structure: "Layers are drawn in list order, later over earlier." |
| `nan_cell_is_drawn_in_a_colour_that_is_not_on_the_map` | Scene format, Sample storage: "A NaN scalar on a valid point is drawn as missing, never as a colour of the map." |
| `complex_cell_takes_the_domain_colouring_reference_colour` | Scene format, Style: "`DomainColouring` maps the argument to hue and the modulus to lightness by that reference function." |
| `solid_segment_is_drawn_without_gaps`, `auxiliary_segment_is_drawn_dashed` | Figures: "`Stroke` is `Solid` or `Auxiliary`, which renderers draw dashed." |
| `reference_layout_of_a_spacious_triangle_passes_the_label_layout_check`, `every_label_of_the_figure_has_a_box_in_the_layout`, `label_boxes_lie_inside_the_image`, `text_of_each_label_is_drawn_inside_its_box` | Figures: "The CPU rasterizer is the reference that produces `FigureLayout`", with the box of every label. |
| `angle_arc_fills_no_sector`, `angle_arc_is_drawn_at_its_reported_radius` | Figures: "A renderer draws every arc of a figure with the one radius … fills no sector". |
| `triangle_with_a_nan_vertex_is_not_drawn` | Scene format, Sample storage: "a triangle containing it is not drawn". |
| `point_with_a_nan_coordinate_is_not_drawn` | Scene format, Sample storage: "A NaN in any coordinate of a point removes that point". |
| `point_removed_for_a_nan_coordinate_adds_a_key_swatch`, `triangle_removed_for_a_nan_vertex_adds_a_key_swatch` | Picture roles: "A layer that removes any element for a missing value, whether a curve or band sample, a point, a mesh triangle or a grid cell, shows the missing role: its swatch and word stand in the key even where no mark in the picture shows the removal." |
| `arrow_is_drawn_from_its_base_along_its_components`, `arrow_is_not_drawn_behind_its_base` | Scene format, Primitive kinds: `Arrows` holds "base coordinate columns and vector component columns". |
| `graph_edge_is_drawn_between_its_nodes` | Scene format, Primitive kinds: `Graph` holds "node positions, edge index pairs". |
| `unresolved_grid_cell_is_not_drawn_in_its_map_colour` | Enclosures: "An unresolved grid cell … drawn in the unresolved style role", with the step "the colour map range's width divided by 256". |
| `picture_awaiting_its_bounds_is_drawn_in_the_provisional_role` | Enclosures: "Until the bounds arrive, the whole picture is drawn in the provisional style role". |
| `points_are_drawn`, `arrows_are_drawn`, `triangle_mesh_is_drawn`, `graph_is_drawn`, `voxels_are_drawn_in_a_space_view`, `formula_is_drawn`, `bit_layout_is_drawn` | Picture output: the scene rasterizer "draws every primitive kind of the scene format". |
| `plot_area_does_not_depend_on_the_samples` | Plot area: "the plot area never depends on the samples, and it is known before sampling". |
| `a_settled_range_with_narrower_ticks_gives_a_wider_plot_area` | Plot area: "An axis gutter is as wide as the widest tick label of the ticks that axis's settled range produces". |
| `scenes_of_one_settled_view_keep_the_plot_area_whatever_their_value_range` | Plot area: the first complete scene of a settled view fixes the gutter and "a later scene of the same settled view never changes it", and the gutter of the last settled view through a gesture. |
| `a_narrower_settled_range_gives_a_wider_plot_area_than_none` | Plot area: the gutter follows the settled range's ticks, and where none exists it is the eleven cells of the format. |
| `the_plot_area_does_not_depend_on_the_value_bounds_of_a_layer` | Plot area: "The gutter never depends on the value bounds of a layer or on a provisional view state." |
| `a_gutter_is_never_wider_than_the_eleven_cells_of_the_format` | Plot area: "It is never wider than the eleven cells the format allows." |
| `colour_mapped_layer_takes_a_legend_row_from_the_plot_area`, `second_colour_mapped_layer_takes_a_further_legend_row` | Colour legends: "Each colour-mapped layer gets one legend row", and so a picture is "shorter in its plot area by one row per layer". Only the height is claimed, because two pictures whose legends differ can have different gutters. |
| `layer_without_a_colour_map_takes_no_legend_row` | Colour legends: "A layer without a colour mapping gets none." |
| `sequential_legend_shows_sixteen_reference_swatches_from_lower_to_upper_left_to_right` | Colour legends: "a bar of discrete swatches over the map's exact range, lower end left", "Each swatch is filled with the reference colour of `calc-viz` at the value of its centre", "16 swatches for `Sequential`". |
| `diverging_legend_shows_seventeen_reference_swatches_from_lower_to_upper_left_to_right` | Colour legends: the same bar for `Diverging`, with "17 for `Diverging`". |
| `image_has_the_requested_size`, `image_holds_four_bytes_per_pixel`, `every_pixel_is_opaque` | Picture output: "An `Image` is width, height and RGBA bytes … Every picture has an opaque background". |
| `same_scene_renders_to_the_same_bytes_twice` | Picture output: "The same scene, size, text size, scale factor, theme and caller text give the same image bytes". |
| `pam_is_the_t128_header_followed_by_the_pixels` | Picture output, PAM row. |
| `png_is_signature_ihdr_one_idat_and_iend`, `every_png_chunk_has_a_valid_crc`, `png_header_is_eight_bit_rgba_of_the_image_size`, `png_data_is_stored_deflate_of_unfiltered_rows_of_the_pixels` | Picture output, PNG row. |

The helpers have their own tests: the view mapping, the colour tolerance, CRC-32 and Adler-32 against published check values, and a stored-block inflater that refuses a compressed block and a wrong checksum.

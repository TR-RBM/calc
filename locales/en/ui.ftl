ui-session-untitled = untitled
ui-session-menu = Session
ui-session-new = New
ui-session-open = Open
ui-session-save = Save
ui-session-export = Export
ui-session-replay = Replay
ui-session-accept-replay = Accept replay
ui-session-precision = precision
ui-session-backend = backend

ui-panel-line-is-all-there-is = This line’s own row is all there is to show.

ui-instrument-inspect = Inspect
ui-instrument-method = Method
ui-instrument-data = Data
ui-instrument-plot = Plot
ui-picture-drawing = drawing…
ui-picture-not-drawn = this picture could not be drawn
ui-orbit-escaped = escaped after { $count } of at most { $limit } iterations, a count with no rounding guarantee
ui-orbit-count-missing = escaped within { $limit } iterations, and the count of that escape was not kept
ui-orbit-inside = inside the set, proven, with a limit of { $limit } iterations
ui-orbit-undecided = undecided after { $limit } iterations
ui-instrument-verify = Verify
ui-instrument-export = Export

ui-input-editing = editing { $line }

ui-line-name = Name
ui-line-delete = Delete
ui-line-cancel = Cancel

ui-value-digit-count = { $count ->
    [one] { $count } digit
   *[other] { $count } digits
}

ui-plot-detach = Detach
ui-plot-dock = Dock
ui-plot-window-title = Plot of { $line }
ui-plot-resolution = resolution
ui-plot-renderer = renderer

ui-theme = Theme
ui-theme-system = System
ui-theme-light = Light
ui-theme-dark = Dark

ui-clipboard-unavailable = The clipboard is not available

ui-solve-find-ways = Find ways
ui-solve-cancel = Cancel
ui-solve-find-with-quantities = Find ways with these quantities
ui-back = Back
ui-solve-ways-to-it = ways to it
ui-solve-show-all = show all

ui-view-other-readings = other readings
ui-view-decimal-places = decimal places
ui-view-enclosure = enclosure
ui-view-significant-digits = significant digits
ui-view-show = show
ui-view-machine = in machine arithmetic
ui-view-heading-exact = { $places ->
    [one] { $places } decimal place of the exact value
   *[other] { $places } decimal places of the exact value
}
ui-view-heading-machine = { $places ->
    [one] { $places } decimal place of the machine value
   *[other] { $places } decimal places of the machine value
}
ui-view-heading-all-exact = { $places ->
    [one] the { $places } decimal place of the exact value
   *[other] all { $places } decimal places of the exact value
}
ui-view-heading-all-machine = { $places ->
    [one] the { $places } decimal place of the machine value
   *[other] all { $places } decimal places of the machine value
}
ui-view-decimal-digits = decimal digits
ui-view-cut-off = cut off
ui-view-repeats = repeats
ui-view-repeats-digit = the digit { $digits }, from decimal place { $start }
ui-view-repeats-digits = the { $length } digits { $digits }, from decimal place { $start }
ui-view-repeats-block = a block of { $length } digits, from decimal place { $start }
ui-view-ends = ends
ui-view-ends-further = the decimal expansion ends further on
ui-view-lies-between = lies between
ui-view-value = value
ui-view-width = width
ui-view-reached = { $digits ->
    [one] reached: { $digits } significant digit
   *[other] reached: { $digits } significant digits
}
ui-view-not-reached = the tightest interval proven before the precision limit
ui-view-running = running for { $seconds } s
ui-pointer-precision-and-backend = Precision and backend
ui-pointer-precision-and-backend-words = precision, backend, accuracy, exact, float, f64, f32, CPU, GPU
ui-pointer-precision-and-backend-sentence = Each session keeps its own, because they change its results.
ui-pointer-units-for-this-session = Units for this session
ui-pointer-units-for-this-session-words = units, session units, other units, override, metric, imperial
ui-pointer-units-for-this-session-sentence = A session can show other units than your preference.
ui-pointer-curriculum = Curriculum
ui-pointer-curriculum-words = curriculum, school, syllabus, answer notation, exercises, exams
ui-pointer-curriculum-sentence = It is chosen on the Learn landing, and it also decides how answers in exercises and exams are written.
ui-pointer-degrees-or-radians = Degrees or radians
ui-pointer-degrees-or-radians-words = angle mode, degrees, radians, deg, rad, DEG, RAD
ui-pointer-degrees-or-radians-sentence = There is no angle mode: the unit is written with the number, as in sin(30 deg).
ui-pointer-decimal-places = Decimal places
ui-pointer-decimal-places-words = decimal places, digits, rounding, number format, significant figures
ui-pointer-decimal-places-sentence = Each result shows the digits it has: all of an exact value, and as many as its uncertainty allows for a measured or computed one.
ui-pointer-default-precision = Default precision for new sessions
ui-pointer-default-precision-words = default precision, new session, start precision
ui-pointer-default-precision-sentence = Every new session starts the same, so the same input gives the same number on every machine.
ui-pointer-decimal-comma = Decimal comma in calculations
ui-pointer-decimal-comma-words = decimal comma, decimal point, decimal separator, comma
ui-pointer-decimal-comma-sentence = Calculations are written with a point, so a session means the same everywhere. Exercises accept the notation of their curriculum.
ui-pointer-door-session-bar = to the session bar
ui-pointer-door-session-bar-menu = to the session bar menu
ui-pointer-door-learn-landing = to the Learn landing
ui-settings = Settings
ui-settings-language = Language
ui-settings-text-size = Text size
ui-settings-text-size-step = { $percent }%
ui-locale-name = English

ui-settings-units = Units
ui-settings-units-not-set = Not set
ui-settings-by-kind = by kind
ui-settings-by-kind-set = { $count ->
    [one] by kind: { $count } set
   *[other] by kind: { $count } set
}
ui-settings-units-by-kind = Units by kind
ui-pointer-heading = Looking for something else?
ui-search-precision = Precision of this session
ui-search-precision-words = precision, digits, exact, f64, f32, accuracy
ui-search-backend = Backend of this session
ui-search-backend-words = backend, CPU, SIMD, GPU, where it computes
ui-search-language = Language
ui-search-language-words = language, English, German, Deutsch, words, translation
ui-search-theme = Theme
ui-search-theme-words = theme, dark, light, colours, night
ui-search-text-size = Text size
ui-search-text-size-words = text size, bigger, smaller, larger text, zoom, scale
ui-search-units = Units
ui-search-units-words = units, metric, imperial, Fahrenheit, Celsius, miles, kilometres
ui-search-go-to-input-area = Go to the input line
ui-search-go-to-input-area-words = go to the input line, type, prompt, write
ui-search-go-to-stack = Go to the stack
ui-search-go-to-stack-words = go to the stack, results, lines
ui-search-go-to-instrument-panel = Go to the instrument panel
ui-search-go-to-instrument-panel-words = go to the panel, instruments, Inspect, Method, Plot
ui-search-go-to-session-bar = Go to the session bar
ui-search-go-to-session-bar-words = go to the session bar, top, selectors, session
ui-search-cancel-running-line = Cancel the running line
ui-search-cancel-running-line-words = cancel, stop, running, abort
ui-search-language-reference = language reference
ui-search-language-reference-words = language reference constructs syntax how to write spelling symbols operators

ui-search-axis-unit = Unit of a picture axis
ui-search-axis-unit-words = axis unit, picture unit, axis, scale of the axis
ui-reference-title = Reference

ui-search-title = Search
ui-search-answers = No such thing
ui-stack-too-narrow = The window is too narrow to show a line.
ui-stack-too-narrow-short = Too narrow
ui-picture-keys-pan = arrows pan
ui-picture-keys-zoom = Page Up/Down zoom
ui-picture-keys-value-zoom = alt+Page Up/Down value zoom
ui-picture-keys-turn = ctrl+arrows turn
ui-picture-keys-reset = Home reset
ui-picture-keys-read = Enter read
ui-picture-keys-range = type a range
ui-picture-keys-cursor = arrows move the cursor
ui-picture-keys-layer = ctrl+arrows layer
ui-picture-keys-commit = Enter keeps the reading
ui-picture-keys-coordinate = type a coordinate
ui-picture-keys-leave = Esc back
ui-picture-range-from = { $axis } from
ui-picture-range-to = { $axis } to
ui-picture-range-unit = { $axis } in
ui-picture-range-azimuth = azimuth
ui-picture-range-elevation = elevation
ui-picture-field-not-exact = { $field } is not an exact number
ui-picture-field-not-below = { $field } is not below the upper bound
ui-picture-field-unit-unknown = { $field } is not a unit of that axis
ui-picture-row-not-applied = the view could not be set from this row
ui-area-calculate = Calculate
ui-recognized-looks-like = looks like:
ui-recognized-separator = {", "}
ui-recognized-last-separator = {", "}and{" "}
ui-recognized-more = more
ui-recognized-unnamed = { $count ->
    [one] one concept this version does not know
   *[other] { $count } concepts this version does not know
}
ui-concept-origin = from line { $line }
ui-learn-recent = Opened recently
ui-learn-landing-empty = To open a concept, go to Calculate and choose its name under a result.

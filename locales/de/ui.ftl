ui-session-untitled = unbenannt
ui-session-menu = Sitzung
ui-session-new = Neu
ui-session-open = Öffnen
ui-session-save = Speichern
ui-session-export = Exportieren
ui-session-replay = Wiederholen
ui-session-accept-replay = Wiederholung übernehmen
ui-session-precision = Genauigkeit
ui-session-backend = Backend

ui-panel-line-is-all-there-is = Mehr als die Zeile selbst gibt es hier nicht zu zeigen.

ui-instrument-inspect = Details
ui-instrument-method = Verfahren
ui-instrument-data = Daten
ui-instrument-plot = Grafik
ui-picture-drawing = wird gezeichnet …
ui-picture-not-drawn = dieses Bild konnte nicht gezeichnet werden
ui-orbit-escaped = entkommen nach { $count } von höchstens { $limit } Iterationen, eine Anzahl ohne Rundungsgarantie
ui-orbit-count-missing = entkommen innerhalb von { $limit } Iterationen, die Anzahl wurde nicht aufbewahrt
ui-orbit-inside = bewiesen in der Menge, mit einer Grenze von { $limit } Iterationen
ui-orbit-undecided = unentschieden nach { $limit } Iterationen
ui-instrument-verify = Gegenprobe
ui-instrument-export = Export

ui-input-editing = bearbeite { $line }

ui-line-name = Benennen
ui-line-delete = Löschen
ui-line-cancel = Abbrechen

ui-value-digit-count = { $count ->
    [one] { $count } Ziffer
   *[other] { $count } Ziffern
}

ui-plot-detach = Lösen
ui-plot-dock = Andocken
ui-plot-window-title = Grafik von { $line }
ui-plot-resolution = Auflösung
ui-plot-renderer = Renderer

ui-theme = Erscheinungsbild
ui-theme-system = System
ui-theme-light = Hell
ui-theme-dark = Dunkel

ui-clipboard-unavailable = Die Zwischenablage ist nicht verfügbar

ui-solve-find-ways = Wege finden
ui-solve-cancel = Abbrechen
ui-solve-find-with-quantities = Wege mit diesen Größen finden
ui-back = Zurück
ui-solve-ways-to-it = Wege dorthin
ui-solve-show-all = alle zeigen

ui-view-other-readings = weitere Ablesungen
ui-view-decimal-places = Nachkommastellen
ui-view-enclosure = Einschließung
ui-view-significant-digits = signifikante Stellen
ui-view-show = zeigen
ui-view-machine = in Maschinenarithmetik
ui-view-heading-exact = { $places ->
    [one] { $places } Nachkommastelle des exakten Werts
   *[other] { $places } Nachkommastellen des exakten Werts
}
ui-view-heading-machine = { $places ->
    [one] { $places } Nachkommastelle des Maschinenwerts
   *[other] { $places } Nachkommastellen des Maschinenwerts
}
ui-view-heading-all-exact = { $places ->
    [one] die { $places } Nachkommastelle des exakten Werts
   *[other] alle { $places } Nachkommastellen des exakten Werts
}
ui-view-heading-all-machine = { $places ->
    [one] die { $places } Nachkommastelle des Maschinenwerts
   *[other] alle { $places } Nachkommastellen des Maschinenwerts
}
ui-view-decimal-digits = Nachkommaziffern
ui-view-cut-off = abgeschnitten
ui-view-repeats = Periode
ui-view-repeats-digit = die Ziffer { $digits }, ab der { $start }. Nachkommastelle
ui-view-repeats-digits = die { $length } Ziffern { $digits }, ab der { $start }. Nachkommastelle
ui-view-repeats-block = ein Block aus { $length } Ziffern, ab der { $start }. Nachkommastelle
ui-view-ends = Ende
ui-view-ends-further = die Dezimalentwicklung bricht weiter hinten ab
ui-view-lies-between = liegt zwischen
ui-view-value = Wert
ui-view-width = Breite
ui-view-reached = { $digits ->
    [one] erreicht: { $digits } signifikante Stelle
   *[other] erreicht: { $digits } signifikante Stellen
}
ui-view-not-reached = das engste Intervall, das vor der Genauigkeitsgrenze bewiesen wurde
ui-view-running = läuft seit { $seconds } s
ui-pointer-precision-and-backend = Genauigkeit und Backend
ui-pointer-precision-and-backend-words = Genauigkeit, Backend, Rechenwerk, exakt, Gleitkomma, f64, f32, CPU, GPU
ui-pointer-precision-and-backend-sentence = Jede Sitzung behält ihre eigene Genauigkeit und ihr eigenes Backend, weil beide ihre Ergebnisse verändern.
ui-pointer-units-for-this-session = Einheiten für diese Sitzung
ui-pointer-units-for-this-session-words = Einheiten, Sitzungseinheiten, andere Einheiten, metrisch, imperial
ui-pointer-units-for-this-session-sentence = Eine Sitzung kann andere Einheiten zeigen als deine Einstellung.
ui-pointer-curriculum = Lehrplan
ui-pointer-curriculum-words = Lehrplan, Schule, Schreibweise der Antworten, Übungen, Prüfungen
ui-pointer-curriculum-sentence = Er wird auf der Startseite von Lernen gewählt und bestimmt auch, wie Antworten in Übungen und Prüfungen geschrieben werden.
ui-pointer-degrees-or-radians = Grad oder Bogenmaß
ui-pointer-degrees-or-radians-words = Winkelmodus, Grad, Bogenmaß, Radiant, deg, rad, DEG, RAD
ui-pointer-degrees-or-radians-sentence = Es gibt keinen Winkelmodus: Die Einheit steht bei der Zahl, wie in sin(30 deg).
ui-pointer-decimal-places = Nachkommastellen
ui-pointer-decimal-places-words = Nachkommastellen, Stellen, Ziffern, Runden, Zahlenformat, signifikante Stellen
ui-pointer-decimal-places-sentence = Jedes Ergebnis zeigt die Ziffern, die es hat: alle bei einem exakten Wert, und so viele, wie seine Unsicherheit erlaubt, bei einem gemessenen oder berechneten.
ui-pointer-default-precision = Standardgenauigkeit für neue Sitzungen
ui-pointer-default-precision-words = Standardgenauigkeit, neue Sitzung, Anfangsgenauigkeit
ui-pointer-default-precision-sentence = Jede neue Sitzung beginnt gleich, damit dieselbe Eingabe auf jedem Gerät dieselbe Zahl ergibt.
ui-pointer-decimal-comma = Dezimalkomma in Rechnungen
ui-pointer-decimal-comma-words = Dezimalkomma, Dezimalpunkt, Dezimaltrennzeichen, Komma
ui-pointer-decimal-comma-sentence = Rechnungen werden mit Punkt geschrieben, damit eine Sitzung überall dasselbe bedeutet. Übungen nehmen die Schreibweise ihres Lehrplans an.
ui-pointer-door-session-bar = zur Sitzungsleiste
ui-pointer-door-session-bar-menu = zum Menü der Sitzungsleiste
ui-pointer-door-learn-landing = zur Startseite von Lernen
ui-settings = Einstellungen
ui-settings-language = Sprache
ui-settings-text-size = Textgröße
ui-settings-text-size-step = { $percent } %
ui-locale-name = Deutsch

ui-settings-units = Einheiten
ui-settings-units-not-set = Nicht gewählt
ui-settings-by-kind = nach Größenart
ui-settings-by-kind-set = { $count ->
    [one] nach Größenart: { $count } gewählt
   *[other] nach Größenart: { $count } gewählt
}
ui-settings-units-by-kind = Einheiten nach Größenart
ui-pointer-heading = Etwas anderes gesucht?
ui-search-precision = Genauigkeit dieser Sitzung
ui-search-precision-words = Genauigkeit, Stellen, exakt, f64, f32
ui-search-backend = Backend dieser Sitzung
ui-search-backend-words = Backend, CPU, SIMD, GPU, wo gerechnet wird
ui-search-language = Sprache
ui-search-language-words = Sprache, Deutsch, Englisch, Wörter, Übersetzung
ui-search-theme = Farbschema
ui-search-theme-words = Farbschema, dunkel, hell, Farben, Nacht
ui-search-text-size = Schriftgröße
ui-search-text-size-words = Schriftgröße, größer, kleiner, Zoom, Skalierung
ui-search-units = Einheiten
ui-search-units-words = Einheiten, metrisch, imperial, Fahrenheit, Celsius, Meilen, Kilometer
ui-search-go-to-input-area = Zur Eingabezeile
ui-search-go-to-input-area-words = zur Eingabezeile, tippen, schreiben
ui-search-go-to-stack = Zum Stapel
ui-search-go-to-stack-words = zum Stapel, Ergebnisse, Zeilen
ui-search-go-to-instrument-panel = Zur Instrumententafel
ui-search-go-to-instrument-panel-words = zur Tafel, Instrumente, Inspect, Method, Plot
ui-search-go-to-session-bar = Zur Sitzungsleiste
ui-search-go-to-session-bar-words = zur Sitzungsleiste, oben, Auswahl, Sitzung
ui-search-cancel-running-line = Laufende Zeile abbrechen
ui-search-cancel-running-line-words = abbrechen, stoppen, läuft
ui-search-language-reference = Sprachreferenz
ui-search-language-reference-words = Sprachreferenz Konstrukte Syntax Schreibweise Symbole Operatoren

ui-search-axis-unit = Einheit einer Bildachse
ui-search-axis-unit-words = Achseneinheit, Bildeinheit, Achse, Maßstab der Achse
ui-reference-title = Referenz

ui-search-title = Suche
ui-search-answers = Gibt es nicht
ui-stack-too-narrow = Das Fenster ist zu schmal, um eine Zeile zu zeigen.
ui-stack-too-narrow-short = Zu schmal
ui-picture-keys-pan = Pfeile verschieben
ui-picture-keys-zoom = Bild auf/ab zoomen
ui-picture-keys-value-zoom = alt+Bild auf/ab Werte zoomen
ui-picture-keys-turn = strg+Pfeile drehen
ui-picture-keys-reset = Pos1 zurücksetzen
ui-picture-keys-read = Eingabe ablesen
ui-picture-keys-range = Bereich tippen
ui-picture-keys-cursor = Pfeile bewegen den Zeiger
ui-picture-keys-layer = strg+Pfeile Ebene
ui-picture-keys-commit = Eingabe behält die Ablesung
ui-picture-keys-coordinate = Koordinate tippen
ui-picture-keys-leave = Esc zurück
ui-picture-range-from = { $axis } von
ui-picture-range-to = { $axis } bis
ui-picture-range-unit = { $axis } in
ui-picture-range-azimuth = Azimut
ui-picture-range-elevation = Höhenwinkel
ui-picture-field-not-exact = { $field } ist keine exakte Zahl
ui-picture-field-not-below = { $field } liegt nicht unter der oberen Grenze
ui-picture-field-unit-unknown = { $field } ist keine Einheit dieser Achse
ui-picture-row-not-applied = die Ansicht ließ sich aus dieser Zeile nicht setzen
ui-area-calculate = Rechnen
ui-recognized-looks-like = sieht aus wie:
ui-recognized-separator = {", "}
ui-recognized-last-separator = {", "}und{" "}
ui-recognized-more = mehr
ui-recognized-unnamed = { $count ->
    [one] einen Begriff, den diese Version nicht kennt
   *[other] { $count } Begriffe, die diese Version nicht kennt
}
ui-concept-origin = aus Zeile { $line }
ui-learn-recent = Zuletzt geöffnet
ui-learn-landing-empty = Um ein Konzept zu öffnen, geh zu Rechnen und wähle seinen Namen unter einem Ergebnis.

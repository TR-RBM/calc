error-parse-unexpected-character = unerwartetes Zeichen { $name } in Spalte { $column }
error-parse-unexpected-token = unerwartetes { $name } in Spalte { $column }
error-parse-unexpected-end = die Eingabe endet zu früh in Spalte { $column }
error-parse-invalid-typed-literal = ungültiges typisiertes Literal in Spalte { $column }
error-parse-exponent-too-large = der Exponent in { $name } ist betragsmäßig größer als { $limit }, an Spalte { $column }
error-parse-not-a-unit = { $name } ist keine Einheit in Spalte { $column }
error-parse-atomic-mass-unit = { $name } ist die atomare Masseneinheit, deren Wert in Kilogramm gemessen ist, und die Einheiten von calc haben alle exakte Faktoren; schreib sie über die Konstante m_u, wie 12 * m_u, oder 2e-26 kg / m_u für eine Masse in atomaren Masseneinheiten, in Spalte { $column }
error-parse-not-a-unit-joined-to-a-unit = { $name } ist keine Einheit in Spalte { $column }: ein Name, der ohne Leerzeichen mit * oder / an eine Einheit gehängt ist, wird als Teil der Einheit gelesen; um mit { $name } zu multiplizieren oder durch { $name } zu teilen, setze auf beide Seiten des * oder / ein Leerzeichen
error-parse-ambiguous-unit = { $name } in Spalte { $column } wird für mehr als eine Einheit geschrieben; schreib eine von { $readings }, die je eine bedeuten
error-parse-type-expected = { $name } in Spalte { $column } ist kein Ganzzahltyp; schreib einen wie u8, i16 oder u32
error-parse-byte-order-missing = bytes in Spalte { $column } braucht die Byte-Reihenfolge: schreib bytes be für das höchstwertige Byte zuerst oder bytes le für das niederwertigste zuerst
error-parse-unit-exponent-out-of-range = die Potenz von { $name } liegt außerhalb von { $lowest } bis { $highest }, an Spalte { $column }
error-parse-unit-exponent-not-whole = eine Einheit nimmt nur ganzzahlige Potenzen, und { $name } ist keine ganze Zahl, an Spalte { $column }
error-parse-reserved-name = { $name } ist ein reservierter Name in Spalte { $column }
error-parse-chained-relation = Relationen können nicht verkettet werden; mit and verbinden, in Spalte { $column }
error-parse-ragged-array = die Zeilen des Feldes sind unterschiedlich lang in Spalte { $column }
error-parse-unknown-keyword = unbekanntes Schlüsselwortargument { $name } in Spalte { $column }
error-parse-duplicate-keyword = Schlüsselwortargument { $name } ist doppelt angegeben in Spalte { $column }
error-parse-positional-after-keyword = { $name } in Spalte { $column } steht nach einem Schlüsselwortargument; schreib jedes Argument ohne Namen vor die Schlüsselwortargumente
error-parse-invalid-keyword-value = { $name } ist kein Wert des Schlüsselwortarguments { $keyword } in Spalte { $column }; { $keyword } nimmt { $values }
error-parse-missing-keyword = das Schlüsselwortargument { $keyword } fehlt in Spalte { $column }; es nimmt { $values }
error-parse-bubble-sort-form = Bubblesort wird in Formen gelehrt, die verschieden zählen, also nenne eine in Spalte { $column }: form=full macht n - 1 Durchläufe über die ganze Liste; form=shrinking beendet jeden Durchlauf vor den schon platzierten Einträgen; form=early_exit hört außerdem nach einem Durchlauf ohne Vertauschung auf; form=last_exchange beendet jeden Durchlauf bei der letzten Vertauschung des vorigen
error-parse-missing-pivot = das Schlüsselwortargument pivot fehlt in Spalte { $column }; bei dieser Aufteilung schreib pivot={ $built }
error-parse-missing-seed = pivot=random zieht seine Pivots aus philox4x32_10, und der Seed entscheidet, welche, deshalb schreib seed= mit einer ganzen Zahl von 0 bis 2^64 − 1 in Spalte { $column }
error-parse-invalid-seed = seed= nimmt eine ganze Zahl von 0 bis 2^64 − 1 in Ziffern, in Spalte { $column }
error-parse-seed-without-random-pivot = seed= in Spalte { $column } gehört zu pivot=random, und dieses Sortieren zieht keine Pivots
error-array-key-not-whole = { $method } zählt ganze Zahlen, und { $key } ist keine; nimm dafür ein Sortieren durch Vergleiche
error-array-key-not-whole-entry = { $method } zählt ganze Zahlen, und ein Schlüssel ist keine; nimm dafür ein Sortieren durch Vergleiche
error-array-key-with-unit = { $method } zählt ganze Zahlen, und { $key } hat eine Einheit, deshalb hängt es von der Einheit ab, in der gezählt wird, ob es überhaupt eine ganze Zahl ist und welche; zähl in der Einheit, die du meinst, indem du durch sie teilst, etwa durch 1 cm
error-array-key-with-unit-entry = { $method } zählt ganze Zahlen, und ein Schlüssel hat eine Einheit, deshalb hängt es von der Einheit ab, in der gezählt wird, ob es überhaupt eine ganze Zahl ist und welche; zähl in der Einheit, die du meinst, indem du durch sie teilst, etwa durch 1 cm
error-array-function-key-machine = die Schlüsselfunktion liefert { $key }, eine Maschinenzahl, und { $method } zählt nur exakte ganze Zahlen; schreib einen Schlüssel, der exakt rechnet, ohne Maschinenumwandlung
error-array-function-key-machine-entry = die Schlüsselfunktion liefert eine Maschinenzahl, und { $method } zählt nur exakte ganze Zahlen; schreib einen Schlüssel, der exakt rechnet, ohne Maschinenumwandlung
error-array-range-too-wide = die Schlüssel umfassen mehr Werte, als Countingsort Zähler vorhält
error-generator-seed-machine = der Seed von philox4x32_10 ist eine exakt geschriebene ganze Zahl, und { $value } ist eine Maschinenzahl; schreib den Seed ohne Maschinenumwandlung
error-generator-stream-machine = der Strom von philox4x32_10 ist eine exakt geschriebene ganze Zahl, und { $value } ist eine Maschinenzahl; schreib den Strom ohne Maschinenumwandlung
error-generator-index-machine = der Index von philox4x32_10 ist eine exakt geschriebene ganze Zahl, und { $value } ist eine Maschinenzahl; schreib den Index ohne Maschinenumwandlung
error-generator-seed-refused = der Seed von philox4x32_10 ist eine ganze Zahl von 0 bis 2^64 − 1 ohne Einheit, und { $value } ist keine solche
error-generator-stream-refused = der Strom von philox4x32_10 ist eine ganze Zahl von 0 bis 2^64 − 1 ohne Einheit, und { $value } ist keine solche
error-generator-index-refused = der Index von philox4x32_10 ist eine ganze Zahl von 0 bis 2^64 − 1 ohne Einheit, und { $value } ist keine solche
error-sort-range-too-wide = die Schlüssel umfassen { $range } Werte, und Countingsort hält höchstens { $limit } Zähler; nimm dafür ein Sortieren durch Vergleiche
error-parse-quick-sort-partition = Quicksort wird mit Aufteilungen gelehrt, die verschieden zählen, also nenne eine in Spalte { $column }: partition=lomuto, pivot=last durchläuft einmal von links mit dem letzten Eintrag als Pivot; partition=hoare, pivot=first läuft von beiden Enden mit dem ersten Eintrag als Pivot
error-parse-cocktail-shaker-sort-form = Cocktail-Shaker-Sort wird in Formen gelehrt, die verschieden zählen, deshalb nenn eine in Spalte { $column }: form=full läuft jedes Mal über die ganze Liste; form=shrinking beendet jeden Durchlauf einen Eintrag früher; form=last_exchange beendet jeden Durchlauf dort, wo der vorige zuletzt vertauscht hat
error-parse-odd-even-sort-form = Odd-even-Sort wird in zwei Formen gelehrt, die verschieden zählen, deshalb nenn eine in Spalte { $column }: form=until_sorted wiederholt Runden, bis eine nichts vertauscht; form=fixed_passes macht genau n Phasen
error-parse-comb-sort-form = Combsort wird mit Schrumpffaktoren gelehrt, die verschieden zählen, deshalb nenn die Form in Spalte { $column }: form=lacey_box verkleinert die Lücke um 1,3 mit der Elferregel; andere Formen gibt es noch nicht
error-parse-shell-sort-gaps = Shellsort wird mit Lückenfolgen gelehrt, die verschieden zählen, deshalb nenn eine in Spalte { $column }: gaps=shell halbiert die Lücke ab n/2; gaps=knuth nimmt (3^k − 1)/2 bis ceil(n/3); gaps=ciura nimmt 1, 4, 10, 23, 57, 132, 301, 701
error-parse-missing-base = Radixsort sortiert Ziffer um Ziffer, und die Basis bestimmt die Ziffern und jede Zählung, deshalb nenn eine in Spalte { $column }: base=10 sortiert nach Dezimalziffern, base=2 nach Binärziffern, base=256 nach Bytes
error-parse-invalid-base = base= nimmt eine ganze Zahl von 2 bis 65536 in Ziffern, in Spalte { $column }
error-parse-missing-shuffle-seed = Bogosort zieht sein Mischen aus philox4x32_10, und der Seed bestimmt es, deshalb schreib seed= mit einer ganzen Zahl von 0 bis 2^64 − 1 in Spalte { $column }
error-parse-missing-limit = Bogosort mischt, bis die Liste sortiert ist, und das hat keine Schranke, deshalb nenn mit limit= in Spalte { $column } die meisten Mischvorgänge, die du erlaubst, etwa limit=1000
error-parse-invalid-limit = limit= nimmt eine ganze Zahl von 0 bis 2^64 − 1 in Ziffern, in Spalte { $column }
error-sort-base-too-small = die Basis von Radixsort ist mindestens 2: eine Basis von 0 oder 1 hat keine Ziffern, nach denen sortiert werden kann
error-sort-base-too-large = die Basis von Radixsort ist höchstens { $limit }, weil jeder Durchlauf einen Zähler für jeden Ziffernwert hält
error-sort-bead-no-key = Beadsort baut jeden Wert aus seinen Perlen neu und bewegt keinen Eintrag, deshalb kann es keinen Eintrag über seinen Schlüssel mitnehmen; sortier die Zahlen selbst mit bead_sort(a), oder nimm ein Sortieren, das Einträge bewegt
error-sort-limit-reached = Bogosort hat bei seinem Limit von { $limit } Mischvorgängen angehalten, ohne zu sortieren, deshalb gibt es keine Liste zu zeigen; bis dahin gezählt: Vergleiche { $comparisons }, Schreibvorgänge { $writes }, Ziehungen { $draws }; es endet mit Wahrscheinlichkeit 1, und ein größeres limit= oder ein anderer seed= lässt es weiterlaufen
error-sort-length-not-power-of-two = Bitonic Sort ist ein Netz für 2^k Einträge, und diese Liste hat { $length }, zwischen { $below } und { $above }; eine Form für andere Längen gibt es noch nicht, deshalb nimm für diese Liste ein Sortieren wie merge_sort
error-sort-bead-below-zero = Beadsort legt jede Zahl als so viele Perlen aus, und { $key } ist kleiner als 0, hat also keine Perlen zum Auslegen; nimm dafür radix_sort oder ein Sortieren durch Vergleiche
error-sort-too-many-beads = Beadsort bewegt jede Perle, eine für jede Einheit jeder Zahl, und diese Zahlen haben { $beads } Perlen, mehr als die { $limit }, die es bewegt; nimm dafür radix_sort oder counting_sort
error-parse-pivot-not-built-for-partition = pivot={ $name } in Spalte { $column } gibt es für diese Aufteilung nicht: Quicksort paart partition=lomuto mit pivot=last oder pivot=random und partition=hoare mit pivot=first, andere Pivots gibt es noch nicht
error-parse-missing-differential = dem Integral fehlt das Differential, etwa dx in Spalte { $column }
error-parse-nested-too-deeply = der Ausdruck geht mehr als { $limit } Ebenen tief; in benannte Zeilen aufteilen, in Spalte { $column }
error-parse-chain-too-long = der Ausdruck verknüpft mehr als { $limit } Operationen in einer Kette; in benannte Zeilen aufteilen, in Spalte { $column }
error-parse-expression-too-deep = Klammern und Ketten zusammen bringen diesen Ausdruck mehr als { $limit } Ebenen tief; in benannte Zeilen aufteilen, in Spalte { $column }
error-parse-ambiguous-application = { $written } kann { $narrow } oder { $wide } bedeuten; schreib das, was du meinst, in Spalte { $column }
error-parse-ambiguous-temperature-sign = { $written } kann die Temperatur meinen, geschrieben { $reading }, oder eine Differenz von so vielen Grad, geschrieben { $difference }; schreib das, was du meinst, in Spalte { $column }
error-parse-fraction-before-unit = { $written } kann { $fraction } oder { $reciprocal } bedeuten; schreib das, was du meinst, in Spalte { $column }
error-parse-percent-in-a-sum = { $written } kann { $literal } oder { $relative } bedeuten; schreib das, was du meinst, in Spalte { $column }
error-parse-attempt-degree-after-name = ° steht nach einer Zahl, einem Messwert, einer Klammer oder einer Konstante wie pi, nach keinem anderen Namen: für einen Winkel in Grad schreib { $corrected }, in Spalte { $column }
error-parse-attempt-degree-after-name-plain = ° steht nach einer Zahl, einem Messwert, einer Klammer oder einer Konstante wie pi, nach keinem anderen Namen: für einen Winkel in Grad multipliziere mit 1°, in Spalte { $column }
error-parse-attempt-double-star-power = eine Potenz wird mit ^ geschrieben, also { $corrected }, in Spalte { $column }
error-parse-attempt-comma-between-digits = der Dezimaltrenner wird hier mit . geschrieben, also { $corrected }, in Spalte { $column }
error-parse-attempt-grouping-comma = Zahlen werden ohne Tausenderkomma geschrieben, und der Dezimaltrenner ist ., etwa 1000 oder 2.5, in Spalte { $column }
error-parse-attempt-absolute-value-bars = Betragsstriche werden mit abs geschrieben, also { $corrected }, in Spalte { $column }
error-parse-attempt-times-sign = das Multiplizieren von Zahlen wird mit * oder · geschrieben, also { $corrected }, in Spalte { $column }
error-parse-attempt-division-sign = die Division wird mit / geschrieben, also { $corrected }, in Spalte { $column }
error-parse-attempt-double-star-power-plain = eine Potenz wird mit ^ geschrieben, etwa 2^3, in Spalte { $column }
error-parse-attempt-comma-between-digits-plain = der Dezimaltrenner wird hier mit . geschrieben, etwa 2.5, in Spalte { $column }
error-parse-attempt-absolute-value-bars-plain = Betragsstriche werden mit abs geschrieben, etwa abs(x - 3), in Spalte { $column }
error-parse-attempt-times-sign-plain = das Multiplizieren von Zahlen wird mit * oder · geschrieben, etwa 2 * 3, in Spalte { $column }
error-parse-attempt-division-sign-plain = die Division wird mit / geschrieben, etwa 6 / 3, in Spalte { $column }
error-parse-attempt-colon-or-time = ein Doppelpunkt wird nicht verwendet: die Division wird mit / geschrieben, etwa 80 / 20, und eine Uhrzeit ist hier keine Zahl, in Spalte { $column }
error-parse-arity-mismatch = { $expected ->
    [one] { $expected } Argument
   *[other] { $expected } Argumente
} erwartet, { $found } gefunden, in Spalte { $column }
error-name-kind-conflict = der Name wird schon für eine andere Art von Namen verwendet

error-expression-too-large = der Ausdruck ist zu groß
error-result-too-large = { $reading } hätte mindestens { $digits } Stellen; die Grenze liegt bei etwa { $limit_digits } Stellen ({ $limit } Bit)
error-intermediate-step-too-large = der Zwischenschritt { $reading } hätte mindestens { $digits } Stellen; die Grenze liegt bei etwa { $limit_digits } Stellen ({ $limit } Bit), daher endete die Auswertung dort
error-undefined-name = { $name } ist nicht definiert
error-undefined-unnamed-generator = calc hat kein { $name }: welcher Generator eine Zahl zieht, entscheidet über die Zahl, deshalb wird ein Generator mit seinem Namen verlangt; schreib philox4x32_10(seed, stream, index)
error-undefined-name-constant = { $name } ist nicht definiert; die physikalische Konstante wird { $constant } geschrieben
error-input-not-parsed = die Eingabe lässt sich nicht mehr lesen, in Spalte { $column }
error-unit-ended-at-space = die Einheit endet am Leerzeichen vor dem Rechenzeichen, darum wird { $name } als Name gelesen und nicht als Einheit: schreib die Einheit ohne Leerzeichen als { $replacement }, etwa { $corrected }, in Spalte { $column }
error-unit-ended-at-space-plain = die Einheit endet am Leerzeichen vor dem Rechenzeichen, darum wird { $name } als Name gelesen und nicht als Einheit: schreib die Einheit ohne Leerzeichen als { $replacement }, in Spalte { $column }
error-reference-cycle = { $line } würde von sich selbst abhängen
error-name-exists = { $name } existiert bereits
error-name-in-use = { $line } kann nicht umbenannt werden, weil { $dependents } es verwenden
error-unknown-line = { $line } existiert nicht
error-line-numbers-exhausted = die Sitzung hat keine freien Zeilennummern
error-dependency-failed = { $line } hat kein Ergebnis
error-precision-not-supported = die Genauigkeit { $precision } wird noch nicht unterstützt
error-result-kind-not-representable = ein Ergebnis der Art { $kind } kann noch nicht angezeigt werden
error-result-not-representable = das Ergebnis kann nicht dargestellt werden
error-division-by-zero = { $reading } teilt durch null
error-relation-is-a-claim = { $reading } ist eine Aussage, kein Wert: sie wird entschieden, nicht berechnet
error-relation-about-free-names = { $reading } ist eine Aussage über { $names }, kein Wert: sie wird entschieden oder gelöst, nicht berechnet
error-outside-domain = { $reading } ist nicht definiert
error-not-in-radical-field = { $operator } zerlegt einen Wert aus rationalen Zahlen und Quadratwurzeln, und { $reading } ist keiner
error-remainder-not-computed = calc berechnet { $reading } nur, wo beide Argumente rationale Zahlen oder Quadratwurzeln daraus sind; für andere Werte berechnet es das noch nicht
error-power-of-zero-without-value = { $reading } hat keinen Wert: eine Potenz von 0 hat nur dann einen Wert, wenn der Realteil ihres Exponenten größer als 0 ist, und hier ist der Realteil 0
error-power-of-zero-sign-undecided = { $reading } hat erst einen Wert, wenn das Vorzeichen des Exponenten bekannt ist: 0 hoch eine Zahl über 0 ist 0, hoch eine Zahl unter 0 teilt es durch null, und hoch 0 ist es 1; calc konnte den Exponenten weder als größer als 0 noch als kleiner als 0 beweisen
error-not-a-square-root-term = das zweite Argument von coefficient_of ist sqrt(n), mit einer ganzen Zahl n über 1, die durch kein Quadrat außer 1 teilbar ist, etwa sqrt(2); { $reading } ist keines
error-not-a-square-root-term-multiple = das zweite Argument von coefficient_of ist sqrt(n), mit einer ganzen Zahl n über 1, die durch kein Quadrat außer 1 teilbar ist; { $reading } ist ein Vielfaches von sqrt({ $radicand }), also frag nach sqrt({ $radicand })
error-tolerance-written-twice = { $reading } steht in dieser Zeile mehr als einmal, und calc kann nicht erkennen, ob es ein Bauteil ist oder mehrere: gib ihm einen Namen, wie R = { $reading }, und verwende den Namen, wo es dasselbe Bauteil ist, oder benenne jedes Bauteil in einer eigenen Zeile
error-worst-case-too-many = eine Zeile darf höchstens { $limit } Werte mit Toleranz tragen
error-worst-case-unsupported = calc kann den ungünstigsten Fall dieser Zeile nicht bestimmen
error-worst-case-endpoint-not-exact = die Enden von { $reading } müssen exakte Werte sein
error-worst-case-endpoints-reversed = bei { $reading } liegt das untere Ende über dem oberen
error-worst-case-negative-tolerance = die Toleranz in { $reading } ist negativ; eine Toleranz ist ein Anteil ab 0
error-worst-case-not-monotone = calc kann nicht zeigen, dass die Zeile nur steigt oder nur fällt, während { $reading } seinen Bereich durchläuft, darum gibt es keinen ungünstigsten Fall aus statt eines, der falsch sein könnte
error-worst-case-divisor-reaches-zero = { $reading } teilt durch einen Wert, der null werden kann, während seine Bereiche ihre Enden durchlaufen, oder bei dem calc nicht zeigen kann, dass er von null wegbleibt
error-worst-case-range-in-function-body = der Körper von { $name } enthält einen Bereich, der für alle Aufrufe ein Teil oder pro Aufruf ein eigener Teil sein kann; gib dem Bereich einen Namen und verwende ihn in { $name }, oder übergib den Bereich { $name } als Argument
error-worst-case-ends-of-two-dimensions = die beiden Enden von { $reading } messen verschiedene Größen; beide Enden brauchen dieselbe Einheit oder Einheiten derselben Größe
error-worst-case-endpoint-not-enclosed = calc kann die Enden von { $reading } nicht eng genug eingrenzen, um den ungünstigsten Fall zu bestimmen; ein Ende wie sqrt(2) wird noch nicht unterstützt
error-integer-not-whole = { $reading } braucht eine ganze Zahl, und diese ist keine
error-integer-negative = { $reading } braucht eine Zahl ab 0: eine negative Zahl hat Bits nur bei fester Breite, und wrap(x, u32) gibt sie als Zahl ab 0
error-integer-shift-too-large = { $reading } schiebt um mehr als { $limit } Stellen oder um eine negative Zahl von Stellen
error-integer-not-whole-bytes = { $reading }: { $kind } ist keine ganze Zahl von Bytes
error-integer-outside-type = { $reading } liegt außerhalb von { $kind }, das { $low } bis { $high } fasst; wrap(x, { $kind }) bringt es absichtlich in { $kind }
error-integer-argument-out-of-range = in { $reading } ist das Argument { $argument } größer als das größte erlaubte Argument { $limit }
error-unsupported-operator = { $reading } kann noch nicht ausgewertet werden
error-unsupported-expression = dieser Ausdruck kann noch nicht ausgewertet werden
error-unsupported-subexpression = { $reading } kann noch nicht ausgewertet werden
error-unsupported-constant = die Konstante { $reading } kann nicht exakt ausgewertet werden
error-not-finite = { $reading } ist nicht endlich
error-machine-number-in-exact = die Maschinenzahl { $reading } kann nicht exakt ausgewertet werden
error-quantity-not-converted = die Größe muss zuerst in eine Einheit umgerechnet werden
error-dimension-mismatch = { $reading } verbindet Größen verschiedener Dimension
error-dimensioned-argument = { $reading } braucht ein dimensionsloses Argument
error-dimensioned-power-exponent-not-constant = der Exponent in { $reading } ist keine Konstante, und seine Basis hat eine Einheit
error-fractional-dimension = { $reading } nimmt eine gebrochene Potenz einer Einheit
error-dimension-out-of-range = die Einheitenpotenzen in { $reading } liegen außerhalb von { $lowest } bis { $highest }
error-empty-range = der Bereich ist leer
error-index-range-too-long = der Indexbereich von { $reading } hat mehr Glieder, als gezählt werden können

error-no-backend = kein Backend kann diese Auswertung ausführen
error-backend-not-registered = das Backend { $backend } ist nicht verfügbar
error-backend-rejected = das Backend { $backend } kann diese Auswertung nicht ausführen
error-out-of-device-memory = dem Gerät fehlt Speicher
error-device-lost = das Gerät ist verloren gegangen

error-cancelled = die Auswertung wurde abgebrochen
error-not-applicable = { $instrument } gilt nicht für diese Zeile
error-renderer-unavailable = Grafiken können im Terminal noch nicht gezeichnet werden
error-internal = interner Fehler { $code }

error-file-not-found = { $path } existiert nicht
error-file-permission-denied = keine Berechtigung für { $path }
error-file-unreadable = { $path } kann nicht gelesen werden
error-file-unwritable = { $path } kann nicht geschrieben werden
error-session-invalid = { $path } ist an { $member } keine gültige Sitzungsdatei
error-session-not-json = { $path } ist keine JSON-Sitzungsdatei; eine Datei mit Ausdrücken, einem je Zeile, läuft mit calc --batch < { $path }
error-session-dependency-cycle = { $path } hat einen Abhängigkeitszyklus über { $line }
error-session-newer-version = { $path } hat Formatversion { $found }, dieser Calculator liest Version { $supported }

error-invalid-request = die Anfrage ist bei { $path } ungültig: { $code }
error-not-a-solve-line = { $line } ist keine Suchzeile
error-solve-failed = die Suche nach Gegeben und Gesucht ist fehlgeschlagen

error-solve-missing-wanted = gesucht: nenne die gesuchte Größe
error-solve-unknown-wanted = gesucht: { $name } ist keine bekannte Größe
error-solve-unknown-given-quantity = gegeben { $row }: { $name } ist keine bekannte Größe
error-solve-unknown-object = gegeben { $row }: { $name } ist kein bekanntes Objekt
error-solve-malformed-given = gegeben { $row }: { $text } ist kein Wert mit optionaler Unsicherheit und Einheit
error-solve-empty-criterion = Kriterium: wähle mindestens ein Kriterium
error-cockpit-no-display = keine Anzeige gefunden: das Cockpit braucht eine grafische Sitzung mit gesetztem DISPLAY oder WAYLAND_DISPLAY, und der Befehl calc funktioniert auch ohne
error-cockpit-window-system-refused = das Fenstersystem hat den Start des Cockpits abgelehnt: { $reason }
error-cockpit-window-not-opened = das Cockpit-Fenster konnte nicht geöffnet werden ({ $code })
error-cockpit-presentation-failed = das Cockpit konnte sein Bild nicht im Fenster zeigen ({ $code })
error-cockpit-fonts-unreadable = die mitgelieferten Schriften konnten nicht gelesen werden ({ $code })
error-cockpit-scale-unusable = die Fensterskalierung ist nicht verwendbar ({ $code })
error-cockpit-canvas-failed = das Cockpit konnte seine Zeichenfläche nicht anlegen ({ $code })
error-cockpit-drawing-failed = das Cockpit konnte sein Fenster nicht zeichnen ({ $code })
error-negative-uncertainty = die Unsicherheit in { $reading } ist negativ
error-nested-uncertainty = { $reading } gibt einer Unsicherheit eine eigene Unsicherheit
error-coverage-factor-below-one = der Erweiterungsfaktor in { $reading } ist kleiner als 1
error-uncertainty-not-propagated = die Unsicherheit lässt sich durch diesen Ausdruck nicht fortpflanzen
error-division-by-zero-plain = Division durch null
error-outside-domain-plain = { $operator } ist für dieses Argument nicht definiert
error-not-in-radical-field-plain = { $operator } zerlegt nur einen Wert aus rationalen Zahlen und Quadratwurzeln
error-remainder-not-computed-plain = calc berechnet mod nur, wo beide Argumente rationale Zahlen oder Quadratwurzeln daraus sind
error-power-of-zero-without-value-plain = eine Potenz von 0 hat nur dann einen Wert, wenn der Realteil ihres Exponenten größer als 0 ist, und hier ist der Realteil 0
error-power-of-zero-sign-undecided-plain = eine Potenz von 0 hat erst einen Wert, wenn das Vorzeichen des Exponenten bekannt ist, und calc konnte den Exponenten weder als größer als 0 noch als kleiner als 0 beweisen
error-not-a-square-root-term-plain = das zweite Argument von coefficient_of ist sqrt(n), mit einer ganzen Zahl n über 1, die durch kein Quadrat außer 1 teilbar ist, etwa sqrt(2)
error-domain-sqrt = sqrt braucht eine Zahl ab 0, und { $operand } ist kleiner als 0
error-domain-ln = ln braucht eine Zahl größer als 0, und { $operand } ist nicht größer als 0
error-domain-sqrt-complex = sqrt braucht eine Zahl ab 0, und { $operand } ist kleiner als 0; für die komplexe Wurzel schreibe i * sqrt({ $magnitude })
error-domain-ln-complex = ln braucht eine Zahl größer als 0, und { $operand } ist nicht größer als 0; für den komplexen Logarithmus schreibe ln({ $magnitude }) + i*pi
error-domain-arc-sine = { $operator } braucht eine Zahl von -1 bis 1, und { $operand } liegt außerhalb
error-domain-tan = tan ist bei { $operand } nicht definiert, einem ungeraden Vielfachen von pi / 2
error-domain-factorial = ! braucht eine ganze Zahl ab 0, und { $operand } ist keine
error-domain-power = eine Potenz, deren Exponent keine ganze Zahl ist, wird nur für eine Basis größer als 0 berechnet, und { $base } ist nicht größer als 0 (Exponent { $exponent })
error-integer-argument-out-of-range-plain = das Argument von { $operator } ist zu groß
error-exponent-out-of-range = der Exponent { $exponent } ist größer als der größte erlaubte Exponent { $limit }
error-exponent-out-of-range-long = der Exponent { $exponent } hat { $digits } Stellen und ist größer als der größte erlaubte Exponent { $limit }
error-root-degree-out-of-range = der Exponent { $exponent } verlangt eine Wurzel vom Grad { $degree }, größer als der größte erlaubte Grad { $limit }
error-root-degree-out-of-range-long = der Exponent { $exponent } verlangt eine Wurzel, deren Grad { $digits } Stellen hat, größer als der größte erlaubte Grad { $limit }
error-factorial-out-of-range = die Zahl { $operand } vor ! ist größer als die größte erlaubte Zahl vor !, { $limit }
error-factorial-out-of-range-long = die Zahl { $operand } vor ! hat { $digits } Stellen und ist größer als die größte erlaubte Zahl vor !, { $limit }
error-number-too-large = { $reading } bräuchte etwa { $digits } Stellen; die Grenze liegt bei etwa { $limit_digits } Stellen ({ $limit } Bit)
error-result-too-large-plain = das Ergebnis ist zu groß
error-unsupported-operator-plain = { $operator } kann noch nicht ausgewertet werden
error-unsupported-constant-plain = diese Konstante kann nicht exakt ausgewertet werden
error-not-finite-plain = das Ergebnis ist nicht endlich
error-machine-number-in-exact-plain = eine Maschinenzahl kann nicht exakt ausgewertet werden
error-index-range-too-long-plain = ein Indexbereich hat mehr Glieder, als gezählt werden können
error-dimension-mismatch-plain = der Ausdruck verbindet Größen verschiedener Dimension
error-dimension-mismatch-sides = { $reading }: { $left_quantity ->
    [0] eine Seite ist eine reine Zahl
    [1] { $left_unit } misst Länge (Einheiten: { $left_units })
    [2] { $left_unit } misst Masse (Einheiten: { $left_units })
    [3] { $left_unit } misst Zeit (Einheiten: { $left_units })
    [4] { $left_unit } misst elektrische Stromstärke (Einheiten: { $left_units })
    [5] { $left_unit } misst Temperatur (Einheiten: { $left_units })
    [6] { $left_unit } misst Stoffmenge (Einheiten: { $left_units })
    [7] { $left_unit } misst Lichtstärke oder Lichtstrom (Einheiten: { $left_units })
    [8] { $left_unit } misst Information (Einheiten: { $left_units })
    [9] { $left_unit } misst Fläche (Einheiten: { $left_units })
    [10] { $left_unit } misst Volumen (Einheiten: { $left_units })
    [11] { $left_unit } misst Geschwindigkeit (Einheiten: { $left_units })
    [12] { $left_unit } misst Beschleunigung (Einheiten: { $left_units })
    [13] { $left_unit } misst Kraft (Einheiten: { $left_units })
    [14] { $left_unit } misst Druck (Einheiten: { $left_units })
    [15] { $left_unit } misst Energie oder Drehmoment (Einheiten: { $left_units })
    [16] { $left_unit } misst Leistung (Einheiten: { $left_units })
    [17] { $left_unit } misst Frequenz oder Aktivität (Einheiten: { $left_units })
    [18] { $left_unit } misst elektrische Ladung (Einheiten: { $left_units })
    [19] { $left_unit } misst elektrische Spannung (Einheiten: { $left_units })
    [20] { $left_unit } misst Kapazität (Einheiten: { $left_units })
    [21] { $left_unit } misst elektrischen Widerstand (Einheiten: { $left_units })
    [22] { $left_unit } misst elektrischen Leitwert (Einheiten: { $left_units })
    [23] { $left_unit } misst magnetischen Fluss (Einheiten: { $left_units })
    [24] { $left_unit } misst magnetische Flussdichte (Einheiten: { $left_units })
    [25] { $left_unit } misst Induktivität (Einheiten: { $left_units })
    [26] { $left_unit } misst Datenrate (Einheiten: { $left_units })
    [27] { $left_unit } misst Dichte (Einheiten: { $left_units })
    [28] { $left_unit } misst Energiedosis oder Äquivalentdosis (Einheiten: { $left_units })
    [29] { $left_unit } misst Beleuchtungsstärke (Einheiten: { $left_units })
    [30] { $left_unit } misst katalytische Aktivität (Einheiten: { $left_units })
   *[other] { $left_unit } misst eine Größe, für die calc keinen Namen hat (Einheiten: { $left_units })
} und { $right_quantity ->
    [0] eine Seite ist eine reine Zahl
    [1] { $right_unit } misst Länge (Einheiten: { $right_units })
    [2] { $right_unit } misst Masse (Einheiten: { $right_units })
    [3] { $right_unit } misst Zeit (Einheiten: { $right_units })
    [4] { $right_unit } misst elektrische Stromstärke (Einheiten: { $right_units })
    [5] { $right_unit } misst Temperatur (Einheiten: { $right_units })
    [6] { $right_unit } misst Stoffmenge (Einheiten: { $right_units })
    [7] { $right_unit } misst Lichtstärke oder Lichtstrom (Einheiten: { $right_units })
    [8] { $right_unit } misst Information (Einheiten: { $right_units })
    [9] { $right_unit } misst Fläche (Einheiten: { $right_units })
    [10] { $right_unit } misst Volumen (Einheiten: { $right_units })
    [11] { $right_unit } misst Geschwindigkeit (Einheiten: { $right_units })
    [12] { $right_unit } misst Beschleunigung (Einheiten: { $right_units })
    [13] { $right_unit } misst Kraft (Einheiten: { $right_units })
    [14] { $right_unit } misst Druck (Einheiten: { $right_units })
    [15] { $right_unit } misst Energie oder Drehmoment (Einheiten: { $right_units })
    [16] { $right_unit } misst Leistung (Einheiten: { $right_units })
    [17] { $right_unit } misst Frequenz oder Aktivität (Einheiten: { $right_units })
    [18] { $right_unit } misst elektrische Ladung (Einheiten: { $right_units })
    [19] { $right_unit } misst elektrische Spannung (Einheiten: { $right_units })
    [20] { $right_unit } misst Kapazität (Einheiten: { $right_units })
    [21] { $right_unit } misst elektrischen Widerstand (Einheiten: { $right_units })
    [22] { $right_unit } misst elektrischen Leitwert (Einheiten: { $right_units })
    [23] { $right_unit } misst magnetischen Fluss (Einheiten: { $right_units })
    [24] { $right_unit } misst magnetische Flussdichte (Einheiten: { $right_units })
    [25] { $right_unit } misst Induktivität (Einheiten: { $right_units })
    [26] { $right_unit } misst Datenrate (Einheiten: { $right_units })
    [27] { $right_unit } misst Dichte (Einheiten: { $right_units })
    [28] { $right_unit } misst Energiedosis oder Äquivalentdosis (Einheiten: { $right_units })
    [29] { $right_unit } misst Beleuchtungsstärke (Einheiten: { $right_units })
    [30] { $right_unit } misst katalytische Aktivität (Einheiten: { $right_units })
   *[other] { $right_unit } misst eine Größe, für die calc keinen Namen hat (Einheiten: { $right_units })
}, darum lassen sie sich weder addieren noch vergleichen noch ineinander umrechnen{ $suggestion_count ->
    [0] .
   *[other] . Meintest du { $suggestion }?
}
error-sort-keys-differ-in-dimension = { $first } an Stelle { $first_position } und { $second } an Stelle { $second_position } lassen sich nicht in eine Reihenfolge bringen: { $left_quantity ->
    [0] eine Seite ist eine reine Zahl
    [1] { $left_unit } misst Länge
    [2] { $left_unit } misst Masse
    [3] { $left_unit } misst Zeit
    [4] { $left_unit } misst elektrische Stromstärke
    [5] { $left_unit } misst Temperatur
    [6] { $left_unit } misst Stoffmenge
    [7] { $left_unit } misst Lichtstärke oder Lichtstrom
    [8] { $left_unit } misst Information
    [9] { $left_unit } misst Fläche
    [10] { $left_unit } misst Volumen
    [11] { $left_unit } misst Geschwindigkeit
    [12] { $left_unit } misst Beschleunigung
    [13] { $left_unit } misst Kraft
    [14] { $left_unit } misst Druck
    [15] { $left_unit } misst Energie oder Drehmoment
    [16] { $left_unit } misst Leistung
    [17] { $left_unit } misst Frequenz oder Aktivität
    [18] { $left_unit } misst elektrische Ladung
    [19] { $left_unit } misst elektrische Spannung
    [20] { $left_unit } misst Kapazität
    [21] { $left_unit } misst elektrischen Widerstand
    [22] { $left_unit } misst elektrischen Leitwert
    [23] { $left_unit } misst magnetischen Fluss
    [24] { $left_unit } misst magnetische Flussdichte
    [25] { $left_unit } misst Induktivität
    [26] { $left_unit } misst Datenrate
    [27] { $left_unit } misst Dichte
    [28] { $left_unit } misst Energiedosis oder Äquivalentdosis
    [29] { $left_unit } misst Beleuchtungsstärke
    [30] { $left_unit } misst katalytische Aktivität
   *[other] { $left_unit } misst eine Größe, für die calc keinen Namen hat
} und { $right_quantity ->
    [0] eine Seite ist eine reine Zahl
    [1] { $right_unit } misst Länge
    [2] { $right_unit } misst Masse
    [3] { $right_unit } misst Zeit
    [4] { $right_unit } misst elektrische Stromstärke
    [5] { $right_unit } misst Temperatur
    [6] { $right_unit } misst Stoffmenge
    [7] { $right_unit } misst Lichtstärke oder Lichtstrom
    [8] { $right_unit } misst Information
    [9] { $right_unit } misst Fläche
    [10] { $right_unit } misst Volumen
    [11] { $right_unit } misst Geschwindigkeit
    [12] { $right_unit } misst Beschleunigung
    [13] { $right_unit } misst Kraft
    [14] { $right_unit } misst Druck
    [15] { $right_unit } misst Energie oder Drehmoment
    [16] { $right_unit } misst Leistung
    [17] { $right_unit } misst Frequenz oder Aktivität
    [18] { $right_unit } misst elektrische Ladung
    [19] { $right_unit } misst elektrische Spannung
    [20] { $right_unit } misst Kapazität
    [21] { $right_unit } misst elektrischen Widerstand
    [22] { $right_unit } misst elektrischen Leitwert
    [23] { $right_unit } misst magnetischen Fluss
    [24] { $right_unit } misst magnetische Flussdichte
    [25] { $right_unit } misst Induktivität
    [26] { $right_unit } misst Datenrate
    [27] { $right_unit } misst Dichte
    [28] { $right_unit } misst Energiedosis oder Äquivalentdosis
    [29] { $right_unit } misst Beleuchtungsstärke
    [30] { $right_unit } misst katalytische Aktivität
   *[other] { $right_unit } misst eine Größe, für die calc keinen Namen hat
}, darum lassen sie sich weder addieren noch vergleichen noch ineinander umrechnen.
error-sort-keyed-keys-differ-in-dimension = der Schlüssel { $first } des Eintrags an Stelle { $first_position } und der Schlüssel { $second } des Eintrags an Stelle { $second_position } lassen sich nicht in eine Reihenfolge bringen: { $left_quantity ->
    [0] eine Seite ist eine reine Zahl
    [1] { $left_unit } misst Länge
    [2] { $left_unit } misst Masse
    [3] { $left_unit } misst Zeit
    [4] { $left_unit } misst elektrische Stromstärke
    [5] { $left_unit } misst Temperatur
    [6] { $left_unit } misst Stoffmenge
    [7] { $left_unit } misst Lichtstärke oder Lichtstrom
    [8] { $left_unit } misst Information
    [9] { $left_unit } misst Fläche
    [10] { $left_unit } misst Volumen
    [11] { $left_unit } misst Geschwindigkeit
    [12] { $left_unit } misst Beschleunigung
    [13] { $left_unit } misst Kraft
    [14] { $left_unit } misst Druck
    [15] { $left_unit } misst Energie oder Drehmoment
    [16] { $left_unit } misst Leistung
    [17] { $left_unit } misst Frequenz oder Aktivität
    [18] { $left_unit } misst elektrische Ladung
    [19] { $left_unit } misst elektrische Spannung
    [20] { $left_unit } misst Kapazität
    [21] { $left_unit } misst elektrischen Widerstand
    [22] { $left_unit } misst elektrischen Leitwert
    [23] { $left_unit } misst magnetischen Fluss
    [24] { $left_unit } misst magnetische Flussdichte
    [25] { $left_unit } misst Induktivität
    [26] { $left_unit } misst Datenrate
    [27] { $left_unit } misst Dichte
    [28] { $left_unit } misst Energiedosis oder Äquivalentdosis
    [29] { $left_unit } misst Beleuchtungsstärke
    [30] { $left_unit } misst katalytische Aktivität
   *[other] { $left_unit } misst eine Größe, für die calc keinen Namen hat
} und { $right_quantity ->
    [0] eine Seite ist eine reine Zahl
    [1] { $right_unit } misst Länge
    [2] { $right_unit } misst Masse
    [3] { $right_unit } misst Zeit
    [4] { $right_unit } misst elektrische Stromstärke
    [5] { $right_unit } misst Temperatur
    [6] { $right_unit } misst Stoffmenge
    [7] { $right_unit } misst Lichtstärke oder Lichtstrom
    [8] { $right_unit } misst Information
    [9] { $right_unit } misst Fläche
    [10] { $right_unit } misst Volumen
    [11] { $right_unit } misst Geschwindigkeit
    [12] { $right_unit } misst Beschleunigung
    [13] { $right_unit } misst Kraft
    [14] { $right_unit } misst Druck
    [15] { $right_unit } misst Energie oder Drehmoment
    [16] { $right_unit } misst Leistung
    [17] { $right_unit } misst Frequenz oder Aktivität
    [18] { $right_unit } misst elektrische Ladung
    [19] { $right_unit } misst elektrische Spannung
    [20] { $right_unit } misst Kapazität
    [21] { $right_unit } misst elektrischen Widerstand
    [22] { $right_unit } misst elektrischen Leitwert
    [23] { $right_unit } misst magnetischen Fluss
    [24] { $right_unit } misst magnetische Flussdichte
    [25] { $right_unit } misst Induktivität
    [26] { $right_unit } misst Datenrate
    [27] { $right_unit } misst Dichte
    [28] { $right_unit } misst Energiedosis oder Äquivalentdosis
    [29] { $right_unit } misst Beleuchtungsstärke
    [30] { $right_unit } misst katalytische Aktivität
   *[other] { $right_unit } misst eine Größe, für die calc keinen Namen hat
}, darum lassen sie sich weder addieren noch vergleichen noch ineinander umrechnen.
error-dimensioned-argument-plain = eine Funktion braucht ein dimensionsloses Argument
error-dimensioned-power-exponent-not-constant-plain = ein Exponent ist keine Konstante, und seine Basis hat eine Einheit
error-fractional-dimension-plain = der Ausdruck nimmt eine gebrochene Potenz einer Einheit
error-dimension-out-of-range-plain = die Einheitenpotenzen im Ausdruck liegen außerhalb des erlaubten Bereichs
error-negative-uncertainty-plain = eine Unsicherheit ist negativ
error-nested-uncertainty-plain = eine Unsicherheit hat selbst eine Unsicherheit
error-coverage-factor-below-one-plain = der Erweiterungsfaktor ist kleiner als 1
error-holds-free-names = { $line } enthält den freien Namen { $names } und hat darum keinen einzelnen Wert, den diese Ansicht zeigen könnte; eine Zeile, die { $names } einen Wert gibt, ergibt einen
error-machine-line-not-applicable = { $line } lässt sich nicht als Maschinenzeile auswerten: Maschinenauswertung nimmt eine Ausdrucks- oder Benennungszeile mit Ergebnis
error-digits-not-applicable = { $line } hat keine Nachkommastellen-Ansicht: sie nimmt einen exakten Bruch oder einen endlichen Maschinenwert
error-enclose-not-applicable = { $line } hat keinen Einschluss: er nimmt einen reellen Wert ohne Unsicherheit, dessen Operationen alle einen bewiesenen Einschluss haben
error-enclosure-call-not-applicable = das nimmt keinen Einschluss: es muss ein reeller Wert ohne Unsicherheit sein, dessen Operationen alle einen bewiesenen Einschluss haben
error-enclosure-digits-not-whole = die Anzahl signifikanter Stellen muss eine ganze Zahl von 1 bis 5000 sein
error-working-not-applicable = { $line } hat keinen Rechenweg: dafür braucht es eine Zeile mit einer Rechenoperation, deren Ergebnis berechnet wurde
error-places-above-limit = { $places } Stellen liegen über der Grenze von { $limit } Stellen
error-significant-digits-out-of-range = { $digits } signifikante Stellen liegen außerhalb von 1 bis { $limit }
error-enclosure-over-budget = der Einschluss braucht Zahlen breiter als { $bits } Bit und endete dort
error-digits-undetermined = Stelle { $places } konnte nicht entschieden werden: der Wert wurde auf etwa { $digits } signifikante Stellen verfeinert und das Intervall umschließt diese Stelle weiterhin
error-unknown-label = { $line } bezeichnet keine Zeile
error-plot-not-plottable = { $line } lässt sich nicht zeichnen: ein Bild nimmt eine Ausdrucks-, Benennungs- oder Funktionszeile mit einer freien Variablen
error-plot-view-count = { $found } Ansichten sind angegeben, und das Bild hat { $expected }
error-plot-view-axis-count = { $found } Achsen sind angegeben, und das Bild hat { $expected }
error-plot-unknown-unit = die Einheit der Achse { $axis } ist keine Einheit
error-plot-unit-of-other-dimension = die Einheit der Achse { $axis } misst eine andere Größe als die Achse
error-plot-empty-interval = der Bereich der Achse { $axis } hat eine obere Grenze, die nicht über der unteren liegt
error-plot-unknown-parameter = { $name } ist keine freie Variable der Zeile
error-plot-divisions-missing = die Achse { $axis } hat keine Teilung
error-plot-sampling-failed = das Bild ließ sich nicht abtasten ({ $code })
error-render-failed = das Bild ließ sich nicht zeichnen ({ $code })
error-render-text-does-not-fit = die Wörter des Bildes passen nicht in ein Bild von { $width } mal { $height } Pixeln
error-image-not-encoded = das Bild ließ sich nicht kodieren ({ $code })
error-plot-view-kind-mismatch = { $line } hat eine Achsenvariable und wird deshalb in einer ebenen Ansicht gezeichnet, nicht in einer räumlichen
error-plot-iteration-limit-not-applicable = { $line } ist keine Escape-Time-Zeile und nimmt deshalb keine Iterationsgrenze
error-plot-parameters-not-applicable = { $line } ist eine Escape-Time-Zeile und nimmt deshalb keine Parameterwerte
error-plot-value-kind-not-plottable = { $line } hat zwei Achsenvariablen und einen komplexen Wert, den noch kein Bild zeigt
error-plot-too-many-axis-variables = { $line } hat mehr als zwei Achsenvariablen; gib den übrigen mit --param einen Wert
error-plot-line-number-too-large = { $line } hat eine zu große Zeilennummer, um ihr Bild zu benennen
error-plot-sample-limit = das Bild bräuchte { $requested } Abtastwerte, mehr als die Grenze von { $limit }
error-plot-mesh-too-large = die Fläche bräuchte { $vertices } Eckpunkte, mehr als ein Bild fassen kann
error-plot-iteration-limit-too-large = { $iterations } Iterationen sind mehr als die größte erlaubte Grenze
error-plot-not-lowerable = dieser Ausdruck lässt sich noch nicht in Maschinenarithmetik zeichnen
error-plot-range-missing = Achse { $axis } von Ansicht { $view } hat keinen Bereich, und ein festgelegtes Bild braucht einen
error-read-scene-not-complete = das Bild dieser Generation ist nicht fertig, also lässt sich nichts daraus ablesen
error-read-layer-has-no-readings = Schicht { $layer } bietet keine Ablesungen
error-read-coordinate-count = die Schicht nimmt { $expected } Koordinaten, angegeben wurden { $found }
error-read-coordinate-not-exact = Koordinate { $axis } lässt sich nicht als exakter Wert lesen
error-read-needs-function-form = { $line } ist keine Funktionszeile, also lässt sich eine Ablesung daraus nicht als Zeile behalten
error-read-not-built = die Ablesung ließ sich aus dem Bild nicht bilden

error-unknown-concept = es gibt kein Konzept { $concept }
error-ambiguous-concept-name = { $name } benennt mehr als ein Konzept: { $candidates }
error-concept-set-not-loaded = die Konzeptmenge konnte nicht geladen werden, daher sind Konzepte nicht verfügbar
error-plot-no-axis-left = jede Variable von { $line } hat einen Wert, dem Bild bleibt also keine Achse zum Zeichnen

error-array-shapes-differ = die beiden Seiten haben verschiedene Formen und lassen sich nicht Eintrag für Eintrag verbinden
error-array-product-of-lists = ein Produkt zweier Listen ist mehrdeutig: es könnte die zusammengehörenden Einträge multiplizieren oder das Skalarprodukt sein, daher wählt calc nicht; ein Matrixprodukt wird mit Matrizen geschrieben, etwa [1, 2; 3, 4] * [1, 0; 0, 1]
error-array-body-and-point = eine Ableitung mit einer Liste im abgeleiteten Ausdruck und einer Liste von Stellen ist mehrdeutig: sie könnte jeden Eintrag mit der danebenstehenden Stelle paaren oder jeden Eintrag an jeder Stelle nehmen, daher wählt calc nicht; schreibe eine Stelle für das Ganze, etwa diff([x, x^2], x, 2), oder einen Eintrag je Stelle
error-array-not-rectangular = die Zeilen dieser Matrix sind nicht alle gleich lang
error-array-unsupported = diese Verbindung einer Zahl mit einer Liste ist nicht definiert
error-array-not-square = hier ist eine quadratische Matrix nötig, mit so vielen Zeilen wie Spalten
error-array-not-invertible = diese Matrix ist nicht invertierbar: ihre Zeilen sind nicht linear unabhängig, darum ist ihre Determinante null
error-array-entries-not-exact = calc bildet das nur für eine Matrix, deren Einträge alle exakte Zahlen sind
error-array-entries-not-rational = calc bildet die Eigenwerte nur für eine Matrix, deren Einträge alle rationale Zahlen sind
error-array-eigenvalues-not-real = diese Matrix hat Eigenwerte, die keine reellen Zahlen sind; calc nennt Eigenwerte nur, wenn alle reell sind
error-array-eigenvalues-not-radical = diese Matrix hat einen Eigenwert, den calc als rootof schreibt, und calc gibt Eigenvektoren nur, wo jeder Eigenwert eine rationale Zahl oder eine Summe von Quadratwurzeln ist
error-array-empty = diese Liste hat keine Einträge, es gibt nichts zu antworten
error-array-empty-transpose = diese Liste hat keine Einträge, es gibt also nichts zu transponieren
error-array-empty-determinant = diese Liste hat keine Einträge, es ist also keine Determinante zu bilden
error-array-not-a-matrix-transpose = Transponieren braucht eine Matrix, und das ist eine Liste
error-array-not-a-matrix = hier ist eine Matrix nötig, und das ist eine Liste
error-array-not-a-matrix-determinant = eine Determinante braucht eine Matrix, und das ist eine Liste
error-array-position-not-whole = die Stelle eines Eintrags ist eine ganze Zahl
error-array-position-outside = an dieser Stelle gibt es keinen Eintrag
error-array-not-comparable = die Einträge ließen sich nicht alle exakt vergleichen, daher gibt es keinen mittleren
error-array-not-ordered = calc konnte nicht entscheiden, welcher von { $first } und { $second } größer ist, und kann die Liste daher nicht ordnen
error-array-not-real = { $entry } ist keine reelle Zahl, und die komplexen Zahlen haben keine Ordnung, daher lässt sich die Liste nicht ordnen
error-array-not-real-entry = ein Eintrag ist keine reelle Zahl, und die komplexen Zahlen haben keine Ordnung, daher lässt sich die Liste nicht ordnen
error-order-not-real = { $side } ist keine reelle Zahl, und die komplexen Zahlen haben keine Ordnung, daher hat der Vergleich keinen Wahrheitswert; = und != werden für komplexe Zahlen entschieden
error-array-entry-without-value = ein Eintrag dieser Liste hat keinen Wert, daher lässt sich die Liste nicht ordnen
error-sort-not-ordered = calc konnte nicht entscheiden, welcher von { $first } und { $second } größer ist, und kann die Liste daher nicht ordnen; bis dahin waren es { $comparisons ->
    [one] { $comparisons } Vergleich
   *[other] { $comparisons } Vergleiche
}, dieser eingeschlossen, und { $writes ->
    [one] { $writes } Schreibvorgang
   *[other] { $writes } Schreibvorgänge
}
error-array-records-need-key = die Zeilen einer Matrix werden nach einem Schlüssel sortiert: schreib ihn als Funktion der Zeile, zum Beispiel r |-> at(r, 1)
error-array-machine-entry = { $reading } ist eine Maschinenzahl, und calc rechnet eine Liste, die eine enthält, noch nicht aus; rechne sie in einer eigenen Zeile
error-array-machine-entry-plain = ein Eintrag ist eine Maschinenzahl, und calc rechnet eine Liste, die eine enthält, noch nicht aus; rechne ihn in einer eigenen Zeile
error-array-not-a-list = das ist keine Liste und keine Matrix
error-limit-not-finite = hier nähern sich die beiden Seiten keiner Zahl: der Nenner wird null, der Zähler nicht
error-limit-not-finite-from-left = von links wächst der Betrag über jede Schranke, es gibt also keine Zahl, der sich der Wert nähert: der Nenner wird null, der Zähler nicht
error-limit-not-finite-from-right = von rechts wächst der Betrag über jede Schranke, es gibt also keine Zahl, der sich der Wert nähert: der Nenner wird null, der Zähler nicht
error-limit-not-a-rational-function = ein Grenzwert wird hier nur gebildet, wo der Rumpf in der Variablen aus +, -, *, /, ganzzahligen Potenzen, sin, cos, tan, exp, ln und atan gebaut ist; calc weiß nicht, ob dieser an der Stelle stetig ist, und nimmt es nicht an
error-taylor-not-analytic = ein Taylorpolynom wird hier nur gebildet, wo der Term in der Variablen aus +, -, *, /, ganzzahligen Potenzen, sin, cos, tan, exp, ln und atan gebaut ist
error-taylor-order-not-whole = das vierte Argument von taylor ist die Ordnung, eine ganze Zahl ab 0
error-taylor-order-too-high = die Ordnung eines Taylorpolynoms ist hier höchstens 64
error-taylor-not-defined-at-point = der Term oder eine seiner Ableitungen ist an der Stelle nicht definiert, darum hat er dort kein Taylorpolynom
error-root-not-a-polynomial = eine Nullstelle wird hier nur genommen, wo der Term ein Polynom mit rationalen Koeffizienten in der Variablen ist, und dieser ist es nicht
error-root-of-zero = jede Zahl ist Nullstelle des Nullpolynoms, darum hat es keine k-te
error-root-index-not-whole = das dritte Argument von rootof zählt die reellen Nullstellen von der kleinsten an und ist darum eine ganze Zahl ab 1
error-root-none-real = dieses Polynom hat keine reelle Nullstelle
error-root-index-out-of-range = dieses Polynom hat { $count } reelle Nullstellen, darum läuft das dritte Argument von rootof von 1 bis { $count }
error-limit-undecided = calc konnte diesen Grenzwert nicht entscheiden
error-integrand-not-a-polynomial = ein Integral wird hier nur gebildet, wo der Integrand ein Polynom oder ein Quotient zweier Polynome mit rationalen Koeffizienten in der Variablen ist, und dieser ist es nicht
error-integrand-pole-in-interval = der Integrand hat zwischen den Grenzen eine Polstelle, darum ist dieses Integral keine Zahl
error-integrand-high-degree-factor = der Nenner hat einen irreduziblen Faktor vom Grad { $degree }, und calc integriert einen Quotienten nur, wo jeder solche Faktor den Grad eins oder zwei hat
error-integrand-repeated-quadratic = der Nenner hat einen mehrfachen irreduziblen quadratischen Faktor, den calc noch nicht integriert
error-integral-bound-not-exact = der Integrand hat eine Polstelle, und calc ordnet eine Polstelle nur dann den Grenzen zu, wenn beide Grenzen exakte Zahlen sind
error-integral-without-bounds = ein Integral braucht eine untere und eine obere Grenze, um eine Zahl zu ergeben
error-array-units-differ = die Einträge dieser Liste tragen nicht alle dieselbe Einheit
error-array-ranks-differ = die eine Seite ist eine Liste, die andere eine Matrix; calc multipliziert zwei Matrizen oder eine Zahl mit beidem und behandelt eine Liste nicht als einzeilige Matrix
error-array-nested = eine Liste, deren Einträge selbst Listen sind, nimmt die Eingabesprache an, und calc rechnet sie noch nicht; eine Matrix schreibt man mit Zeilen, die durch Semikolons getrennt sind, etwa [2, 1; 1, 3]
error-array-power = calc potenziert eine Liste oder Matrix noch nicht
error-parse-chemistry-empty = die chemische Formel ist leer, in Spalte { $column }
error-parse-chemistry-unknown-element = { $name } ist kein Elementsymbol, in Spalte { $column }
error-parse-chemistry-wrong-case = { $name } ist so geschrieben kein Elementsymbol; Symbole beginnen mit einem Großbuchstaben, wie Co für Cobalt, und CO ist Kohlenstoff und Sauerstoff, deshalb rät calc nicht, in Spalte { $column }
error-parse-chemistry-ambiguous-charge = { $name } sagt nicht, welche Ziffern Atome zählen und welche die Ladung; schreib die Ladung nach ^, wie NH4^+ für Ammonium oder Fe^3+ für Eisen mit Ladung 3+, in Spalte { $column }
error-parse-chemistry-sign-before-number = { $name } stellt ein Vorzeichen vor eine Zahl, und das ist eine Ladung mit dem Vorzeichen zuerst oder die Massenzahl eines Nuklids; schreib eine Ladung als Fe^3+ und ein Nuklid in nuc'…', wie nuc'He-4', in Spalte { $column }
error-parse-chemistry-attached-number = { $name } beginnt mit einer angehängten Zahl, und das ist eine Massenzahl oder ein Koeffizient; schreib 2 H2O mit Leerzeichen in einer Reaktion, und Isotope werden noch nicht gelesen, in Spalte { $column }
error-parse-chemistry-oxidation-state = { $name } liest sich als Oxidationsstufe oder als die Elementsymbole, die seine Buchstaben ergeben; Oxidationsstufen werden noch nicht gelesen, in Spalte { $column }
error-parse-chemistry-sum-without-spaces = { $name } hat ein Vorzeichen zwischen zwei Teilchen; schreib eine Summe mit Leerzeichen, wie H2 + O2, oder eine Ladung nach ^, in Spalte { $column }
error-parse-chemistry-dot-separator = { $name } enthält einen Punkt, der hier kein Hydratpunkt ist; schreib · oder *, wie CuSO4·5H2O, in Spalte { $column }
error-parse-chemistry-decimal-count = { $name } ist entweder ein Hydrat mit Punkt oder eine nicht ganze Anzahl; schreib ein Hydrat mit · oder *, wie CuSO4·5H2O, und nicht ganze Anzahlen werden noch nicht gelesen, in Spalte { $column }
error-parse-chemistry-resonance = { $name } kennzeichnet Mesomerie, keine Reaktion; schreib eine Reaktion mit ->, in Spalte { $column }
error-parse-chemistry-nuclide = { $name } ist ein Nuklid, das chem'…' nicht liest; schreib Nuklide und Kernreaktionen in nuc'…', wie nuc'^14C', in Spalte { $column }
error-parse-chemistry-equilibrium = { $name } kennzeichnet ein Gleichgewicht, das noch nicht gelesen wird; schreib eine Reaktion mit ->, in Spalte { $column }
error-parse-chemistry-fractional-coefficient = { $name } ist ein nicht ganzer Koeffizient, der noch nicht gelesen wird; schreib ganze Koeffizienten, in Spalte { $column }
error-parse-chemistry-two-arrows = eine Reaktion hat einen Pfeil, und { $name } ist ein zweiter, in Spalte { $column }
error-parse-chemistry-empty-side = eine Reaktion braucht Teilchen auf beiden Seiten ihres Pfeils, in Spalte { $column }
error-parse-chemistry-unclosed-bracket = eine Klammer in der chemischen Formel ist nicht geschlossen, in Spalte { $column }
error-parse-chemistry-unexpected-character = { $name } gehört nicht zu einer chemischen Formel, in Spalte { $column }
error-parse-chemistry-coefficient-without-reaction = { $name } ist ein Koeffizient, und ein Koeffizient gehört zu einer Reaktion; ein Stoff allein steht ohne ihn, wie chem'H2O', in Spalte { $column }
error-chemistry-reaction-alone = { $reading } ist eine Reaktion, und eine Reaktion steht in einer eigenen Zeile
error-chemistry-substance-not-a-number = { $reading } ist ein Stoff, keine Zahl; frag nach einer seiner Eigenschaften, wie molar_mass({ $reading })
error-chemistry-too-many-species = { $reading } hat keinen eindeutigen Ausgleich: seine Teilchen erlauben { $count } unabhängige Reaktionen, und die kleinsten werden nur bis { $limit } Teilchen aufgezählt
error-chemistry-empty = { $reading } enthält kein Teilchen
error-chemistry-element-not-conserved = { $reading } ist nicht ausgeglichen: { $element } zählt links { $reactants } und rechts { $products }
error-chemistry-charge-not-conserved = { $reading } ist nicht ausgeglichen: die Ladung ist links { $reactants } und rechts { $products }
error-chemistry-not-unique = { $reading } hat keinen eindeutigen Ausgleich: seine Teilchen erlauben { $count } unabhängige Reaktionen, und die kleinsten sind { $reactions }
error-chemistry-impossible = { $reading } lässt sich nicht ausgleichen: keine Koeffizienten außer null erhalten jedes Element und die Ladung
error-chemistry-takes-no-part = { $species } kann an { $reading } nicht teilnehmen: jeder Ausgleich gibt ihm den Koeffizienten 0
error-chemistry-other-side = { $species } kann an { $reading } nur auf der anderen Seite des Pfeils teilnehmen
error-chemistry-molar-mass-needs-a-substance = { $reading } braucht einen als chem'…' geschriebenen Stoff, wie molar_mass(chem'H2O')
error-chemistry-no-standard-atomic-weight = { $element } hat kein Standardatomgewicht, also hat { $reading } hier keine molare Masse
error-chemistry-molar-mass-of-an-ion = { $reading } fragt nach der molaren Masse eines geladenen Teilchens, und eine molare Masse wird hier nur von einem neutralen Stoff genommen
error-chemistry-molar-mass-of-no-element = { $reading } enthält kein Element
error-parse-chemistry-wrong-arrow = { $name } ist kein Reaktionspfeil; schreib eine Reaktion mit ->, in Spalte { $column }
error-parse-chemistry-no-arrow = eine Summe von Teilchen braucht einen Pfeil, um eine Reaktion zu sein; schreib Edukte -> Produkte, in Spalte { $column }
error-parse-chemistry-repeated-species = { $name } steht mehr als einmal in der Reaktion; schreib jedes Teilchen einmal, in Spalte { $column }
error-parse-chemistry-zero-count = { $name } ist eine Anzahl 0 oder eine, die mit 0 beginnt; wenn der Buchstabe O gemeint war, schreib O, wie in HO2, in Spalte { $column }
error-parse-chemistry-zero-coefficient = { $name } ist ein Koeffizient 0 oder einer, der mit 0 beginnt; lass einen Stoff weg, der nicht teilnimmt, und schreib einen Koeffizienten ohne führende 0, in Spalte { $column }
error-parse-chemistry-zero-charge = { $name } ist eine Ladung 0; einen neutralen Stoff schreibst du ohne Ladung, wie Fe, in Spalte { $column }
error-parse-chemistry-count-too-large = { $name } ergibt eine Zahl über 9223372036854775807, und eine größere Zahl liest calc in einer chemischen Formel nicht, in Spalte { $column }
error-parse-chemistry-isotope = { $name } ist ein Isotopensymbol, Deuterium oder Tritium, und Isotope werden noch nicht gelesen, in Spalte { $column }
error-parse-chemistry-dangling-hydrate-dot = { $name } ist ein Hydratpunkt ohne Bestandteil auf beiden Seiten; schreib beide Teile, wie CuSO4·5H2O, in Spalte { $column }
error-parse-chemistry-coefficient-outside-literal = eine Zahl vor { $name } ist ein Koeffizient außerhalb der Formel; ein Koeffizient gehört in eine Reaktion, wie chem'2 H2 + O2 -> 2 H2O', in Spalte { $column }
error-chemistry-no-reaction-with-these-sides = { $reading } hat keine Reaktion mit diesen Seiten: seine Teilchen erlauben { $count } unabhängige Ausgleiche, und bei keinem steht jeder Koeffizient auf der Seite, auf der er geschrieben ist
error-temperature-difference-as-reading = { $written } ist eine Temperaturdifferenz und keine Ablesung; für die Ablesung von { $number } { $unit } schreib { $reading }
error-below-absolute-zero = { $reading } liegt unter dem absoluten Nullpunkt, { $limit }
error-parse-nuclear-empty = die Kernreaktion ist leer, in Spalte { $column }
error-parse-nuclear-unknown-species = { $name } ist kein Nuklid und kein Teilchen, das calc liest; schreib ein Nuklid als ^14C oder C-14 und ein Teilchen als n, p, d, t, α, γ, e-, e+, ν_e oder ν̄_e, in Spalte { $column }
error-parse-nuclear-attached-number = { $name } beginnt mit einer angehängten Zahl, und das ist eine Massenzahl oder ein Koeffizient; schreib das Nuklid als ^14C oder C-14 und einen Koeffizienten mit Leerzeichen, wie 3 n, in Spalte { $column }
error-parse-nuclear-atom-symbol = { $name } ist das Symbol eines Atoms, Deuterium oder Tritium, und eine Kernreaktion zählt Kerne; schreib d oder ^2H, oder t oder ^3H, in Spalte { $column }
error-parse-nuclear-bare-element = { $name } ist ein Element, kein Nuklid; schreib seine Massenzahl dazu, wie ^14C oder C-14, in Spalte { $column }
error-parse-nuclear-particle-or-element = { $name } ist ein Element oder das Neutron oder Proton als Großbuchstabe, und calc rät nicht zwischen ihnen; schreib n für das Neutron, p für das Proton oder das Element mit seiner Massenzahl, wie ^14N, in Spalte { $column }
error-parse-nuclear-school-letter = { $name } ist ein einzelner Buchstabe, den calc nicht als Teilchen liest; schreib ^3He für das Helion, γ für ein Photon und α für ein α-Teilchen, in Spalte { $column }
error-parse-nuclear-unsigned-electron = { $name } sagt nicht, ob es ein Elektron oder ein Positron ist; schreib e- oder e+, oder ^0_-1e oder ^0_+1e, in Spalte { $column }
error-parse-nuclear-neutrino-flavour = { $name } nennt keine Neutrinosorte; schreib ν_e oder nu_e für das Elektron-Neutrino und ν̄_e oder anti_nu_e für das Elektron-Antineutrino, in Spalte { $column }
error-parse-nuclear-charge = { $name } trägt ein Ladungszeichen, und eine Kernreaktion liest Kerne und Teilchen ohne eines; Ionen werden hier nicht gelesen, in Spalte { $column }
error-parse-nuclear-mass-below-charge = { $name } hat die Massenzahl 0 oder eine unter seiner Ordnungszahl, und einen solchen Kern gibt es nicht, in Spalte { $column }
error-parse-nuclear-numbers-disagree = die Zahlen bei { $name } gehören nicht dazu: die Ordnungszahl eines Nuklids ist die seines Symbols, ein Elektron ist ^0_-1e, ein Positron ^0_+1e, ein Neutron ^1_0n und ein Proton ^1_1p, in Spalte { $column }
error-parse-nuclear-atomic-number-disagrees = { $name } schreibt die Ordnungszahl { $written }, und { $element } hat die Ordnungszahl { $expected }; schreib { $expected } unter die Massenzahl oder lass die Ordnungszahl weg, in Spalte { $column }
error-parse-nuclear-isomer = { $name } ist ein Kernisomer, und Isomere werden noch nicht gelesen, in Spalte { $column }
error-parse-nuclear-other-lepton = { $name } ist ein Myon, ein Tauon oder eines ihrer Neutrinos, und bisher wird nur die Elektronfamilie gelesen, in Spalte { $column }
error-parse-nuclear-compact-form = { $name } ist in der Kurzform Kern(ein,aus)Produkt geschrieben, die noch nicht gelesen wird; schreib dieselbe Reaktion mit Pfeil, wie ^14N + α -> ^17O + p, in Spalte { $column }
error-parse-nuclear-element-name = { $name } nennt ein Element in Worten, und das wird nicht gelesen; schreib sein Symbol, wie C-14, in Spalte { $column }
error-parse-nuclear-resonance = { $name } kennzeichnet Mesomerie, keine Reaktion; schreib eine Reaktion mit ->, in Spalte { $column }
error-parse-nuclear-equilibrium = { $name } kennzeichnet ein Gleichgewicht, das nicht gelesen wird; schreib eine Reaktion mit ->, in Spalte { $column }
error-parse-nuclear-wrong-arrow = { $name } ist kein Reaktionspfeil; schreib eine Reaktion mit ->, in Spalte { $column }
error-parse-nuclear-no-arrow = eine Summe von Teilchen braucht einen Pfeil, um eine Reaktion zu sein; schreib Edukte -> Produkte, in Spalte { $column }
error-parse-nuclear-two-arrows = eine Reaktion hat einen Pfeil, und { $name } ist ein zweiter, in Spalte { $column }
error-parse-nuclear-empty-side = eine Reaktion braucht Teilchen auf beiden Seiten ihres Pfeils, in Spalte { $column }
error-parse-nuclear-zero-coefficient = { $name } ist ein Koeffizient 0 oder einer, der mit 0 beginnt; lass ein Teilchen weg, das nicht teilnimmt, und schreib einen Koeffizienten ohne führende 0, in Spalte { $column }
error-parse-nuclear-fractional-coefficient = { $name } ist ein nicht ganzer Koeffizient, der nicht gelesen wird; schreib ganze Koeffizienten, in Spalte { $column }
error-parse-nuclear-coefficient-without-reaction = { $name } ist ein Koeffizient, und ein Koeffizient gehört zu einer Reaktion; ein Nuklid allein steht ohne ihn, wie nuc'^14C', in Spalte { $column }
error-parse-nuclear-number-too-large = { $name } ergibt eine Zahl über 9223372036854775807, und eine größere Zahl liest calc in einer Kernreaktion nicht, in Spalte { $column }
error-parse-nuclear-coefficient-outside-literal = eine Zahl vor { $name } ist ein Koeffizient außerhalb der Reaktion; ein Koeffizient gehört hinein, wie nuc'4 p -> ^4He + 2 e+ + 2 ν_e', in Spalte { $column }
error-nuclear-reaction-alone = { $reading } ist eine Reaktion, und eine Reaktion steht in einer eigenen Zeile
error-nuclear-nuclide-not-a-number = { $reading } ist ein Nuklid, keine Zahl; frag nach dem Q-Wert einer Reaktion, wie q_value(nuc'^2H + ^3H -> ^4He + n')
error-nuclear-empty = { $reading } enthält kein Teilchen außer Photonen
error-nuclear-mass-number-not-conserved = { $reading } ist nicht ausgeglichen: die Massenzahl ist links { $reactants } und rechts { $products }
error-nuclear-charge-not-conserved = { $reading } ist nicht ausgeglichen: die Ladung ist links { $reactants } und rechts { $products }
error-nuclear-lepton-number-not-conserved = { $reading } ist nicht ausgeglichen: Massenzahl und Ladung stimmen, aber die Elektron-Leptonenzahl ist links { $reactants } und rechts { $products }, und { $neutrinos } rechts würde sie ausgleichen
error-nuclear-q-value-needs-a-reaction = { $reading } braucht eine als nuc'…' geschriebene Kernreaktion, wie q_value(nuc'^2H + ^3H -> ^4He + n')
error-nuclear-q-value-does-not-balance = { $reading } hat hier keinen Q-Wert, weil seine Reaktion nicht ausgeglichen ist; die Reaktion in einer eigenen Zeile sagt, warum
error-nuclear-estimated-mass = { $nuclide } hat in AME2020 nur eine geschätzte Masse, aus dem Verlauf der Massenfläche, also hat { $reading } hier keinen Q-Wert
error-nuclear-no-mass = { $nuclide } hat in AME2020 keine Masse, also hat { $reading } hier keinen Q-Wert
error-nuclear-mass-number-not-conserved-balance = { $reading } ist nicht ausgeglichen: die Massenzahl ist links { $reactants } und rechts { $products }; rein rechnerisch ist der einzige Satz von Koeffizienten, der diese Teilchen ausgleicht, { $balance }, und das sagt nicht, dass eine solche Reaktion stattfindet
error-nuclear-charge-not-conserved-balance = { $reading } ist nicht ausgeglichen: die Ladung ist links { $reactants } und rechts { $products }; rein rechnerisch ist der einzige Satz von Koeffizienten, der diese Teilchen ausgleicht, { $balance }, und das sagt nicht, dass eine solche Reaktion stattfindet
error-nuclear-lepton-number-not-conserved-balance = { $reading } ist nicht ausgeglichen: Massenzahl und Ladung stimmen, aber die Elektron-Leptonenzahl ist links { $reactants } und rechts { $products }, und { $neutrinos } rechts würde sie ausgleichen; rein rechnerisch ist der einzige Satz von Koeffizienten, der diese Teilchen ausgleicht, { $balance }, und das sagt nicht, dass eine solche Reaktion stattfindet
error-kind-mismatch = { $reading }: die eine Seite ist { $left ->
    [1] Frequenz
    [2] Aktivität
    [3] Energiedosis
    [4] Äquivalentdosis
    [5] Lichtstärke
   *[6] Lichtstrom
} und die andere { $right ->
    [1] Frequenz
    [2] Aktivität
    [3] Energiedosis
    [4] Äquivalentdosis
    [5] Lichtstärke
   *[6] Lichtstrom
}; sie teilen eine Einheit, sind aber verschiedene Größen und lassen sich daher nicht addieren, vergleichen oder ineinander umrechnen

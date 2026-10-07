common-kind-exact = exakt
common-kind-symbolic = symbolisch
common-kind-numeric = numerisch
common-kind-sampled = Stichprobe
common-kind-measured = gemessen
common-kind-derived = abgeleitet
common-kind-running = läuft
common-kind-differs = abweichend
common-kind-stale = veraltet
common-kind-failed = fehlerhaft
common-kind-definition = Definition

common-result-kind-exact-rational = exakt rational
common-result-kind-exact-complex = exakte komplexe Zahl, rationale Teile
common-result-kind-machine-complex = komplexe Zahl aus zwei Maschinengleitkommazahlen
common-result-kind-algebraic = exakt, mit Wurzeln (algebraisch)
common-result-kind-symbolic = exakt, als Ausdruck (symbolisch)
common-result-kind-machine-float = Maschinenzahl

common-precision-exact = exakt
common-precision-f32 = f32
common-precision-f64 = f64

common-backend-automatic = automatisch
common-backend-cpu = CPU
common-backend-simd = SIMD
common-backend-gpu = GPU
common-backend-none = keines

common-gpu-no-adapter = Es ist keine Grafikkarte verfügbar, daher läuft dies ohne das GPU-Rechenwerk.
common-gpu-exact-case-failed = Die Grafikkarte hat eine Ganzzahlprüfung nicht exakt gerechnet, daher läuft dies ohne das GPU-Rechenwerk.
common-gpu-exact-case-failed-operation = Die Grafikkarte hat { $operation } auf Ganzzahlen nicht exakt gerechnet, daher läuft dies ohne das GPU-Rechenwerk.
common-gpu-case-not-run = Eine Prüfung konnte auf der Grafikkarte nicht laufen, daher läuft dies ohne das GPU-Rechenwerk.
common-gpu-case-not-run-operation = Die Prüfung von { $operation } konnte auf der Grafikkarte nicht laufen, daher läuft dies ohne das GPU-Rechenwerk.

common-time-of-day = { $hour }:{ $minute }:{ $second }
common-date-and-time = { $day }.{ $month }.{ $year } { $time }
common-status-replay-not-run = Wiederholung: nicht gelaufen
common-status-replay-running = Wiederholung: läuft
common-status-replay-verified = Wiederholung: bestätigt { $time }
common-status-replay-differs = Wiederholung: abweichend { $time }
common-status-results = { $count ->
    [one] { $count } Ergebnis
   *[other] { $count } Ergebnisse
}
common-status-running = { $count ->
    [one] { $count } läuft
   *[other] { $count } laufen
}
common-status-differ = { $count ->
    [one] { $count } weicht ab
   *[other] { $count } weichen ab
}
common-status-locale = { $locale }
common-status-precision = Genauigkeit: { $precision }
common-status-backend = Backend: { $backend }

common-running-progress = { $percent } %  { $backend }  { $elapsed }

common-record-value = Wert
common-record-kind = Zahl
common-record-machine-type = Maschinentyp
common-record-uncertainty = Unsicherheit
common-record-coverage-factor = Erweiterungsfaktor
common-record-uncertainty-budget = Unsicherheitsbudget
common-record-rounding-error = Rundungsfehler
common-record-rounding-unknown = unbekannt, in Maschinenarithmetik gerundet
common-record-rounding-measure-last-place = in Einheiten der letzten Stelle
common-record-rounding-measure-relative = relativ
common-record-unit = Einheit
common-record-method = gerechnet
common-record-parameters = Parameter
common-record-convergence = Konvergenz
common-record-converged = konvergiert nach { $iterations ->
    [one] { $iterations } Iteration
   *[other] { $iterations } Iterationen
}
common-record-not-converged = nicht konvergiert nach { $iterations ->
    [one] { $iterations } Iteration
   *[other] { $iterations } Iterationen
}
common-record-iteration-limit = Iterationsgrenze erreicht nach { $iterations ->
    [one] { $iterations } Iteration
   *[other] { $iterations } Iterationen
}
common-record-error-estimate = Fehlerschätzung
common-record-condition = Konditionszahl
common-record-condition-absolute = absolut
common-record-condition-relative = relativ
common-record-notes = Hinweise
common-record-ran-on = gerechnet auf
common-record-run-time = Laufzeit
common-record-how-it-ran = wie gerechnet wurde
common-record-modes-all-native = jede Operation nativ
common-record-modes-all-integer-exact = jede Operation ganzzahlig-exakt
common-record-modes-all-exact-in-both-forms = jede Operation in beiden Formen exakt
common-record-modes-native-count = { $count ->
    [one] { $count } nativ
   *[other] { $count } nativ
}
common-record-modes-integer-exact-count = { $count ->
    [one] { $count } ganzzahlig-exakt
   *[other] { $count } ganzzahlig-exakt
}
common-record-modes-exact-in-both-forms-count = { $count ->
    [one] { $count } in beiden Formen exakt
   *[other] { $count } in beiden Formen exakt
}
common-record-modes-by-operation = nach Operation
common-record-modes-native = nativ
common-record-modes-integer-exact = ganzzahlig-exakt
common-record-modes-exact-in-both-forms = in beiden Formen exakt
common-record-modes-not-offered = nicht angeboten
common-record-modes-what-native-means = Nativ heißt, dass die Prüfungen auf dieser Grafikkarte bei diesem Start bestanden wurden. Es ist keine Aussage darüber, dass die Operation für jeden Operanden richtig rechnet.
common-record-run-time-nanoseconds = { $time } ns, bei diesem Lauf gemessen
common-record-run-time-microseconds = { $time } µs, bei diesem Lauf gemessen
common-record-run-time-milliseconds = { $time } ms, bei diesem Lauf gemessen
common-record-run-time-seconds = { $time } s, bei diesem Lauf gemessen
common-record-computed-at = berechnet am
common-record-produced-by = berechnet mit Version
common-record-depends-on = hängt ab von
common-record-used-by = verwendet von
common-record-references = Quellen
common-record-seed = Startwert
common-record-generator = Generator
common-record-replay = Wiederholung
common-record-none = keine

common-replay-verified = bestätigt
common-replay-differs = abweichend
common-replay-not-compared = nicht verglichen
common-replay-failed-to-parse = Eingabe nicht lesbar

common-kind-ways = Wege
common-kind-not-reached = nicht erreicht

common-criterion-fewest-measurements = wenigste Messungen
common-criterion-fewest-steps = wenigste Schritte
common-criterion-exactness = Exaktheit
common-criterion-error-bound = Fehlerschranke
common-criterion-gate-count = Gatterzahl
common-criterion-smallest-uncertainty = kleinste Unsicherheit
common-criterion-conditioning = Kondition
common-criterion-rule-id = Regelkennung
common-criterion-front = Front

common-solve-wanted = gesucht
common-solve-given = gegeben
common-solve-criterion = Kriterium
common-solve-cap = Obergrenze
common-solve-tie-break = dann
common-solve-shown = { $count } gezeigt
common-solve-more-exist = weitere vorhanden
common-solve-obtainable = { $count ->
    [one] { $count } Größe beschaffbar
   *[other] { $count } Größen beschaffbar
}
common-solve-no-bound = keine Schranke
common-solve-rank = Rang
common-solve-inputs = Eingaben
common-solve-rule = Regel
common-solve-conditions = Bedingungen
common-solve-quantity = Größe
common-solve-mark = Markierung
common-solve-value = Wert
common-solve-mark-given = gegeben
common-solve-mark-yes = ja
common-solve-mark-no = nein
common-solve-mark-unmarked = offen
common-solve-condition-holds = erfüllt
common-solve-condition-fails = nicht erfüllt
common-solve-condition-undecided = unentschieden
common-solve-dominated = dominiert
common-solve-further-input = eine weitere Eingabe
common-solve-bound = Schranke
common-solve-other-ways = andere Wege
common-solve-obtainable-list = beschaffbar
common-solve-not-obtainable-list = nicht beschaffbar
common-solve-between = zwischen
common-solve-score-exact = exakt
common-solve-score-machine = Maschine
common-solve-score-zero = null
common-solve-score-documented = dokumentiert

common-kind-reachable = erreichbar
common-solve-quantities = { $count ->
    [one] { $count } Größe
   *[other] { $count } Größen
}
common-solve-shown-of = { $shown } von { $total } gezeigt
common-solve-nothing-follows = aus diesem Gegebenen folgt nichts
common-solve-if-also-known = wenn du auch { $names } kennst
common-solve-steps = Schritte
common-solve-derivation = Herleitung
common-solve-sources = Quellen
common-solve-object = Objekt

common-method-worst-case = als kleinster und größter Wert über jede Kombination der Toleranzen, beide exakt, an einer Ecke, die calc bewiesen hat
common-method-chemistry-composition = aus der Formel, Element für Element
common-method-chemistry-balance = als kleinste ganze Koeffizienten, die jedes Element und die Ladung erhalten, aus dem exakten Kern der Reaktion
common-method-chemistry-check = durch Prüfen der geschriebenen Koeffizienten für jedes Element und die Ladung
common-method-nuclear-nuclide = aus der Schreibweise: Massenzahl, Ordnungszahl und Neutronen
common-method-nuclear-check = durch Prüfen der Reaktion, wie sie geschrieben ist, wobei ein fehlender Koeffizient als 1 zählt, für Massenzahl, Ladung und Elektron-Leptonenzahl
common-method-exact-evaluation = exakt, ohne Rundung
common-method-plan-evaluation = in Maschinenarithmetik
common-method-way-evaluation = entlang des gewählten Wegs
common-method-named = mit Verfahren { $name }
common-record-order = Reihenfolge
common-record-form = Form
common-record-partition = Aufteilung
common-record-draws = Ziehungen
common-record-flips = Umdrehungen
common-record-fewest-flips = wenigste Umdrehungen
common-record-most-flips = meiste Umdrehungen
common-record-average-flips = mittlere Umdrehungen
common-record-gaps = Lücken
common-record-gaps-used = benutzte Lücken
common-record-passes = Durchläufe
common-record-shuffles = Mischvorgänge
common-record-expected-shuffles = erwartete Mischvorgänge
common-record-tallies = Zählerschritte
common-record-key-range = Schlüsselbereich
common-record-from-positions = von Stellen
common-record-comparisons = Vergleiche
common-record-writes = Schreibvorgänge
common-record-key-evaluations = Schlüssel
common-record-fewest-comparisons = wenigste Vergleiche
common-record-most-comparisons = meiste Vergleiche
common-record-average-comparisons = mittlere Vergleiche
common-record-fewest-writes = wenigste Schreibvorgänge
common-record-most-writes = meiste Schreibvorgänge
common-record-average-writes = mittlere Schreibvorgänge
common-record-lower-bound = untere Schranke
common-record-described-in = beschrieben in
common-sort-increasing = nach steigendem Schlüssel
common-sort-decreasing = nach fallendem Schlüssel
common-sort-positions = { $positions }, die Stelle, die jeder Eintrag des Ergebnisses in der Eingabe hatte, von eins an gezählt
common-sort-counted = { $count }, bei dieser Eingabe gezählt
common-sort-key-evaluations = { $count ->
    [one] einmal
   *[other] { $count }-mal
} ausgewertet, einmal für jeden Eintrag, bevor die Einträge geordnet werden
common-sort-extreme = { $formula } = { $value } bei n = { $length }, über jede Anordnung von n verschiedenen Einträgen; { $provenance }
common-sort-extreme-powers-of-two = { $formula } = { $value } bei n = { $length }, über jede Anordnung von n verschiedenen Einträgen, für n eine Zweierpotenz; { $provenance }
common-sort-average = { $formula } = { $value } bei n = { $length }, der Mittelwert über jede Anordnung von n verschiedenen Einträgen, jede gleich wahrscheinlich; { $provenance }
common-sort-average-powers-of-two = { $formula } = { $value } bei n = { $length }, der Mittelwert über jede Anordnung von n verschiedenen Einträgen, jede gleich wahrscheinlich, für n eine Zweierpotenz; { $provenance }
common-sort-extreme-constant = { $value } bei jedem n, über jede Anordnung von n verschiedenen Einträgen; { $provenance }
common-sort-average-constant = { $value } bei jedem n, der Mittelwert über jede Anordnung von n verschiedenen Einträgen, jede gleich wahrscheinlich; { $provenance }
common-sort-extreme-over-choices = { $formula } = { $value } bei n = { $length }, über jede Anordnung von n verschiedenen Einträgen und jede Wahl der Pivots; { $provenance }
common-sort-expected-over-choices = { $formula } = { $value } bei n = { $length }, der Erwartungswert, wenn jedes Pivot gleichverteilt gezogen wird, derselbe für jede Anordnung von n verschiedenen Einträgen; { $provenance }
common-sort-extreme-constant-over-choices = { $value } bei jedem n, über jede Anordnung von n verschiedenen Einträgen und jede Wahl der Pivots; { $provenance }
common-sort-expected-constant-over-choices = { $value } bei jedem n, der Erwartungswert, wenn jedes Pivot gleichverteilt gezogen wird, derselbe für jede Anordnung von n verschiedenen Einträgen; { $provenance }
common-sort-cost-holds-from = { $formula }, gültig ab n = { $from }, sagt daher bei n = { $length } nichts aus; { $provenance }
common-sort-derived = hier hergeleitet und an jeder Anordnung von bis zu { $limit } Einträgen geprüft
common-sort-fitted = eine geschlossene Form, an die Zählungen angepasst und an jeder Anordnung von bis zu { $limit } Einträgen geprüft, nicht für jedes n hergeleitet
common-sort-derived-from = hergeleitet aus { $source } und an jeder Anordnung von bis zu { $limit } Einträgen geprüft
common-sort-derived-over-choices = hier hergeleitet und an jeder Anordnung von bis zu { $limit } Einträgen mit jeder Folge von Pivotwahlen geprüft; die Pivots dieses Laufs kamen aus dem Generator
common-sort-derived-from-over-choices = hergeleitet aus { $source } und an jeder Anordnung von bis zu { $limit } Einträgen mit jeder Folge von Pivotwahlen geprüft; die Pivots dieses Laufs kamen aus dem Generator
common-sort-derived-expected-over-choices = hier hergeleitet und an jeder Anordnung von bis zu { $limit } Einträgen mit jeder Folge von Pivotwahlen geprüft, jede mit ihrer Wahrscheinlichkeit gewichtet; die Pivots dieses Laufs kamen aus dem Generator, nicht aus diesem Mittel
common-sort-derived-from-expected-over-choices = hergeleitet aus { $source } und an jeder Anordnung von bis zu { $limit } Einträgen mit jeder Folge von Pivotwahlen geprüft, jede mit ihrer Wahrscheinlichkeit gewichtet; die Pivots dieses Laufs kamen aus dem Generator, nicht aus diesem Mittel
common-sort-lower-bound = ceil(log2(n!)) = { $bound } bei n = { $length }: kein Sortieren durch Vergleiche kommt im schlechtesten Fall bei n verschiedenen Einträgen mit weniger Vergleichen aus
common-sort-lower-bound-without-comparisons = ceil(log2(n!)) gilt hier nicht: dieses Sortieren vergleicht nie, deshalb kann es darunter bleiben
common-sort-tallies = { $count }, bei dieser Eingabe gezählt: einer für jeden Eintrag beim Zählen, einer für jede Präfixsumme und einer für jeden Eintrag beim Einsetzen
common-sort-radix-tallies = { $count }, bei dieser Eingabe gezählt: in jedem Durchlauf einer für jeden Eintrag beim Zählen, einer für jede der Basis − 1 Präfixsummen und einer für jeden Eintrag beim Einsetzen
common-sort-bead-tallies = { $count }, bei dieser Eingabe gezählt: einer für jede Perle, wenn sie auf ihren Stab fällt, und einer für jede Perle, wenn ihre Reihe gelesen wird, also das Doppelte der Summe der Zahlen
common-sort-no-positions = keine: Beadsort baut jeden Wert aus den Perlen seiner Reihe neu und bewegt keinen Eintrag, deshalb hat kein Eintrag des Ergebnisses eine Stelle, von der er kam
common-sort-passes = { $passes ->
    [0] 0 in Basis { $base }: jeder Schlüssel ist gleich dem kleinsten, deshalb bleibt keine Ziffer zu sortieren
    [one] 1 in Basis { $base }, einer für jede Ziffer des größten Schlüssels minus dem kleinsten
   *[other] { $passes } in Basis { $base }, einer für jede Ziffer des größten Schlüssels minus dem kleinsten, die niedrigste Ziffer zuerst
}
common-sort-shuffles = { $count ->
    [one] 1 von höchstens { $limit }, mit n − 1 Vertauschungen
   *[other] { $count } von höchstens { $limit }, jeder mit n − 1 Vertauschungen
}
common-sort-expected-shuffles = { $factorial } = n! bei n = { $length } von jeder Anordnung von n verschiedenen Schlüsseln außer der sortierten, und 0 von der sortierten, weil jeder Mischvorgang eine solche Liste mit Wahrscheinlichkeit 1/n! sortiert; gleiche Schlüssel, m1, m2, … von jedem Wert, machen daraus n!/(m1!·m2!·…); nach Gruber, Holzer und Ruepp 2007, hier nicht geprüft, weil die Ziehungen keine Schranke haben
common-sort-expected-shuffles-none = 0: eine Liste aus höchstens einem Eintrag ist sortiert und wird nie gemischt
common-sort-unbounded = keine: das Mischen hat keine Schranke, deshalb auch diese Zahl nicht; limit= hält das Sortieren an
common-sort-per-shuffle = 2*(n − 1) für jeden Mischvorgang, mal die erwarteten Mischvorgänge
common-sort-per-pass-of-the-keys = 2*n in jedem Durchlauf, dieselbe Zahl für jede Anordnung derselben Schlüssel; die Zahl der Durchläufe hängt von den Schlüsseln ab und nicht von n allein, deshalb wird keine Formel in n angegeben
common-sort-key-range = { $range ->
    [one] 1 Wert, da der kleinste Schlüssel auch der größte ist, mit einem Zähler
   *[other] { $range } Werte vom kleinsten bis zum größten Schlüssel, ein Zähler für jeden
}
common-sort-flips = { $count ->
    [one] 1, bei dieser Eingabe gezählt; eine Umdrehung kehrt die ersten k Einträge der Liste um, für ein k
   *[other] { $count }, bei dieser Eingabe gezählt; eine Umdrehung kehrt die ersten k Einträge der Liste um, für ein k
}
common-sort-draws = { $count ->
    [one] 1 Block
   *[other] { $count } Blöcke
} von philox4x32_10 bei Seed { $seed }, Strom 0, ab Index 0, bei dieser Eingabe gezählt: einer für jeden Teil aus zwei oder mehr Einträgen und einer mehr für jeden verworfenen Block, damit jeder Platz gleich wahrscheinlich ist
common-sort-shuffle-draws = { $count ->
    [one] 1 Block
   *[other] { $count } Blöcke
} von philox4x32_10 bei Seed { $seed }, Strom 1, ab Index 0, bei dieser Eingabe gezählt: einer für jede Stelle jedes Mischvorgangs und einer mehr für jeden verworfenen Block, damit jeder Platz gleich wahrscheinlich ist
common-generator-layout = Schlüssel: der Seed { $seed } als zwei 32-Bit-Wörter, das niedrige zuerst; Zähler: Index + 2^64 · Strom mit Index { $index } und Strom { $stream }, als vier 32-Bit-Wörter, das niedrige zuerst; derselbe Seed, Strom und Index geben auf jedem Rechner dieselben Wörter
common-generator-statistical = ein statistischer Generator: wer Seed, Strom und Index kennt, kann seine Wörter ausrechnen, deshalb taugt er nicht für Kryptographie und für nichts, was ein Angreifer nicht vorhersagen darf
common-sort-source-vitter-flajolet-1990 = J. S. Vitter und P. Flajolet, Average-Case Analysis of Algorithms and Data Structures, Handbook of Theoretical Computer Science, Bd. A, 1990, Abschnitt { $section }, DOI 10.1016/B978-0-444-88071-0.50014-X
common-sort-source-short-vitter-flajolet-1990 = Vitter und Flajolet 1990, Abschnitt { $section }
common-sort-source-wikipedia-insertion-sort = Wikipedia (englisch), Insertion sort, Abschnitt Variants (CC BY-SA 4.0)
common-sort-source-short-wikipedia-insertion-sort = Wikipedia (englisch), Insertion sort
common-sort-source-wikipedia-bubble-sort = Wikipedia (englisch), Bubble sort, Abschnitte Pseudocode implementation und Optimizing bubble sort (CC BY-SA 4.0)
common-sort-source-short-wikipedia-bubble-sort = Wikipedia (englisch), Bubble sort
common-sort-source-flajolet-golin-1994 = P. Flajolet und M. Golin, Mellin transforms and asymptotics: the mergesort recurrence, Acta Informatica 31 (1994), Abschnitt 1, DOI 10.1007/BF01177551
common-sort-source-short-flajolet-golin-1994 = Flajolet und Golin 1994
common-sort-source-wikipedia-heapsort = Wikipedia (englisch), Heapsort, die Standardfassung mit einem von unten aufgebauten Heap (CC BY-SA 4.0)
common-sort-source-short-wikipedia-heapsort = Wikipedia (englisch), Heapsort
common-sort-source-wikipedia-quicksort-lomuto = Wikipedia (englisch), Quicksort, Abschnitt Lomuto partition scheme (CC BY-SA 4.0)
common-sort-source-wikipedia-quicksort-hoare = Wikipedia (englisch), Quicksort, Abschnitt Hoare partition scheme (CC BY-SA 4.0), nach C. A. R. Hoare, Quicksort, The Computer Journal 5 (1962), DOI 10.1093/comjnl/5.1.10
common-sort-source-short-wikipedia-quicksort = Wikipedia (englisch), Quicksort
common-sort-source-wikipedia-counting-sort = Wikipedia (englisch), Counting sort, Abschnitt Pseudocode, nach Cormen, Leiserson, Rivest und Stein, Introduction to Algorithms, Abschnitt 8.2 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-counting-sort = Wikipedia (englisch), Counting sort
common-sort-source-cormen-randomized-quicksort = T. H. Cormen, C. E. Leiserson, R. L. Rivest und C. Stein, Introduction to Algorithms, Abschnitt 7.3, A randomized version of quicksort
common-sort-source-short-cormen-randomized-quicksort = Cormen, Leiserson, Rivest und Stein, Abschnitt 7.3
common-sort-source-wikipedia-selection-sort-variants = Wikipedia (englisch), Selection sort, Abschnitt Variants (CC BY-SA 4.0)
common-sort-source-short-wikipedia-selection-sort-variants = Wikipedia (englisch), Selection sort
common-sort-source-wikipedia-cocktail-shaker-sort = Wikipedia (englisch), Cocktail shaker sort, Abschnitt Pseudocode, nach D. E. Knuth, The Art of Computer Programming, Bd. 3, 1973, S. 110–111 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-cocktail-shaker-sort = Wikipedia (englisch), Cocktail shaker sort
common-sort-source-wikipedia-gnome-sort = Wikipedia (englisch), Gnome sort, Abschnitt Pseudocode, nach D. Grune, Gnome Sort, 2000 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-gnome-sort = Wikipedia (englisch), Gnome sort
common-sort-source-wikipedia-odd-even-sort = Wikipedia (englisch), Odd–even sort, Abschnitt Algorithm, nach N. Habermann, Parallel Neighbor Sort, 1972 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-odd-even-sort = Wikipedia (englisch), Odd–even sort
common-sort-source-wikipedia-comb-sort = Wikipedia (englisch), Comb sort, Abschnitt Pseudocode, nach S. Lacey und R. Box, A Fast, Easy Sort, Byte 16(4), 1991 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-comb-sort = Wikipedia (englisch), Comb sort
common-sort-source-wikipedia-cycle-sort = Wikipedia (englisch), Cycle sort, Abschnitt Implementation, nach B. K. Haddon, Cycle-Sort: A Linear Sorting Method, The Computer Journal 33(4), 1990, DOI 10.1093/comjnl/33.4.365 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-cycle-sort = Wikipedia (englisch), Cycle sort
common-sort-source-wikipedia-pancake-sorting = Wikipedia (englisch), Pancake sorting, Abschnitt Algorithm, nach W. H. Gates und C. H. Papadimitriou, Bounds for Sorting by Prefix Reversal, Discrete Mathematics 27, 1979, DOI 10.1016/0012-365X(79)90068-2 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-pancake-sorting = Wikipedia (englisch), Pancake sorting
common-sort-source-wikipedia-shellsort = Wikipedia (englisch), Shellsort, Abschnitte Example und Gap sequences, nach D. L. Shell, A high-speed sorting procedure, Communications of the ACM 2(7), 1959, D. E. Knuth, The Art of Computer Programming, Bd. 3, 1973, und M. Ciura, Best Increments for the Average Case of Shellsort, 2001 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-shellsort = Wikipedia (englisch), Shellsort
common-sort-source-wikipedia-merge-sort-bottom-up = Wikipedia (englisch), Merge sort, Abschnitt Bottom-up implementation, nach D. E. Knuth, The Art of Computer Programming, Bd. 3, Abschnitt 5.2.4 (CC BY-SA 4.0)
common-sort-source-wikipedia-merge-sort-natural = Wikipedia (englisch), Merge sort, Abschnitt Natural merge sort, nach D. E. Knuth, The Art of Computer Programming, Bd. 3, Abschnitt 5.2.4 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-merge-sort = Wikipedia (englisch), Merge sort
common-sort-source-wikipedia-radix-sort = Wikipedia (englisch), Radix sort, Abschnitt Least significant digit, nach D. E. Knuth, The Art of Computer Programming, Bd. 3, Abschnitt 5.2.5 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-radix-sort = Wikipedia (englisch), Radix sort
common-sort-source-arulanandham-calude-dinneen-2002 = J. J. Arulanandham, C. S. Calude, M. J. Dinneen, Bead-Sort: A Natural Sorting Algorithm, Bulletin of the EATCS 76 (2002), und Wikipedia (englisch), Bead sort (CC BY-SA 4.0)
common-sort-source-short-arulanandham-calude-dinneen-2002 = Arulanandham, Calude und Dinneen 2002
common-sort-source-batcher-1968-bitonic = K. E. Batcher, Sorting networks and their applications, Proceedings of the AFIPS Spring Joint Computer Conference 1968, 307–314, und Wikipedia (englisch), Bitonic sorter (CC BY-SA 4.0)
common-sort-source-short-batcher-1968-bitonic = Batcher 1968
common-sort-source-gruber-holzer-ruepp-2007 = H. Gruber, M. Holzer, O. Ruepp, Sorting the Slow Way: An Analysis of Perversely Awful Randomized Sorting Algorithms, Fun with Algorithms 2007, LNCS 4475, 183–197
common-sort-source-short-gruber-holzer-ruepp-2007 = Gruber, Holzer und Ruepp 2007
common-sort-form-full = full: n - 1 Durchläufe, jeder über die ganze Liste
common-sort-form-shrinking = shrinking: n - 1 Durchläufe, jeder endet vor den Einträgen, die frühere Durchläufe platziert haben
common-sort-form-early-exit = early_exit: kürzer werdende Durchläufe, Schluss nach einem Durchlauf ohne Vertauschung
common-sort-form-last-exchange = last_exchange: jeder Durchlauf endet dort, wo der vorige zuletzt vertauscht hat (Knuths Algorithmus B)
common-sort-shaker-form-full = full: jeder Durchlauf geht über die ganze Liste, und das Sortieren endet nach einem Durchlauf ohne Vertauschung
common-sort-shaker-form-shrinking = shrinking: jeder Durchlauf endet einen Eintrag vor dem Durchlauf auf seiner Seite, und das Sortieren endet nach einem Durchlauf ohne Vertauschung
common-sort-shaker-form-last-exchange = last_exchange: jeder Durchlauf endet dort, wo der vorige in derselben Richtung zuletzt vertauscht hat
common-sort-odd-even-form-until-sorted = until_sorted: Runden aus den ungeraden, dann den geraden Paaren, bis eine Runde nichts vertauscht
common-sort-odd-even-form-fixed-passes = fixed_passes: genau n Phasen, abwechselnd ungerade und gerade Paare, die immer genügen
common-sort-comb-form-lacey-box = lacey_box: die Lücke beginnt bei n und wird vor jedem Durchlauf floor(10*gap/13), aus 9 und 10 wird 11, und Durchläufe mit Lücke 1 wiederholen sich, bis einer nichts vertauscht (Lacey und Box 1991)
common-sort-gaps-shell = shell: die Lücken n/2, n/4, … abgerundet, bis 1 (Shell 1959)
common-sort-gaps-knuth = knuth: die Lücken 1, 4, 13, 40, … = (3^k − 1)/2, die nicht größer als ceil(n/3) und kleiner als n sind, die größte zuerst (Knuth 1973, nach Pratt 1971)
common-sort-gaps-ciura = ciura: die Lücken 701, 301, 132, 57, 23, 10, 4, 1, die kleiner als n sind, wie Ciura sie 2001 fand; die Folge wird über 701 hinaus nicht verlängert
common-sort-gaps-used = { $gaps } bei n = { $length }, die Lücken, die die Regel darüber bei diesem n gibt, die größte zuerst
common-sort-gaps-used-none = keine bei n = { $length }: die Regel darüber gibt bei diesem n keine Lücke, deshalb gab es keinen Durchlauf
common-sort-partition-lomuto-last = lomuto mit pivot=last: der letzte Eintrag ist das Pivot, ein Durchlauf von links bringt jeden Eintrag, der nicht nach ihm kommt, nach vorn, dann nimmt das Pivot seinen Platz ein
common-sort-partition-hoare-first = hoare mit pivot=first: der erste Eintrag ist das Pivot, zwei Durchläufe laufen aufeinander zu und vertauschen die Paare, die auf der falschen Seite stehen
common-sort-partition-lomuto-random = lomuto mit pivot=random: für jeden Teil zieht philox4x32_10 eine seiner Positionen, jede gleich wahrscheinlich, dieser Eintrag wird auf die letzte Position getauscht und ist das Pivot, dann läuft die Aufteilung wie mit pivot=last
common-sort-not-in-closed-form = nicht angegeben: calc kennt dafür keine geschlossene Form, die es auswertet
common-method-insertion-sort = durch Sortieren durch Einfügen, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; der beiseitegelegte Eintrag wird auch zurückgeschrieben, wo er bleibt
common-method-binary-insertion-sort = durch binäres Sortieren durch Einfügen, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; ein Eintrag wird nur geschrieben, wo er sich bewegt
common-method-selection-sort = durch Sortieren durch Auswahl, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen
common-method-bubble-sort = durch Bubblesort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe
common-method-merge-sort = durch Mergesort von oben nach unten, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; jedes Mischen schreibt in einen Zwischenspeicher und kopiert zurück
common-method-heap-sort = durch Heapsort, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen
common-method-quick-sort = durch Quicksort, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen
common-method-counting-sort = durch Countingsort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; es vergleicht nie, es zählt jeden Schlüssel, summiert die Zähler, setzt jeden Eintrag von hinten in eine Ausgabeliste und kopiert sie zurück
common-method-double-selection-sort = durch doppeltes Auswahlsortieren, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen; jeder Durchlauf vergleicht den Rest paarweise, um seinen kleinsten und größten Eintrag zu finden, und setzt sie an seine beiden Enden
common-method-cocktail-shaker-sort = durch Cocktail-Shaker-Sort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; es läuft abwechselnd vorwärts und rückwärts und vertauscht jedes Nachbarpaar in falscher Reihenfolge
common-method-gnome-sort = durch Gnomesort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; es geht vorwärts, solange zwei Nachbarn in Reihenfolge sind, und wo sie es nicht sind, vertauscht es sie und geht zurück
common-method-odd-even-sort = durch Odd-even-Sort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; es vergleicht die Nachbarn an ungeraden Positionen, dann die an geraden, und vertauscht jedes Paar in falscher Reihenfolge
common-method-comb-sort = durch Combsort, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen; es vergleicht Einträge im Abstand einer Lücke und vertauscht sie, wenn sie falsch stehen, wobei die Lücke von Durchlauf zu Durchlauf kleiner wird
common-method-cycle-sort = durch Cyclesort, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen; es zählt die Einträge, die kleiner als ein Eintrag sind, um seinen Platz zu finden, und bringt ihn dorthin, sodass jeder Eintrag höchstens einmal geschrieben wird
common-method-pancake-sort = durch Pfannkuchensortieren, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen; es dreht nur Anfänge der Liste um, erst den größten unsortierten Eintrag nach vorn, dann an seinen Platz
common-method-shell-sort = durch Shellsort, das nicht stabil ist: Einträge mit gleichem Schlüssel können ihre Reihenfolge aus der Eingabe verlassen; für jede Lücke der Folge, die größte zuerst und 1 zuletzt, sortiert es die Einträge im Abstand dieser Lücke durch Einfügen
common-method-bottom-up-merge-sort = durch Bottom-up-Mergesort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; es mischt Läufe aus 1, dann 2, dann 4 Einträgen und so weiter, jeder Durchlauf über die ganze Liste, und kopiert dabei jeden Eintrag in eine Hilfsliste und zurück
common-method-natural-merge-sort = durch natürliches Mergesort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; es sucht zuerst die Läufe, die schon geordnet sind, und mischt dann benachbarte Läufe paarweise, Durchlauf um Durchlauf, und kopiert dabei jeden Eintrag in eine Hilfsliste und zurück
common-method-radix-sort = durch Radixsort, das stabil ist: Einträge mit gleichem Schlüssel behalten ihre Reihenfolge aus der Eingabe; es sortiert nach der niedrigsten Ziffer jedes Schlüssels minus dem kleinsten Schlüssel, in der durch base= genannten Basis, dann nach der nächsten Ziffer und so fort, jeder Durchlauf ein Countingsort, das nie vergleicht
common-method-bead-sort = durch Beadsort: jede Zahl ist eine Reihe aus so vielen Perlen, eine auf jedem Stab vom ersten an; die Perlen fallen, sodass jeder Stab eine Perle für jede Zahl behält, die ihn erreicht, und jede Reihe wird als die Zahl ihrer Perlen zurückgelesen; es vergleicht nie und baut die Werte aus den Perlen neu, statt die Einträge zu bewegen
common-method-bitonic-sort = durch Bitonic Sort, das nicht stabil ist: ein Netz, das Blöcke aus 2, 4, 8, … Einträgen sortiert, jeder Block aus einer steigenden und einer fallenden Hälfte, gemischt, indem Einträge im Abstand eines halben Blocks verglichen werden, dann eines Viertels und so fort; es macht dieselben Vergleiche, welche Einträge es auch sind
common-method-bogo-sort = durch Bogosort, das nicht stabil ist: es prüft, ob die Liste sortiert ist, indem es Nachbarn von links vergleicht bis zum ersten Paar in falscher Reihenfolge, und solange sie es nicht ist, mischt es die ganze Liste nach Fisher–Yates mit Stellen, die aus philox4x32_10 gezogen werden; es endet mit Wahrscheinlichkeit 1, hat aber keine Schranke, deshalb begrenzt limit= das Mischen
common-method-philox4x32-10 = durch Philox4x32-10, den zählerbasierten Zufallszahlengenerator von J. K. Salmon, M. A. Moraes, R. O. Dror und D. E. Shaw, Parallel random numbers: as easy as 1, 2, 3, SC11, 2011, DOI 10.1145/2063384.2063405; das Ergebnis ist sein Block aus vier Wörtern, jedes eine ganze Zahl von 0 bis 2^32 − 1
common-result-kind-machine-float-64 = Maschinenzahl, 64 Bit (f64)
common-result-kind-machine-float-32 = Maschinenzahl, 32 Bit (f32)
common-record-propagation = Fortpflanzung
common-record-propagated-first-order = erster Ordnung aus { $count ->
    [one] { $count } unkorrelierten Eingabe
   *[other] { $count } unkorrelierten Eingaben
}
common-solve-line-ways = { $count ->
    [one] { $count } Weg gefunden
   *[other] { $count } Wege gefunden
}
common-solve-line-solved = { $count ->
    [one] gelöst entlang { $count } Wegs
   *[other] gelöst entlang { $count } Wegen
}
common-solve-line-not-reached = { $count ->
    [0] die Regeln führen aus keiner weiteren Eingabe dorthin
    [one] { $count } weitere Eingabe würde es erreichen
   *[other] { $count } weitere Eingabemengen würden es erreichen
}
common-solve-line-reachable = { $count ->
    [0] aus dem Gegebenen folgt keine weitere Größe
    [one] { $count } Größe folgt aus dem Gegebenen
   *[other] { $count } Größen folgen aus dem Gegebenen
}
common-result-kind-exact-integer = exakte ganze Zahl
common-result-kind-exact-decimal = exakte Dezimalzahl
common-result-kind-exact-fraction = exakter Bruch
common-result-kind-proven-range = ein Bereich, der jeden Wert enthält, den die Toleranzen der Eingaben zulassen; beide Enden sind exakt
common-record-rounding-at-most = höchstens { $bound }, absolut
common-result-kind-derived-from-measured = { $count ->
    [one] abgeleitet aus { $count } Messwert
   *[other] abgeleitet aus { $count } Messwerten
}
common-record-input = Eingabe
common-record-digits = Ziffern
common-value-digit-count = { $count ->
    [one] { $count } Ziffer
   *[other] { $count } Ziffern
}
common-value-fraction-digit-counts = { $numerator } und { $denominator } Ziffern
common-solve-bound-at-most = höchstens { $expression }, aus { $inputs }
common-solve-bound-at-least = mindestens { $expression }, aus { $inputs }
common-solve-bound-strictly-between = größer als { $lower } und kleiner als { $upper }, aus { $inputs }
common-solve-bound-from-inputs = aus { $inputs }

common-unit-override-system-not-applied = das Einheitensystem { $system } wurde nicht gefunden und wird nicht angewendet
common-unit-override-kind-not-applied = die Größenart { $kind } wurde nicht gefunden, ihre Einheit wird nicht angewendet
common-unit-override-unit-not-applied = die Einheit { $unit } für { $kind } wurde nicht gefunden und wird nicht angewendet
common-kind-unit-not-a-unit = { $unit } ist keine Einheit
common-kind-unit-other-dimension = { $unit } ist keine Einheit für { $kind }
common-kind-unit-compound-not-allowed = { $kind } wird in einer Einheit gezeigt, nicht in mehreren
common-kind-unit-compound-not-descending = die Einheiten von { $unit } müssen von der größten zur kleinsten gehen
common-kind-unit-scale-required = { $kind } wird auf einer Skala gezeigt: kelvin, celsius oder fahrenheit
common-kind-unit-not-a-display-unit = { $unit } kann nicht als Einheit gewählt werden, in der { $kind } gezeigt wird

common-quantity-kind-length = Länge
common-quantity-kind-area = Fläche
common-quantity-kind-volume = Volumen
common-quantity-kind-angle = Winkel
common-quantity-kind-count = Anzahl
common-quantity-kind-ratio = Verhältnis
common-quantity-kind-mass = Masse
common-quantity-kind-time = Zeit
common-quantity-kind-speed = Geschwindigkeit
common-quantity-kind-force = Kraft
common-quantity-kind-energy = Energie
common-quantity-kind-temperature = Temperatur
common-quantity-kind-temperature-difference = Temperaturdifferenz
common-view-decimal-places = Nachkommastellen
common-view-digits-of = Ziffern von
common-view-encloses = schließt ein
common-view-of-exact-value = dem exakten Wert
common-view-of-machine-value = dem Maschinenwert
common-view-encloses-exact-value = den exakten Wert
common-view-encloses-machine-value = den Maschinenwert
common-view-truncated = abgeschnitten
common-view-remainder = Rest
common-view-remainder-zero = 0, der Wert endet innerhalb dieser Stellen
common-view-uncertainty = Unsicherheit
common-view-uncertainty-below-place = kleiner als die letzte gezeigte Stelle
common-view-uncertainty-not-a-number = die Unsicherheit dieses Wertes ist keine einzelne Zahl
common-view-expansion = Entwicklung
common-view-expansion-finite = endlich
common-view-expansion-period = periodisch ab Stelle { $start }, Periodenlänge { $length }
common-view-expansion-period-beyond-limit = periodisch, die Periode endet nach Stelle { $limit }
common-view-expansion-not-known-to-recur = nicht als periodisch bekannt; die Ziffern stammen aus einer bewiesenen Einschließung
common-view-significant-digits = signifikante Stellen
common-view-lower = untere Schranke
common-view-upper = obere Schranke
common-view-width = Breite
common-view-precision = Genauigkeit
common-view-reached = erreicht
common-view-not-reached = an der Genauigkeitsgrenze beendet, Intervall wie bewiesen
common-unit-source-written = geschrieben
common-unit-source-this-session = diese Sitzung
common-unit-source-preference = deine Einstellung
common-unit-source-curriculum = Lehrplan { $name }
common-unit-source-stored-unit = gespeicherte Einheit
common-unit-source-as-computed = wie berechnet
common-record-stored-as = gespeichert als
common-picture-missing = fehlend
common-picture-unresolved = unaufgelöst
common-picture-may-be-hit = vielleicht getroffen
common-picture-marked = Genauigkeit erschöpft
common-picture-marked-columns = Genauigkeit erschöpft: { $marked } von { $columns } Spalten bei dieser Genauigkeit nicht aufgelöst
common-picture-width-columns = vielleicht getroffen: { $wide } von { $columns } Spalten bei dieser Breite nicht aufgelöst
common-picture-width-meaning = vielleicht getroffen: bei dieser Breite nicht aufgelöst
common-picture-below-bounds = die abgetasteten Werte schwanken weniger als ihre Rundungsschranken
common-picture-back-face = Rückseite
common-picture-provisional = wird noch berechnet
common-picture-grid-limit = Stellen bei diesem Maßstab erschöpft
common-picture-value-limit = Genauigkeit bei diesem Maßstab erschöpft
common-picture-unknown-bounds = Schranken unbekannt
common-picture-inside = innen
common-picture-undecided = unentschieden
common-picture-argument = Argument
common-picture-modulus = Betrag
common-picture-sketch = Skizze, nicht maßstäblich
common-picture-unit-joiner = in
common-picture-angle-arc = Winkel
common-picture-right-angle = rechter Winkel
common-picture-equal-sides = gleiche Seiten
common-picture-direction = Richtung
common-picture-hypotenuse = Hypotenuse
common-picture-leg = Kathete
common-picture-height = Höhe
common-picture-real-part = Realteil von { $name }
common-picture-imaginary-part = Imaginärteil von { $name }

# The language reference
common-reference-group-numbers = Zahlen
common-reference-group-uncertainty = Unsicherheit
common-reference-group-units = Einheiten
common-reference-group-operators = Rechenzeichen
common-reference-group-relations = Vergleiche und Logik
common-reference-group-calls = Funktionsaufrufe
common-reference-group-keyword-arguments = Schlüsselwortargumente
common-reference-group-arrays = Listen und Matrizen
common-reference-group-binders = Binder
common-reference-group-statements = Anweisungen

common-reference-constant-pi = Pi
common-reference-constant-pi-meaning = das Verhältnis von Umfang zu Durchmesser eines Kreises
common-reference-constant-e = e
common-reference-constant-e-meaning = die eulersche Zahl, die Basis des natürlichen Logarithmus
common-reference-constant-imaginary-unit = imaginäre Einheit
common-reference-constant-imaginary-unit-meaning = die Zahl, deren Quadrat minus eins ist
common-reference-constant-infinity = Unendlich
common-reference-constant-infinity-meaning = ein Wert über jeder Zahl, mit Minus davor unter jeder Zahl
common-reference-typed-literal-rational = exakter Bruch
common-reference-typed-literal-rational-meaning = ein Bruch, der exakt bleibt, Zähler durch Nenner
common-reference-typed-literal-f32 = 32-Bit-Maschinenzahl
common-reference-typed-literal-f32-meaning = eine als 32-Bit-Gleitkommazahl gespeicherte Zahl
common-reference-typed-literal-f64 = 64-Bit-Maschinenzahl
common-reference-typed-literal-f64-meaning = eine als 64-Bit-Gleitkommazahl gespeicherte Zahl
common-reference-literal-decimal = Dezimalzahl
common-reference-literal-decimal-meaning = eine Zahl mit Dezimalpunkt, die exakt bleibt
common-reference-literal-exponent = Zehnerpotenzform
common-reference-literal-exponent-meaning = eine Zahl, gefolgt von e und der Zehnerpotenz, mit der sie multipliziert wird
common-reference-literal-line-label = Zeilenmarke
common-reference-literal-line-label-meaning = das Ergebnis einer früheren Zeile, r und die Nummer der Zeile
common-reference-operator-uncertain = Messwert
common-reference-operator-uncertain-meaning = ein Wert mit seiner Standardunsicherheit
common-reference-operator-uncertain-expanded = Messwert mit Erweiterungsfaktor
common-reference-operator-uncertain-expanded-meaning = ein Wert mit erweiterter Unsicherheit und dem Faktor k, mit dem sie multipliziert wurde
common-reference-operator-convert-unit = Einheitenumrechnung
common-reference-operator-convert-unit-meaning = dieselbe Größe in einer anderen Einheit geschrieben
common-reference-form-quantity = Größe
common-reference-form-quantity-meaning = eine Zahl oder eine Konstante wie pi mit einer Einheit dahinter
common-reference-form-unit-expression = Einheitenausdruck
common-reference-form-unit-expression-meaning = Einheiten, multipliziert, dividiert und potenziert
common-reference-form-degree-sign = Grad
common-reference-form-degree-sign-meaning = ein Winkel in Grad, als deg oder mit dem Gradzeichen geschrieben
common-reference-operator-add = Addition
common-reference-operator-add-meaning = die Summe zweier Werte
common-reference-operator-sub = Subtraktion
common-reference-operator-sub-meaning = die Differenz zweier Werte
common-reference-operator-mul = Multiplikation
common-reference-operator-mul-meaning = das Produkt zweier Werte
common-reference-operator-div = Division
common-reference-operator-div-meaning = der Quotient zweier Werte
common-reference-operator-neg = Vorzeichenwechsel
common-reference-operator-neg-meaning = der Wert mit umgekehrtem Vorzeichen
common-reference-operator-pow = Potenz
common-reference-operator-pow-meaning = die Basis hoch den Exponenten
common-reference-operator-factorial = Fakultät
common-reference-operator-factorial-meaning = das Produkt der ganzen Zahlen von 1 bis n
common-reference-operator-percent = Prozent
common-reference-operator-percent-meaning = die Zahl geteilt durch hundert
common-reference-operator-round = runden
common-reference-operator-round-meaning = die nächste ganze Zahl, eine Hälfte von der Null weg
common-reference-operator-binomial = Binomialkoeffizient
common-reference-operator-binomial-meaning = auf wie viele Arten man k Dinge aus n wählen kann
common-reference-operator-sinh = Sinus hyperbolicus
common-reference-operator-sinh-meaning = die halbe Differenz von e hoch x und e hoch minus x
common-reference-operator-cosh = Kosinus hyperbolicus
common-reference-operator-cosh-meaning = die halbe Summe von e hoch x und e hoch minus x
common-reference-operator-tanh = Tangens hyperbolicus
common-reference-operator-tanh-meaning = der Sinus hyperbolicus geteilt durch den Kosinus hyperbolicus
common-reference-operator-log = Logarithmus zu einer Basis
common-reference-operator-log-meaning = die Hochzahl, mit der die Basis x ergibt
common-reference-operator-log2 = Zweierlogarithmus
common-reference-operator-log2-meaning = die Hochzahl, mit der 2 x ergibt
common-reference-operator-log10 = Zehnerlogarithmus
common-reference-operator-log10-meaning = die Hochzahl, mit der 10 x ergibt
common-reference-operator-gcd = größter gemeinsamer Teiler
common-reference-operator-gcd-meaning = die größte ganze Zahl, die beide teilt
common-reference-operator-count = Anzahl
common-reference-operator-count-meaning = wie viele Einträge eine Liste oder Matrix hat
common-reference-operator-total = Summe
common-reference-operator-total-meaning = die Summe aller Einträge, exakt
common-reference-operator-mean = arithmetisches Mittel
common-reference-operator-mean-meaning = die Summe der Einträge geteilt durch ihre Anzahl
common-reference-operator-median = Median
common-reference-operator-median-meaning = der mittlere Eintrag nach dem Sortieren, bei gerader Anzahl das Mittel der beiden mittleren
common-reference-operator-transpose = Transponierte
common-reference-operator-transpose-meaning = die Matrix mit vertauschten Zeilen und Spalten
common-reference-operator-determinant = Determinante
common-reference-operator-determinant-meaning = die Zahl, die angibt, um welchen Faktor eine quadratische Matrix ein Volumen streckt
common-reference-operator-trace = Spur
common-reference-operator-trace-meaning = die Summe der Einträge auf der Diagonale einer quadratischen Matrix
common-reference-operator-inverse = Inverse
common-reference-operator-inverse-meaning = die Matrix, die mit einer quadratischen Matrix multipliziert die Einheitsmatrix ergibt
common-reference-operator-rank = Rang
common-reference-operator-rank-meaning = die Anzahl linear unabhängiger Zeilen einer Matrix
common-reference-operator-eigenvalues = Eigenwerte
common-reference-operator-eigenvalues-meaning = die reellen Zahlen t, für die die Matrix minus t mal die Einheitsmatrix nicht invertierbar ist
common-reference-operator-kernel = Kern
common-reference-operator-kernel-meaning = eine Basis der Vektoren, die eine Matrix auf null abbildet, eine Spalte je Basisvektor
common-reference-operator-eigenvectors = Eigenvektoren
common-reference-operator-eigenvectors-meaning = eine Basis jedes Eigenraums, als Spalten in der Reihenfolge der Eigenwerte von der kleinsten an
common-reference-operator-at = Eintrag
common-reference-operator-at-meaning = der Eintrag einer Liste an dieser Stelle, von eins an gezählt
common-reference-operator-bit-and = bitweises Und
common-reference-operator-bit-and-meaning = die ganze Zahl, deren Bits dort gesetzt sind, wo beide ganzen Zahlen ab 0 sie gesetzt haben
common-reference-operator-bit-or = bitweises Oder
common-reference-operator-bit-or-meaning = die ganze Zahl, deren Bits dort gesetzt sind, wo eine der ganzen Zahlen ab 0 sie gesetzt hat
common-reference-operator-bit-xor = bitweises exklusives Oder
common-reference-operator-bit-xor-meaning = die ganze Zahl, deren Bits dort gesetzt sind, wo genau eine der beiden ganzen Zahlen ab 0 sie gesetzt hat; ^ bleibt die Potenz
common-reference-operator-shift-left = nach links schieben
common-reference-operator-shift-left-meaning = x mal 2 hoch n, exakt
common-reference-operator-shift-right = nach rechts schieben
common-reference-operator-shift-right-meaning = x geteilt durch 2 hoch n, abgerundet, für x ab 0
common-reference-operator-bit-not = bitweises Nicht in einem Typ
common-reference-operator-bit-not-meaning = jedes Bit von x umgedreht innerhalb des Typs, in dem es liegt, etwa u8 oder i16
common-reference-operator-wrap = in einen Typ umbrechen
common-reference-operator-wrap-meaning = x in den Bereich des Typs gebracht, um ganze Vielfache von 2 hoch seiner Breite, so wie ein Register dieser Breite es hielte
common-reference-operator-in-type = Wert in einem Typ
common-reference-operator-in-type-meaning = x als Wert des Typs, abgelehnt, wo es außerhalb des Bereichs des Typs liegt
common-reference-operator-radix = in einer Basis
common-reference-operator-radix-meaning = eine ganze Zahl in Basis 16, 2, 8 oder 10 geschrieben; mit einem Typ wird eine negative Zahl als ihr Zweierkomplement in der Breite des Typs geschrieben
common-reference-operator-bytes = als Bytes
common-reference-operator-bytes-meaning = eine ganze Zahl als ihre Bytes in der genannten Reihenfolge, be für das höchstwertige zuerst, le für das niederwertigste
common-reference-operator-between = Wert in einem Bereich
common-reference-operator-between-meaning = ein Wert irgendwo zwischen den beiden Enden, beide eingeschlossen; die Zeile antwortet dann mit ihrem kleinsten und größten Wert
common-reference-operator-tolerance = Wert mit Toleranz
common-reference-operator-tolerance-meaning = x mit einer Toleranz des Anteils p nach beiden Seiten, wie ein Datenblatt 25 Ω ± 10 % schreibt; anders als ±, das eine Messunsicherheit angibt, nennt es harte Grenzen
common-reference-operator-substance = chemischer Stoff
common-reference-operator-substance-meaning = ein Stoff, geschrieben mit seiner Formel, wie chem'H2O'; allein nennt er die Zusammensetzung, die calc gelesen hat
common-reference-operator-reaction = chemische Reaktion
common-reference-operator-reaction-meaning = eine Reaktion, geschrieben als chem'Edukte -> Produkte'; ohne Koeffizienten wird sie exakt ausgeglichen, und mit einem geschriebenen Koeffizienten geprüft, wobei ein fehlender als 1 zählt
common-reference-operator-molar-mass = molare Masse
common-reference-operator-molar-mass-meaning = die molare Masse eines Stoffs als Bereich in g/mol, aus den Standardatomgewichten CIAAW 2024 und der molaren Massenkonstante CODATA 2022
common-reference-typed-literal-chemistry = chemische Formel
common-reference-typed-literal-chemistry-meaning = eine chemische Formel oder Reaktion zwischen chem' und ', gelesen mit eigener Grammatik und nie als Einheiten oder Namen
common-reference-operator-nuclide = Nuklid
common-reference-operator-nuclide-meaning = ein Kern oder ein Teilchen, geschrieben als nuc'^14C'; allein nennt es, was calc gelesen hat
common-reference-operator-nuclear-reaction = Kernreaktion
common-reference-operator-nuclear-reaction-meaning = eine Kernreaktion, geschrieben als nuc'Edukte -> Produkte'; sie wird geprüft, wie sie geschrieben ist, wobei ein fehlender Koeffizient als 1 zählt, und exakt ausgeglichen, wo das scheitert und es genau einen Ausgleich gibt
common-reference-operator-q-value = Q-Wert
common-reference-operator-q-value-meaning = die Energie, die eine Kernreaktion freisetzt, als Bereich in keV, aus den Atommassen AME2020 und den Konstanten CODATA 2022
common-reference-operator-philox4x32-10 = Philox4x32-10
common-reference-operator-philox4x32-10-meaning = die vier 32-Bit-Wörter des zählerbasierten Zufallszahlengenerators Philox4x32-10 bei einem Seed, einem Strom und einem Index, jeder von 0 bis 2^64 − 1; nicht für Kryptographie
common-reference-operator-rational-part = rationaler Teil
common-reference-operator-rational-part-meaning = der rationale Teil eines exakten Werts aus rationalen Zahlen und Quadratwurzeln: 17 in 17 + 12 * sqrt(2)
common-reference-operator-coefficient-of = Koeffizient eines Terms
common-reference-operator-coefficient-of-meaning = der Koeffizient eines Terms in calcs Normalform eines Werts; der Term ist sqrt(n) mit n ohne quadratische Faktoren: 12 für sqrt(2) in 17 + 12 * sqrt(2)
common-reference-operator-re = Realteil
common-reference-operator-re-meaning = der Realteil einer komplexen Zahl: 3 in 3 + 4 * i
common-reference-operator-im = Imaginärteil
common-reference-operator-im-meaning = der Imaginärteil einer komplexen Zahl, eine reelle Zahl: 4 in 3 + 4 * i
common-reference-operator-conj = konjugiert komplexe Zahl
common-reference-operator-conj-meaning = die konjugiert komplexe Zahl, dieselbe Zahl mit umgekehrtem Vorzeichen des Imaginärteils: 3 - 4 * i zu 3 + 4 * i
common-reference-typed-literal-nuclear = Kernschreibweise
common-reference-typed-literal-nuclear-meaning = ein Nuklid oder eine Kernreaktion zwischen nuc' und ', gelesen mit eigener Grammatik und nie als Einheiten oder Namen
common-reference-operator-lcm = kleinstes gemeinsames Vielfaches
common-reference-operator-lcm-meaning = die kleinste positive ganze Zahl, die beide teilen
common-reference-operator-mod = Rest
common-reference-operator-mod-meaning = was von a bleibt, wenn man ganze Vielfache von b abzieht, nie negativ
common-reference-operator-equal = Gleichheit
common-reference-operator-equal-meaning = gilt, wenn beide Seiten denselben Wert haben
common-reference-operator-not-equal = Ungleichheit
common-reference-operator-not-equal-meaning = gilt, wenn sich die beiden Seiten unterscheiden
common-reference-operator-less = kleiner
common-reference-operator-less-meaning = gilt, wenn die linke Seite unter der rechten liegt
common-reference-operator-less-or-equal = kleiner oder gleich
common-reference-operator-less-or-equal-meaning = gilt, wenn die linke Seite nicht über der rechten liegt
common-reference-operator-greater = größer
common-reference-operator-greater-meaning = gilt, wenn die linke Seite über der rechten liegt
common-reference-operator-greater-or-equal = größer oder gleich
common-reference-operator-greater-or-equal-meaning = gilt, wenn die linke Seite nicht unter der rechten liegt
common-reference-operator-and = Und
common-reference-operator-and-meaning = gilt, wenn beide Seiten gelten
common-reference-operator-or = Oder
common-reference-operator-or-meaning = gilt, wenn mindestens eine Seite gilt
common-reference-operator-not = Nicht
common-reference-operator-not-meaning = gilt, wenn sein Operand nicht gilt
common-reference-operator-sqrt = Quadratwurzel
common-reference-operator-sqrt-meaning = der Wert ab null, dessen Quadrat der Operand ist
common-reference-operator-abs = Betrag
common-reference-operator-abs-meaning = der Wert ohne sein Vorzeichen
common-reference-operator-mul-add = Multiplizieren und Addieren
common-reference-operator-mul-add-meaning = x mal y plus z, einmal statt zweimal gerundet
common-reference-operator-floor = Abrunden
common-reference-operator-floor-meaning = die größte ganze Zahl, die nicht über dem Wert liegt
common-reference-operator-ceil = Aufrunden
common-reference-operator-ceil-meaning = die kleinste ganze Zahl, die nicht unter dem Wert liegt
common-reference-operator-trunc = Abschneiden
common-reference-operator-trunc-meaning = der ganze Teil des Wertes, zur Null hin
common-reference-operator-round-ties-even = Runden, Hälfte zur geraden Zahl
common-reference-operator-round-ties-even-meaning = die nächste ganze Zahl, bei genau einer Hälfte die gerade
common-reference-operator-copysign = Vorzeichen übernehmen
common-reference-operator-copysign-meaning = der erste Wert mit dem Vorzeichen des zweiten
common-reference-operator-min = kleinerer Wert
common-reference-operator-min-meaning = der kleinere zweier Werte
common-reference-operator-max = größerer Wert
common-reference-operator-max-meaning = der größere zweier Werte
common-reference-operator-exp = Exponentialfunktion
common-reference-operator-exp-meaning = e hoch den Wert
common-reference-operator-ln = natürlicher Logarithmus
common-reference-operator-ln-meaning = der Exponent, mit dem e den Wert ergibt
common-reference-operator-sin = Sinus
common-reference-operator-sin-meaning = der Sinus eines Winkels
common-reference-operator-cos = Kosinus
common-reference-operator-cos-meaning = der Kosinus eines Winkels
common-reference-operator-tan = Tangens
common-reference-operator-tan-meaning = der Tangens eines Winkels
common-reference-operator-asin = Arkussinus
common-reference-operator-asin-meaning = der Winkel, dessen Sinus der Wert ist
common-reference-operator-acos = Arkuskosinus
common-reference-operator-acos-meaning = der Winkel, dessen Kosinus der Wert ist
common-reference-operator-atan = Arkustangens
common-reference-operator-atan-meaning = der Winkel, dessen Tangens der Wert ist
common-reference-operator-atan2 = Arkustangens zweier Werte
common-reference-operator-atan2-meaning = der Winkel des Punktes x, y, gemessen von der positiven x-Achse
common-reference-operator-select = Auswahl
common-reference-operator-select-meaning = der zweite Wert, wo die Bedingung gilt, sonst der dritte
common-reference-operator-complex = komplexe Zahl
common-reference-operator-complex-meaning = die Zahl mit dem gegebenen Real- und Imaginärteil
common-reference-operator-to-f32 = als 32-Bit-Maschinenzahl
common-reference-operator-to-f32-meaning = der Wert, auf eine 32-Bit-Gleitkommazahl gerundet
common-reference-operator-to-f64 = als 64-Bit-Maschinenzahl
common-reference-operator-to-f64-meaning = der Wert, auf eine 64-Bit-Gleitkommazahl gerundet
common-reference-operator-exact = exakter Wert
common-reference-operator-exact-meaning = der Wert als exakte Zahl, ohne Maschinenarithmetik gerechnet
common-reference-operator-enclosure-lower = die untere Grenze eines bewiesenen Einschlusses
common-reference-operator-enclosure-lower-meaning = die größte Dezimalzahl mit n signifikanten Stellen, die höchstens der Wert ist, bewiesen
common-reference-operator-enclosure-upper = die obere Grenze eines bewiesenen Einschlusses
common-reference-operator-enclosure-upper-meaning = die kleinste Dezimalzahl mit n signifikanten Stellen, die mindestens der Wert ist, bewiesen
common-reference-operator-from-celsius = aus Grad Celsius
common-reference-operator-from-celsius-meaning = eine Angabe in Grad Celsius als Temperatur
common-reference-operator-from-fahrenheit = aus Grad Fahrenheit
common-reference-operator-from-fahrenheit-meaning = eine Angabe in Grad Fahrenheit als Temperatur
common-reference-operator-to-celsius = in Grad Celsius
common-reference-operator-to-celsius-meaning = eine Temperatur als Angabe in Grad Celsius
common-reference-operator-to-fahrenheit = in Grad Fahrenheit
common-reference-operator-to-fahrenheit-meaning = eine Temperatur als Angabe in Grad Fahrenheit
common-reference-keyword-argument-shape = Form
common-reference-keyword-argument-shape-meaning = wie eine Summe oder ein Produkt beim Rechnen gruppiert wird
common-reference-keyword-argument-side = Seite
common-reference-keyword-argument-side-meaning = die Seite, von der ein Grenzwert genommen wird
common-reference-keyword-argument-coverage = Erweiterungsfaktor
common-reference-keyword-argument-coverage-meaning = der Faktor, mit dem eine Standardunsicherheit multipliziert wurde
common-reference-form-array = Liste
common-reference-form-array-meaning = Werte in Reihenfolge, zwischen eckigen Klammern
common-reference-form-matrix = Matrix
common-reference-form-matrix-meaning = Zeilen von Werten, mit Semikolon zwischen den Zeilen
common-reference-binder-lambda = Funktion
common-reference-binder-lambda-meaning = eine Funktion ihrer Variablen: die Variable, dann der Term
common-reference-binder-sum = Summe
common-reference-binder-sum-meaning = die Summe des Terms über die Variable, von der unteren bis zur oberen Grenze
common-reference-binder-sum-halving = Summe in Halbierungsform
common-reference-binder-sum-halving-meaning = dieselbe Summe, hälftig addiert, was den Rundungsfehler kleiner hält
common-reference-binder-product = Produkt
common-reference-binder-product-meaning = das Produkt des Terms über die Variable, von der unteren bis zur oberen Grenze
common-reference-binder-product-halving = Produkt in Halbierungsform
common-reference-binder-product-halving-meaning = dasselbe Produkt, hälftig multipliziert, was den Rundungsfehler kleiner hält
common-reference-binder-integral = Integral
common-reference-binder-integral-meaning = das Integral des Terms über die Variable zwischen den beiden Grenzen
common-reference-binder-limit = Grenzwert
common-reference-binder-limit-meaning = der Wert, dem der Term zustrebt, wenn die Variable zur Stelle geht
common-reference-binder-limit-left = Grenzwert von links
common-reference-binder-limit-left-meaning = der Grenzwert über Werte unterhalb der Stelle
common-reference-binder-limit-right = Grenzwert von rechts
common-reference-binder-limit-right-meaning = der Grenzwert über Werte oberhalb der Stelle
common-reference-binder-derivative = Ableitung
common-reference-binder-derivative-meaning = die Ableitung des Terms nach der Variablen
common-reference-binder-root = reelle Nullstelle
common-reference-binder-root-meaning = die k-te reelle Nullstelle des Terms als Polynom in der Variablen, von der kleinsten an gezählt
common-reference-binder-taylor = Taylorpolynom
common-reference-binder-taylor-meaning = das Polynom vom Grad n um die Stelle a, dessen Ableitungen dort mit denen des Terms übereinstimmen
common-reference-binder-insertion-sort = Sortieren durch Einfügen
common-reference-binder-insertion-sort-meaning = die Liste, durch Einfügen sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen; order=decreasing sortiert nach fallendem Schlüssel
common-reference-binder-merge-sort = Mergesort
common-reference-binder-merge-sort-meaning = die Liste, durch Mergesort von oben nach unten sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-heap-sort = Heapsort
common-reference-binder-heap-sort-meaning = die Liste, durch Heapsort sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen; nicht stabil
common-reference-binder-quick-sort = Quicksort
common-reference-binder-quick-sort-meaning = die Liste, durch Quicksort mit der genannten Aufteilung und dem genannten Pivot sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen; nicht stabil
common-reference-binder-counting-sort = Countingsort
common-reference-binder-counting-sort-meaning = die Liste, durch Countingsort sortiert, nach dem Schlüssel, wenn einer angegeben ist, der eine ganze Zahl sein muss, mit gezählten Schreibvorgängen und Zählerschritten
common-reference-binder-double-selection-sort = doppeltes Auswahlsortieren
common-reference-binder-double-selection-sort-meaning = die Liste, durch doppeltes Auswahlsortieren sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-cocktail-shaker-sort = Cocktail-Shaker-Sort
common-reference-binder-cocktail-shaker-sort-meaning = die Liste, durch Cocktail-Shaker-Sort in der mit form= genannten Form sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-gnome-sort = Gnomesort
common-reference-binder-gnome-sort-meaning = die Liste, durch Gnomesort sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-odd-even-sort = Odd-even-Sort
common-reference-binder-odd-even-sort-meaning = die Liste, durch Odd-even-Transpositionssort in der mit form= genannten Form sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-comb-sort = Combsort
common-reference-binder-comb-sort-meaning = die Liste, durch Combsort in der mit form= genannten Form sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-cycle-sort = Cyclesort
common-reference-binder-cycle-sort-meaning = die Liste, durch Cyclesort sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-pancake-sort = Pfannkuchensortieren
common-reference-binder-pancake-sort-meaning = die Liste, durch Pfannkuchensortieren sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen, Schreibvorgängen und Umdrehungen
common-reference-binder-shell-sort = Shellsort
common-reference-binder-shell-sort-meaning = die Liste, durch Shellsort mit der durch gaps= genannten Lückenfolge sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-bottom-up-merge-sort = Bottom-up-Mergesort
common-reference-binder-bottom-up-merge-sort-meaning = die Liste, durch Bottom-up-Mergesort sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-natural-merge-sort = natürliches Mergesort
common-reference-binder-natural-merge-sort-meaning = die Liste, durch natürliches Mergesort sortiert, das die schon geordneten Läufe mischt, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-radix-sort = Radixsort
common-reference-binder-radix-sort-meaning = die Liste, durch Radixsort in der durch base= genannten Basis sortiert, nach dem Schlüssel, wenn einer angegeben ist, der eine ganze Zahl sein muss, mit gezählten Schreibvorgängen und Zählerschritten
common-reference-binder-bead-sort = Beadsort
common-reference-binder-bead-sort-meaning = die Liste ganzer Zahlen ab 0, durch Beadsort geordnet neu gebaut, mit jeder Perle gezählt, wenn sie fällt und wenn sie gelesen wird
common-reference-binder-bitonic-sort = Bitonic Sort
common-reference-binder-bitonic-sort-meaning = die Liste aus 2^k Einträgen, durch Batchers bitonisches Netz sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-bogo-sort = Bogosort
common-reference-binder-bogo-sort-meaning = die Liste, sortiert, indem sie gemischt wird, bis sie sortiert ist, mit Mischvorgängen aus philox4x32_10 mit dem angegebenen Seed und höchstens dem angegebenen Limit, mit gezählten Vergleichen, Schreibvorgängen und Ziehungen
common-reference-binder-binary-insertion-sort = binäres Sortieren durch Einfügen
common-reference-binder-binary-insertion-sort-meaning = die Liste, durch binäres Einfügen sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-binder-selection-sort = Sortieren durch Auswahl
common-reference-binder-selection-sort-meaning = die Liste, durch Auswahl sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen; nicht stabil
common-reference-binder-bubble-sort = Bubblesort
common-reference-binder-bubble-sort-meaning = die Liste, durch Bubblesort in der mit form= genannten Form sortiert, nach dem Schlüssel, wenn einer angegeben ist, mit gezählten Vergleichen und Schreibvorgängen
common-reference-form-comment = Kommentar
common-reference-form-comment-meaning = Text nach dem Rautezeichen, der nicht gerechnet wird
common-reference-statement-naming = Benennung
common-reference-statement-naming-meaning = gibt einem Wert einen Namen, den spätere Zeilen verwenden können
common-reference-statement-function-naming = Funktionsbenennung
common-reference-statement-function-naming-meaning = gibt einer Funktion ihrer Parameter einen Namen

common-working-steps = Rechenweg
common-working-step-reference = der Wert von { $line }
common-working-backend-unavailable = das Backend, das diese Zeile gerechnet hat, ist hier nicht verfügbar, daher werden ihre Schritte nicht gezeigt
common-working-backend-unavailable-named = das Backend { $backend }, das diese Zeile gerechnet hat, ist hier nicht verfügbar, daher werden ihre Schritte nicht gezeigt
common-working-approximate-operations = diese Zeile verwendet eine Operation, deren Ergebnis sich zwischen Backends unterscheiden kann, daher werden ihre Schritte nicht gezeigt
common-working-result-differs = der Rechenweg ergibt { $shown } für diese Zeile, was von ihrem Wert abweicht
common-working-further-steps = { $count ->
    [one] { $count } weiterer Schritt
   *[other] { $count } weitere Schritte
}
common-record-recognized = Konzept
common-record-offer = Angebot
common-recognized-no-concept-set = die Konzeptmenge wurde nicht geladen, daher wurde nichts erkannt
common-recognized-no-patterns = die Muster der Konzeptmenge ließen sich nicht bauen, daher wurde nichts erkannt
common-recognized-matcher-failed = der Matcher konnte nicht laufen, daher wurde nichts erkannt

common-lens-explore = Erkunden
common-lens-learn = Lernen
common-lens-train = Üben
common-lens-read = Lesen
common-concept-statement = Aussage
common-concept-intuition = Anschauung
common-concept-examples = Beispiele
common-concept-misconception = Fehlvorstellung
common-concept-lens = Sicht
common-concept-shown-by = hat etwas in
common-concept-shown-by-none = noch nichts
common-concept-prerequisites = kommt nach
common-concept-sources = Quellen
common-concept-exercises = Aufgaben
common-concept-activities = Aktivitäten
common-concept-activity = { $kind } mit { $shapes }, { $variation }
common-activity-variation-identical = jede wie sie ist
common-activity-variation-orientation = gedreht
common-activity-variation-size = in verschiedenen Größen
common-activity-shape-matching = Formen zuordnen
common-activity-shape-circle = Kreis
common-activity-shape-square = Quadrat
common-activity-shape-triangle = Dreieck
common-reference-operator-smallest = kleinster Eintrag
common-reference-operator-smallest-meaning = der kleinste Eintrag einer Liste oder Matrix
common-reference-operator-largest = größter Eintrag
common-reference-operator-largest-meaning = der größte Eintrag einer Liste oder Matrix
common-reference-operator-sorted = sortiert
common-reference-operator-sorted-meaning = dieselbe Liste mit ihren Einträgen in aufsteigender Reihenfolge
common-note-temperature-difference = das ist eine Temperaturdifferenz in { $unit } und keine Ablesung auf dieser Skala; für eine Ablesung schreib { $reading }
common-note-temperature-difference-inside = dieser Wert ist eine Temperaturdifferenz in { $unit } und keine Ablesung auf dieser Skala; eine Ablesung schreibt man mit { $operator }
common-note-temperature-difference-entry = der Eintrag { $written } an Position { $position } ist eine Temperaturdifferenz in { $unit } und keine Ablesung auf dieser Skala, und neben einer Ablesung zählt er als diese Differenz über dem absoluten Nullpunkt; für eine Ablesung schreib { $reading }
common-note-temperature-difference-entries = die Einträge in { $unit } sind Temperaturdifferenzen und keine Ablesungen auf dieser Skala; eine Ablesung schreibt man mit { $operator }
common-note-antiderivative = eine Stammfunktion; jede andere unterscheidet sich von ihr um eine Konstante
common-note-taylor-polynomial = das Taylorpolynom, das nahe der Stelle mit der Funktion übereinstimmt und nicht die Funktion selbst ist
common-note-radix-decimal = dezimal { $value }
common-note-chemistry-charge = Ladung { $charge }
common-note-chemistry-each-side = jede Seite enthält { $counts }
common-note-chemistry-charge-each-side = die Ladung auf jeder Seite ist { $charge }
common-note-chemistry-tables = Standardatomgewichte CIAAW 2024; molare Massenkonstante CODATA 2022, als ihr Wert ± 2 Standardunsicherheiten genommen
common-note-chemistry-natural-interval = ein Bereich natürlicher Schwankung, den CIAAW angibt: { $elements }
common-note-chemistry-expanded-uncertainty = die erweiterte Unsicherheit von CIAAW, eine Grenze und keine Standardunsicherheit: { $elements }
common-note-nuclear-measured-mass = AME2020 enthält eine gemessene Atommasse dafür
common-note-nuclear-estimated-mass = AME2020 enthält dafür nur eine geschätzte Masse, aus dem Verlauf der Massenfläche
common-note-nuclear-no-mass = AME2020 enthält keine Masse dafür
common-note-nuclear-each-side = jede Seite hat die Massenzahl { $mass }, die Ladung { $charge } und die Elektron-Leptonenzahl { $leptons }
common-note-nuclear-photons = die Zahl der Photonen legt keine Erhaltungsgröße fest, sie wird so genommen, wie sie geschrieben ist
common-note-nuclear-q-tables = Atommassen AME2020; u·c² und m_e·c² CODATA 2022; jeweils als Wert ± 2 Standardunsicherheiten genommen, und eine gedruckte Masse zusätzlich ± 3 Einheiten ihrer letzten gedruckten Stelle; die Enden werden addiert, also hält der Bereich, wie auch immer die Massen korreliert sind
common-note-nuclear-q-atomic = ein Q-Wert aus Atommassen: er unterscheidet sich vom Q-Wert nackter Kerne um die Änderung der Elektronenbindung
common-note-nuclear-q-ground-state = vom Grundzustand zum Grundzustand
common-note-nuclear-q-capture = ein Elektroneneinfang steht ohne die Bindungsenergie des eingefangenen Elektrons
common-note-nuclear-q-positron = die Energie steht vor der Zerstrahlung des Positrons
common-note-nuclear-q-decay-impossible = als Zerfall kann das beim neutralen Atom nicht geschehen: sein Q-Wert liegt unter 0
common-note-nuclear-q-sign-undecided = das Vorzeichen dieses Q-Werts ist bei dieser Überdeckung nicht entschieden: sein Bereich enthält 0
common-note-radix-written-decimal = dezimal geschrieben
common-note-worst-case-low = am kleinsten, wo { $corner }
common-note-worst-case-high = am größten, wo { $corner }
common-note-free-names = in den freien Namen { $names } geschrieben, denen keine Zeile einen Wert gibt
common-note-overflowed-to-infinity = alle Operanden waren endlich, das Ergebnis ist es nicht: das Maschinenformat fasst eine so große Zahl nicht
common-note-not-a-number-from-finite-operands = alle Operanden waren endlich, das Ergebnis ist keine Zahl: die Maschinenarithmetik hat dafür keinen Wert
common-note-bound-not-smaller-than-value = der Rundungsfehler ist nicht kleiner als der Wert, die gezeigten Ziffern sagen also nichts über ihn; dieselbe Zeile ohne { $conversion } wird exakt ausgewertet
common-note-temperature-difference-mixed = dieser Wert ist eine Temperaturdifferenz und keine Ablesung auf einer Temperaturskala
common-note-temperature-reading = das ist eine Ablesung auf der { $scale }-Skala
common-method-exact-after-conversion = exakt, nachdem jedes Argument von to_f64 oder to_f32 auf eine Maschinenzahl gerundet wurde
common-note-machine-conversion = { $written } rundet { $argument } auf die Maschinenzahl { $machine }; der Unterschied ist { $difference }
common-reading-number = Dezimalzahl, auf { $digits } gültige Stellen gerundet
common-reading-computed = aus dem exakten Wert oben, kaufmännisch gerundet
common-reading-below = { $distance } unter dem exakten Wert
common-reading-above = { $distance } über dem exakten Wert
common-reading-at-most = höchstens { $distance }
common-note-zero-to-the-zero = 0^0 wird als 1 genommen, das leere Produkt, wie es der binomische Lehrsatz und Potenzreihen brauchen; dass x^y keinen Grenzwert hat, wenn x und y beide gegen 0 gehen, ist eine andere Frage
common-record-valid = gültig
common-where-not-zero = { $count ->
    [one] überall, wo { $denominators } nicht null ist
   *[other] überall, wo { $denominators } nicht null sind
}
common-valid-where = { $count ->
    [one] { $denominators }; wo es null ist, hat die Zeile keinen Wert
   *[other] { $denominators }; wo einer davon null ist, hat die Zeile keinen Wert
}

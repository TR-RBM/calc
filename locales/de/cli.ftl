cli-help-usage =
    Aufruf:
      calc <Ausdruck> | <Datei.calc> [<Zeile>] [--digits <n> | --enclose <n>] [--json]
      calc - --begin | --enter <Text> | --machine-line <Zeile> <f64 oder f32> | --solve <JSON> | --choose-wanted <Zeile> <Rolle>
      calc solve --json [--request <JSON>]
      calc plot <Ausdruck> | <Datei.calc> <Zeile> --output <Pfad> [Bildoptionen]
      calc read <Datei.calc> <Zeile> --at <Koordinaten> [Bildoptionen] [--commit]
      calc concept <Bezeichner oder Name> [--lens <Bereich>] [--json]
      calc asm <Datei.s> | - [--given <Register>=<n>]... [--function <Name>]
      calc help language [--json]
      calc --help
cli-help-page =
    calc — ein Rechner, der ein Ergebnis so lange exakt hält, wie die
    Mathematik es zulässt

    Aufruf:
      calc <Ausdruck> [--digits <n> | --enclose <n> | --working [<Pfad>]]
           [--json] [--locale <Tag>]
           [--units <System>] [--unit <Art>=<Einheit>]... | [--coherent-units]
      calc <Datei.calc> [<Zeile>] [--digits <n> | --enclose <n> | --inspect]
           [--json]
      calc - --begin
      calc - --enter <Text>
      calc - --machine-line <Zeile> <f64 oder f32>
      calc - --solve <JSON> | --choose-wanted <Zeile> <Rolle>
      calc solve --json [--request <JSON>]
      calc plot <Ausdruck> | <Datei.calc> <Zeile> --output <Pfad>
           [--view <untere>..<obere> [Einheit]]... [--param <Name>=<Wert>]...
           [--size <Breite>x<Höhe>] [--limit <Regel>] [--settle]
      calc read <Datei.calc> <Zeile> --at <Koordinaten>
           [--layer <Index>] [--commit] [Bildoptionen]
      calc concept <Bezeichner oder Name> [--lens <Bereich>] [--json]
      calc asm <Datei.s> | - [--given <Register>=<n>]... [--function <Name>]
      calc help language [--json]
      calc --complete -- <Wort>...

    Beschreibung:
      Ohne Unterbefehl wertet calc einen Ausdruck aus oder öffnet eine
      Sitzungsdatei und gibt ihre Zeilen aus. Ein Ergebnis bleibt exakt, wo die
      Mathematik es zulässt; wo nicht, nennt calc das verwendete
      Maschinenformat und die Größe des Rundungsfehlers.

      solve          für ein Programm und nicht für einen Menschen: es liest
                     eine Anfrage in JSON und antwortet in JSON, welche Wege zu
                     einer gesuchten Größe führen, was noch fehlt und was aus
                     den vorhandenen Größen schon folgt.
      plot           zeichnet das Bild eines Ausdrucks oder einer Zeile in eine
                     PNG-Datei.
      read           liest dieses Bild an getippten Koordinaten ab und kann die
                     Ablesung als neue Zeile der Sitzung behalten.
      concept        gibt ein Konzept aus, benannt entweder mit seinem
                     Bezeichner oder mit dem Namen, den eine Zeile zeigt.
      asm            zählt Befehle und Speicherzugriffe von x86-64-Assembler,
                     aus NASM, aus der Intel-Syntax von GCC oder aus
                     objdump -d -M intel, exakt, als Formel in den Registern,
                     die eine Funktion bekommt, und sagt, warum der Code die
                     Takte nicht bestimmt.
      help language  gibt jedes Konstrukt der Eingabesprache mit einem Beispiel
                     aus.

    Optionen:
      --digits <n>      n Nachkommastellen zeigen, mit dem Rest, der sie exakt
                        macht
      --enclose <n>     ein bewiesenes Intervall mit n signifikanten Stellen
                        zeigen
      --json            in der JSON-Form der Sitzungsdatei ausgeben, für ein
                        Programm
      --locale <Tag>    die Sprache als BCP-47-Tag wählen
      --units <System>  Werte in den Einheiten dieses Systems zeigen: si oder
                        us-customary
      --unit <Art>=<Einheit>
                        eine Größenart in dieser Einheit zeigen, einmal je Art
      --coherent-units  jeden Wert in seiner kohärenten Einheit zeigen
      --begin           eine leere Sitzung auf die Standardausgabe schreiben,
                        zum Weiterleiten an die Befehle darunter
      --enter <Text>    eine Zeile mit diesem Text anfügen und auswerten
      --machine-line <Zeile> <f64 oder f32>
                        eine Zeile anfügen, die <Zeile> in Maschinenarithmetik
                        auswertet
      --solve <JSON>    eine Zeile anfügen, die nach den Wegen zu einer Größe
                        fragt
      --choose-wanted <Zeile> <Rolle>
                        aus einer erreichbaren Zeile eine Suche nach Wegen
                        dorthin machen
      --request <JSON>  die Anfrage als Argument statt über die Eingabe geben
      --output <Pfad>   das Bild in diese PNG-Datei schreiben
      --view <untere>..<obere> [Einheit]
                        die nächste Achse des Bildes in exakten Zahlen setzen
      --param <Name>=<Wert>
                        einer freien Variablen einen exakten Wert geben
      --size <Breite>x<Höhe>
                        die Bildgröße in Pixeln, 800x600 ohne Angabe
      --limit fixed:<n> | following:<Basis>,<je Halbierung>,<Obergrenze>
                        die Iterationsgrenze eines Escape-Time-Bildes
      --settle          die gezeigte Ansicht und die Parameter in der
                        Sitzungsdatei behalten
      --at <Koordinaten>
                        die Koordinaten der Ablesung, eine je Achse, mit Komma
                        getrennt
      --layer <Index>   die abzulesende Schicht, ohne Angabe die erste
      --commit          die Ablesung als neue Zeile der Sitzungsdatei behalten
      --how-it-ran      sagen, wie das Backend jede Operation dieser Zeile
                        gerechnet hat
      --trace           jeden Schritt eines benannten Sortierens wie
                        insertion_sort zeigen
      --batch           eine Eingabe je Zeile lesen und der Reihe nach
                        beantworten
      --terse           jede Zeile in einer Zeile beantworten: Wert, Einheit
                        und ob er exakt ist
      --find <Name>=<von>..<bis>
                        jede ganze Zahl des Bereichs für diesen Namen einsetzen
                        und jede nennen, für die die Behauptung gilt
      --counter         die Zahlen des Bereichs nennen, für die die Behauptung
                        nicht gilt, statt der Zahlen, für die sie gilt
      --check           jede Zeile als Behauptung wie x = y lesen und sagen,
                        ob sie gilt, exakt
      --identity        jede Zeile als Behauptung über Namen lesen und sagen,
                        ob sie für jeden ihrer Werte gilt, mit Algebra, oder
                        überall dort, wo die Nenner, mit denen sie geschrieben
                        ist, nicht null sind
      --expand          Produkte und Potenzen von Summen ausmultiplizieren und
                        die Terme zusammenfassen, exakt
      --factor          den exakten rationalen Wert jeder Zeile als Produkt von
                        Primzahlpotenzen schreiben und sagen, welche Primzahlen
                        bewiesen sind
      --solve-for       jede Zeile als Gleichung oder Ungleichung in einem
                        Namen oder als Liste von Gleichungen lesen und jede
                        Lösung angeben, exakt
      --ode <system>    die Lösung von diff(x, t) = …; … mit bewiesener
                        Schranke einschließen, mit --initial und --at
      --initial <werte>
                        der Anfang von --ode, etwa "t = 0 s; x = 1 m";
                        Einheiten werden mitgeführt
      --at <zeiten>     die Zeiten, zu denen --ode die Lösung einschließt,
                        etwa "t = 1 s, 10 s"
      --recognize       im Stapel die Konzepte jeder Zeile benennen, was weit
                        mehr kostet als das Rechnen
      --replay          jede Zeile einer Sitzungsdatei neu rechnen und sagen,
                        ob sie noch das Gespeicherte ergibt
      --lens <Bereich>  die Sicht, in der ein Konzept öffnet: explore, learn,
                        train oder read, ohne Angabe explore
      --given <Register>=<n>
                        so zählen, als stünde diese ganze Zahl beim Eintritt im
                        Register, einmal je Register
      --function <Name> nur diese Funktion des Assemblers zählen
      --working [<Pfad>]
                        zeigen, wie das Ergebnis zustande kam, Schritt für Schritt
      --version         die Version zeigen, den Commit, aus dem gebaut wurde,
                        das Ziel und das Sitzungsdateiformat
      --help            diese Hilfe zeigen
      --complete -- <Wort>...
                        auflisten, was auf diese Wörter folgen kann, eines je
                        Zeile mit einer Beschreibung nach einem Tab, für die
                        Vervollständigung einer Shell

    Rückgabewerte:
      0  die Arbeit ist getan: ein Wert, ein Bild, eine Ablesung, eine
         Antwort mit Wegen, Front, gelöst oder erreichbar, oder eine
         Aussage, die gilt
      1  die Arbeit ist gescheitert, die Fehlerausgabe sagt warum; unter
         --check und --identity eine Aussage, die calc widerlegt hat, oder
         eine, die etwas ohne Wert benennt
      2  die Befehlszeile wurde nicht verstanden: eine unbekannte Option, ein
         fehlendes Argument, ein Argument der falschen Art
      3  das Gelesene ließ sich nicht verwenden: eine ungültige Anfrage oder
         eine Zeile, die keine Aussage ist, über die calc urteilen kann
      4  es wurde verwendet und hat nicht erreicht, was es wollte: eine
         Anfrage, die sagt was fehlt, eine unentschiedene Aussage, ein
         --find ohne Treffer oder ein --ode, das das Verfahren nicht
         einschließen konnte
      5  nur bei --complete: an dieser Stelle passen auch Dateinamen

    Beispiele:
      calc "1/3 + 1/6"
          1/2, exakt: ein Bruch wird als Bruch beantwortet
      calc "0.1 + 0.2"
          0.3, exakt – die nächste Zeile zeigt, was eine Maschine daraus macht
      calc "to_f64(0.1 + 0.2)"
          0.30000000000000004, maschinell, mit seinem Rundungsfehler
      calc "2/3" --digits 5
          0.66666 und der Rest 1/150000, der es exakt macht
      calc "sqrt(2)" --enclose 12
          1.41421356237 und 1.41421356238, dazwischen liegt der exakte Wert
      calc "200 + 15%"
          keine der Lesarten: es nennt die beiden möglichen und bittet um eine
      calc "3 kg * 9.81 m/s^2"
          29.43 N, exakt: die Einheit wird gerechnet, nicht mitgeführt
      calc "(9.81 +- 0.02) m/s^2 * 2 s"
          19.620 m/s ± 0.040 m/s, die Unsicherheit fortgepflanzt
      calc "100 km / 1 h" --units us-customary
          (781250/12573) mi/h: ein System ändert die Einheit, in der ein
          Wert gezeigt wird, nie den Wert
      calc "integral(x^2, x, 0, 1)"
          1/3, exakt
      calc --identity "(x+1)^2 = x^2 + 2*x + 1"
          gilt für jeden Wert, und das ist ein Beweis und keine Stichprobe
      calc --identity "sqrt(x^2) = x"
          keine Identität: sie gilt nicht bei x = -1
      calc --check "2 + 2 = 4"
          gilt, exakt entschieden, und unentschieden, wo calc nicht entscheiden
          kann
      calc --find "n=1..200" --counter "n^2 > n"
          n = 1, die einzige Zahl im Bereich, bei der sie nicht gilt
      calc --expand "(x+1)*(x-1)"
          x^2 - 1, und --solve-for "x^2 = 4" antwortet -2, 2
      calc --ode "diff(y, t) = -y" --initial "t = 0; y = 1" --at "t = 1"
          y bei t = 1 zwischen zwei Dezimalzahlen um exp(-1), bewiesen
      calc --batch --json < lines.txt
          ein JSON-Objekt je Zeile, in der Reihenfolge der gelesenen Zeilen
      calc "100 km/h * 5 s" --json
          die Zeile für ein Programm, in kohärenten Einheiten
      calc plot "sin(x)" --output wave.png
          die Kurve als PNG-Datei, abgetastet in der Breite des Bildes
      calc read session.calc r1 --at 0.5 --commit
          der Wert dieser Zeile bei x = 0.5, als neue Zeile behalten
cli-error = Fehler: { $detail }
cli-terse-other = { $state }
cli-terse-exact = { $value } exakt
cli-terse-range = { $value } Bereich, beide Enden exakt
cli-terse-machine = { $value } maschinell
cli-record-digit-counts = { $counts }, vollständig mit --json
cli-terse-elided = { $value } ({ $counts })
cli-terse-measured = { $value } ± { $uncertainty } { $kind }
cli-terse-failed = Fehler { $detail }
cli-language-not-computed = {"    "}die Eingabesprache nimmt das an, calc rechnet es noch nicht
cli-found = { $assignment }
cli-found-none = nichts im Bereich lässt es gelten
cli-found-no-counter = nichts im Bereich lässt es scheitern
cli-found-none-decided = kein Wert im Bereich, den calc entscheiden konnte, lässt es gelten
cli-found-no-counter-decided = kein Wert im Bereich, den calc entscheiden konnte, lässt es scheitern
cli-found-undecided = { $count ->
    [one] calc konnte die Aussage bei { $assignment } nicht entscheiden
   *[other] calc konnte die Aussage bei { $count } Werten nicht entscheiden, zuerst bei { $assignment }
}
cli-found-too-many = { $count } Kandidaten liegen über der Grenze von { $limit }; enge die Bereiche ein
cli-claim-definition = definiert  { $claim }
cli-claim-holds = gilt  { $claim }
cli-claim-fails = GILT NICHT  { $claim }
cli-claim-undecided = unentschieden  { $claim }
cli-claim-holds-throughout = gilt für jeden Wert ihrer Bereiche  { $claim }
cli-claim-fails-throughout = GILT für keinen Wert ihrer Bereiche  { $claim }
cli-claim-holds-in-part = GILT NICHT: nur für einen Teil ihrer Bereiche  { $claim }
cli-claim-not-a-relation = keine Relation  { $claim }
cli-claim-unknown-name = unbekannter Name  { $claim }: { $detail }
cli-claim-refused = abgelehnt  { $detail }
cli-error-no-input = kein Ausdruck und keine Sitzungsdatei angegeben
cli-error-unknown-option = unbekannte Option { $option }
cli-error-missing-option-value = Option { $option } braucht einen Wert
cli-error-unexpected-argument = unerwartetes Argument { $argument }
cli-error-invalid-locale = { $value } ist kein BCP-47-Sprachtag
cli-error-invalid-count = Option { $option } braucht eine ganze Zahl, nicht { $value }
cli-error-view-without-line = eine Ansicht einer Sitzungsdatei braucht die Zeile, die sie zeigt
cli-error-option-given-twice = Option { $option } ist mehr als einmal angegeben
cli-error-descending-range = der Bereich { $value } läuft abwärts, und ein Bereich wird aufwärts durchsucht; schreib { $ascending }
cli-error-coherent-units-with-chosen-units = --coherent-units zeigt jeden Wert in seiner kohärenten Einheit und kann nicht zusammen mit --units oder --unit angegeben werden
cli-error-two-forms-of-one-line = eine Zeile wird in einer Form gezeigt: --inspect, --digits, --enclose oder --working
cli-error-instrument-not-applicable = { $option } gilt nicht für { $line }, die { $options } hat
cli-instrument-none = keine andere Form
cli-error-unknown-unit-system = { $value } ist kein Einheitensystem; calc hat { $systems }
cli-error-unit-needs-kind-and-unit = Option --unit braucht <Größenart>=<Einheit>, nicht { $value }
cli-error-unknown-quantity-kind = { $value } ist keine Größenart, die --unit setzen kann; setzen kann es { $kinds }
cli-error-kind-given-twice = Option --unit gibt die Einheit von { $kind } mehr als einmal an
cli-error-view-not-finished = die Ansicht endete ohne Antwort
cli-error-output-failed = die Ausgabe konnte nicht geschrieben werden

cli-line-input = { $line }  { $input }
cli-record-row = {"  "}{ $label }  { $value }
cli-line-failed = {"  "}{ $detail }
cli-line-not-evaluated = {"  "}gespeichert, bevor es berechnet war
cli-quantity = { $value } { $unit }
cli-uncertainty = ± { $standard }, Standardunsicherheit, Erweiterungsfaktor k = { $coverage_factor }
cli-view-uncertainty = ± { $standard }
cli-view-uncertainty-cut = ± { $standard }, an dieser Stelle abgeschnitten
cli-status = { $replay }  { $results }  { $running }  { $locale }  { $precision }  { $backend }
cli-status-replayed = { $replay }  { $results }  { $running }  { $differing }  { $locale }  { $precision }  { $backend }
cli-line-solve = {"  "}{ $state }
cli-line-picture = {"  "}ein Bild, von plot gezeichnet
cli-line-defined = {"  "}definiert eine Funktion für andere Zeilen
cli-line-orbit-escaped = {"  "}entkommen nach { $count } von höchstens { $limit } Iterationen, eine Anzahl ohne Rundungsgarantie
cli-line-orbit-count-missing = {"  "}entkommen innerhalb von { $limit } Iterationen, die Anzahl wurde nicht aufbewahrt
cli-line-orbit-inside = {"  "}bewiesen in der Menge, mit einer Grenze von { $limit } Iterationen
cli-line-orbit-undecided = {"  "}unentschieden nach { $limit } Iterationen
cli-plot-written = { $path }, { $width } mal { $height } Pixel
cli-plot-notice = {"  "}{ $notice }
cli-error-invalid-view = Option --view braucht <untere>..<obere> in exakten Zahlen, nicht { $value }
cli-error-invalid-parameter = Option --param braucht <Name>=<exakter Wert>, nicht { $value }
cli-error-invalid-size = Option --size braucht <Breite>x<Höhe> in Pixeln, nicht { $value }
cli-error-plot-needs-output = calc plot braucht --output <Pfad>
cli-error-plot-needs-line = calc plot einer Sitzungsdatei braucht die zu zeichnende Zeile
cli-error-file-not-written = { $path } ließ sich nicht schreiben
cli-error-output-permission-denied = { $path } kann nicht geschrieben werden: keine Berechtigung
cli-error-output-directory-missing = { $path } kann nicht geschrieben werden: dieses Verzeichnis gibt es nicht
cli-error-output-is-a-directory = { $path } kann nicht geschrieben werden: es ist ein Verzeichnis
cli-error-output-not-writable = { $path } kann nicht geschrieben werden
cli-error-invalid-coordinate = { $value } ist keine exakte Koordinate
cli-error-read-needs-coordinates = calc read braucht --at mit einer Koordinate je Achse
cli-session-written = Sitzung nach { $path } geschrieben
cli-error-settle-needs-session = --settle braucht eine Sitzungsdatei, in die das festgelegte Bild geschrieben wird
cli-error-invalid-limit = Option --limit braucht fixed:<Iterationen> oder following:<Basis>,<je Halbierung>,<Obergrenze> in ganzen Zahlen, nicht { $value }
cli-error-limit-given-twice = Option --limit ist mehr als einmal angegeben
cli-reading = { $value } ≈ { $reading }
cli-language-entry = {"  "}{ $name } — { $meaning }
cli-language-entry-without-words = {"  "}{ $name }
cli-language-spelling = {"    "}{ $mode }  { $pattern }  wie in { $example }
cli-language-precedence-left = {"    "}Stufe { $level }, gruppiert von links
cli-language-precedence-right = {"    "}Stufe { $level }, gruppiert von rechts
cli-language-precedence-none = {"    "}Stufe { $level }, gruppiert nicht
cli-language-arguments = {"    "}{ $least } bis { $largest } Argumente
cli-language-keyword = {"    "}Schlüsselwortargument { $name }, Wert der Art { $value }
cli-language-units = Einheiten
cli-language-unit = {"  "}{ $symbol }, gezeigt als { $display_symbol }
cli-language-prefixes = Präfixe
cli-language-prefix = {"  "}{ $symbol }, zehn hoch { $exponent }
cli-error-unknown-help-topic = unbekanntes Hilfethema { $topic }
cli-language-precedence-table = Bindungsstärke, von der stärksten an
cli-language-level-left = {"  "}Stufe { $level }  { $name }, gruppiert von links
cli-language-level-right = {"  "}Stufe { $level }  { $name }, gruppiert von rechts
cli-language-level-none = {"  "}Stufe { $level }  { $name }, gruppiert nicht
cli-language-unit-with-prefixes = {"  "}{ $symbol }, gezeigt als { $display_symbol }, nimmt Dezimalpräfixe
cli-modes-row = {"  "}{ $label }{"  "}{ $operations }
cli-modes-what-native-means = Ein nativer Modus sagt, dass die Prüfungen auf diesem Adapter bei diesem Start bestanden haben. Er behauptet nicht, dass die Operation für jeden Operanden richtig ist.
cli-modes-one-way = jede Operation auf dieselbe Weise
cli-modes-no-backend = exakt ausgewertet, daher hat kein Backend eine Operation gerechnet
cli-ran-on-width = { $backend } ({ $width })
cli-modes-evaluation = { $width }: { $modes }
cli-modes-block-label = { $width } { $label }
cli-modes-sentence-row = {"  "}{ $text }
cli-recognized-running = wird noch erkannt
cli-recognized-nothing = keines aus der Konzeptmenge passt
cli-recognized-more = mehr
cli-recognized-list = { $head }, und { $last }
cli-recognized-unnamed = { $count ->
    [one] einen Begriff, den diese Version nicht kennt
   *[other] { $count } Begriffe, die diese Version nicht kennt
}
cli-recognized-cut-short = kein Konzept passte, bevor die Suche vorzeitig endete
cli-offer-cut-short = die Suche endete, bevor jeder Begriff geprüft war
cli-offer-whole = jeder Begriff wurde geprüft
cli-offer-not-said = diese Datei sagt nicht, ob jeder Begriff geprüft wurde
cli-asm-heading = { $name }
cli-asm-row = {"  "}{ $label }{"  "}{ $value }
cli-asm-label-always = immer
cli-asm-label-count = Zählung
cli-asm-label-cycles = Takte
cli-asm-label-outside = nicht gezählt
cli-asm-label-assumes = nimmt an
cli-asm-counts = { $instructions } { $instruction_count ->
    [one] Befehl
   *[other] Befehle
}, { $reads } { $read_count ->
    [one] Lesezugriff
   *[other] Lesezugriffe
} und { $writes } { $write_count ->
    [one] Schreibzugriff
   *[other] Schreibzugriffe
} auf den Speicher
cli-asm-counts-without-memory = { $instructions } { $instruction_count ->
    [one] Befehl
   *[other] Befehle
}; die Speicherzugriffe sind nicht gezählt, weil ein Befehl, für den calc keine Zeile hat, auf den Speicher zugreift
cli-asm-between = zwischen { $low } und { $high }
cli-asm-input-argument = der Wert in { $name } beim Eintritt, das { $position ->
    [1] erste
    [2] zweite
    [3] dritte
    [4] vierte
    [5] fünfte
   *[other] sechste
} ganzzahlige Argument der System-V-Aufrufkonvention
cli-asm-input-register = der Wert in { $name } beim Eintritt
cli-asm-outside = Aufrufe von { $names }, die nicht in der Eingabe stehen
cli-asm-unresolved-call = eine Adresse, die der Linker noch nicht eingetragen hat (objdump -dr nennt sie)
cli-asm-indirect-call = einer erst zur Laufzeit berechneten Adresse
cli-asm-assumes-no-overflow = keine Rechnung mit einem Schleifenzähler oder seiner Grenze läuft über ihr Register hinaus
cli-asm-assumes-separate-stack = Schreibzugriffe über eine berechnete Adresse ändern keine eigenen Stack-Variablen der Funktion
cli-asm-cycles-x86-64 = nicht durch den Code bestimmt: ein x86-64-Kern führt Befehle außer der Reihe und mehrere zugleich aus, darum hängen seine Takte von Caches, Sprungvorhersage und Taktfrequenz ab
cli-asm-place-line = Zeile { $line }
cli-asm-indirect-jump = der Sprung in { $place } geht an eine erst zur Laufzeit berechnete Adresse, der calc nicht folgen kann
cli-asm-jump-outside = der Sprung in { $place } verlässt die Funktion nach { $name }, das nicht in der Eingabe steht
cli-asm-recursion = der Aufruf in { $place } ruft { $name } auf, während es noch läuft; calc zählt keine Rekursion
cli-asm-repeat-prefix = der Befehl in { $place } wiederholt sich so oft, wie rcx sagt; ein rep-Präfix zählt calc noch nicht
cli-asm-unsupported-jump = { $mnemonic } in { $place } ist ein Sprung, dem calc noch nicht folgt
cli-asm-runs-past-end = die Funktion läuft über ihren letzten Befehl hinaus
cli-asm-too-many-paths = die Funktion hat mehr als { $limit } Wege durch sich
cli-asm-calls-too-deep = der Aufruf in { $place } von { $name } liegt tiefer als { $limit } Aufrufe; calc folgt Aufrufen dort nicht weiter
cli-asm-trap = bei diesen Eingaben hält die Funktion bei { $mnemonic } in { $place } an; das beendet das Programm, statt zurückzukehren, darum gibt es keine Zählung eines beendeten Aufrufs
cli-asm-leaves-without-returning = bei diesen Eingaben verlässt die Funktion bei { $name } in { $place } ihren Aufrufer, ohne zurückzukehren, durch eine Ausnahme, ein Thread-Ende oder einen Weitsprung; darum gibt es keine Zählung eines beendeten Aufrufs
cli-asm-data-range = die Zahl schwankt mit den Daten auf eine Weise, die calc nicht eingrenzen kann
cli-asm-in-callee = in { $name }, aufgerufen in { $place }: { $reason }
cli-asm-loop-second-exit = die Schleife, die in { $place } endet, hat einen zweiten Ausgang, den calc noch nicht zählt
cli-asm-loop-unconditional = die Schleife, die in { $place } endet, hat an ihrem Ende keine Bedingung
cli-asm-loop-no-counter = calc findet keinen Zähler, den die Schleife, die in { $place } endet, weiterzählt und prüft
cli-asm-loop-step = die Schleife, die in { $place } endet, zählt in Schritten, die den Abstand zu ihrer Grenze nicht teilen
cli-asm-loop-unsigned = die Schleife, die in { $place } endet, vergleicht ihren Zähler ohne Vorzeichen, was calc noch nicht zählt
cli-asm-loop-data = wie oft die Schleife, die in { $place } endet, läuft, hängt von Daten im Speicher ab oder von einem Wert, dem calc nicht folgen kann
cli-asm-loop-outer-counter = wie oft die Schleife, die in { $place } endet, läuft, hängt vom Zähler einer äußeren Schleife ab; diese Summe zählt calc noch nicht
cli-asm-loop-never-ends = die Schleife, die in { $place } endet, endet hier nicht durch ihre Bedingung
cli-asm-loop-wraps = der Zähler der Schleife, die in { $place } endet, läuft hier über, bevor die Schleife endet
cli-asm-loop-stack = die Schleife, die in { $place } endet, verschiebt den Stack-Zeiger, was calc noch nicht zählt
cli-asm-loop-overlapping = die Schleifen in { $place } überlappen, ohne dass eine in der anderen liegt
cli-error-asm-needs-input = calc asm braucht eine Assembler-Datei oder -, um sie aus der Eingabe zu lesen
cli-error-asm-given = --given braucht ein Register und eine ganze Zahl, etwa edx=14, nicht { $value }
cli-error-asm-no-functions = { $path } enthält keine Befehle, die calc lesen kann
cli-error-asm-no-function = { $name } ist keine Funktion in { $path }
cli-concept-heading = { $identifier }  { $name }
cli-concept-row = {"  "}{ $label }{"  "}{ $value }
cli-concept-lens-empty = {"  "}{ $lens } hat für dieses Konzept noch nichts zu zeigen
cli-error-concept-needs-identifier = calc concept braucht einen Konzeptbezeichner oder -namen
cli-error-unknown-lens = { $value } ist nicht explore, learn, train oder read
cli-error-invalid-path = Option --working braucht einen Pfad aus Schrittnummern mit Punkten getrennt, nicht { $value }
cli-working-ask-for = abrufbar mit
cli-working-path = --working { $path }

cli-identity-holds-everywhere = gilt für jeden Wert
cli-identity-fails-everywhere = gilt für keinen Wert
cli-identity-not-everywhere = keine Identität: die beiden Seiten sind als Polynome in ihren Namen nicht gleich
cli-identity-holds-only-instance = gilt, und darin kann nichts variieren
cli-identity-fails-only-instance = gilt nicht, und darin kann nichts variieren
cli-identity-holds-with = gilt, mit { $substitution }
cli-identity-fails-with = gilt nicht, mit { $substitution }
cli-identity-holds-where-defined = { $count ->
    [one] gilt überall dort, wo { $denominators } nicht null ist
   *[other] gilt überall dort, wo { $denominators } nicht null sind
}
cli-identity-exclusion-list = { $head } und { $last }
cli-identity-fails-where-defined = { $count ->
    [one] gilt nirgends, wo { $denominators } nicht null ist
   *[other] gilt nirgends, wo { $denominators } nicht null sind
}
cli-identity-undecided = unentschieden: calc konnte die Differenz allein mit Algebra nicht auf null bringen
cli-identity-refuted = keine Identität: sie gilt nicht bei { $witness }
cli-identity-undecided-searched = unentschieden: calc konnte die Differenz allein mit Algebra nicht auf null bringen, und keine ganze Zahl von { $from } bis { $to } widerlegt sie, was ein Hinweis und kein Beweis ist
cli-identity-undecided-searched-fractions = unentschieden: calc konnte die Differenz allein mit Algebra nicht auf null bringen, und weder eine ganze Zahl von { $from } bis { $to } noch ein Bruch mit einem Nenner von { $smallest } bis { $largest } zwischen -{ $bound } und { $bound } widerlegt sie, was ein Hinweis und kein Beweis ist
cli-identity-undecided-stopped = unentschieden: calc konnte die Differenz allein mit Algebra nicht auf null bringen, und die Suche nach einem Gegenbeispiel hielt bei { $witness } an, wo die Aussage nicht entschieden werden konnte
cli-identity-undecided-not-searched = unentschieden: calc konnte die Differenz allein mit Algebra nicht auf null bringen, und die Suche nach einem Gegenbeispiel lief nicht, weil die Namen mehr Belegungen ergeben als die Grenze erlaubt
cli-expanded = { $expression }
cli-not-algebraic = dieser Ausdruck ist kein Polynom in seinen Namen und hat daher keine ausmultiplizierte Form

cli-solved = { $solutions }
cli-solved-nothing = keine Zahl löst diese Gleichung
cli-solved-real-only = { $solutions }; die übrigen Lösungen sind keine reellen Zahlen, und --solve-for listet sie noch nicht auf
cli-solved-nothing-real = keine reelle Zahl löst diese Gleichung; ihre Lösungen sind keine reellen Zahlen, und --solve-for listet sie noch nicht auf
cli-solved-root-of-note = rootof(p, x, k) ist die k-te reelle Nullstelle von p, von der kleinsten an gezählt; --enclose gibt ihre Stellen
cli-solved-inequality = { $intervals }
cli-solved-inequality-every = jede Zahl löst diese Ungleichung
cli-solved-inequality-none = keine Zahl löst diese Ungleichung
cli-inequality-or = oder
cli-factored = { $factors }
cli-factor-probable = { $factor } ist wahrscheinlich prim: calc beweist eine Primzahl nur unterhalb von 3317044064679887385961981
cli-factor-not-split = { $factor } ist nicht prim, und calc hat seine Faktoren nicht gefunden
cli-factor-not-rational = calc zerlegt eine exakte rationale Zahl oder ein Polynom in einem Namen mit rationalen Koeffizienten, und diese Zeile ist keins von beiden
cli-solved-system = { $values }
cli-solved-system-free = { $values }, für jeden Wert von { $free }
cli-solved-system-nothing = keine Zahlen lösen alle diese Gleichungen zugleich
cli-solve-system-not-linear = calc löst eine Liste von Gleichungen nur, wo jede linear in ihren Namen ist
cli-solve-system-not-equations = calc löst eine Liste nur, wo jeder Eintrag eine Gleichung ist
cli-solve-no-name = diese Gleichung enthält keinen Namen, nach dem sich auflösen ließe
cli-solve-several-names = diese Gleichung enthält mehr als einen Namen ({ $names }); calc löst nach einem auf und wählt nicht aus
cli-solve-every-number = jede Zahl löst diese Gleichung
cli-solve-degree-too-high = der Grad dieser Gleichung ist { $degree } und liegt über dem, was calc löst
cli-solve-radical-coefficients = in einem Koeffizienten dieser Gleichung steht eine Quadratwurzel, und calc löst eine solche Gleichung beim Grad 1 oder beim Grad 2 mit rationaler Diskriminante; diese hat den Grad { $degree }
cli-solve-not-a-polynomial = calc löst eine Gleichung nur, wo sie ein Polynom in einem Namen ist
cli-solve-constant-coefficients = calc löst eine Gleichung mit Konstanten in ihren Koeffizienten nur, wo diese aus rationalen Zahlen, Quadratwurzeln aus rationalen Zahlen, pi und e durch Summen, Produkte und Quotienten gebildet sind, und { $names } ist so nicht gebildet; mit einem solchen Koeffizienten löst es noch nicht
cli-solve-constant-coefficient-degree = mit pi oder e in ihren Koeffizienten löst calc eine Gleichung vom Grad 1, und diese hat den Grad { $degree }; solche Gleichungen löst es noch nicht
cli-solve-unproven-coefficient = ein Koeffizient dieser Gleichung enthält pi und e zusammen, und seine Einschließung beweist nicht, dass er ungleich null ist, was calc vor dem Lösen braucht; eine solche Gleichung löst es noch nicht
cli-solve-square-part-unknown = calc konnte { $number } nicht in Primzahlen zerlegen, die es beweisen kann; deshalb findet es die Quadratwurzel nicht, die diese Gleichung braucht, und antwortet nicht, statt eine Lösung zu verlieren

cli-version =
    calc { $version } ({ $commit })
    Ziel { $target }
    Sitzungsdateiformat { $format }
cli-claim-unreadable = unlesbar  { $claim }: { $detail }

cli-complete-command-solve = eine JSON-Anfrage nach den Wegen zu einer Größe beantworten
cli-complete-command-plot = das Bild eines Ausdrucks in eine PNG-Datei zeichnen
cli-complete-command-read = ein Bild an Koordinaten ablesen
cli-complete-command-concept = ein Konzept zeigen, nach Kennung oder Name
cli-complete-command-asm = die Befehle von x86-64-Assembler zählen
cli-complete-command-help = die Eingabesprache zeigen
cli-complete-topic-language = jede Form, die die Eingabesprache annimmt
cli-complete-json = in JSON ausgeben, für ein Programm
cli-complete-locale = die Sprache wählen
cli-complete-help = die Hilfe zeigen
cli-complete-version = die Version zeigen
cli-complete-digits = n Nachkommastellen zeigen, mit dem Rest
cli-complete-enclose = ein bewiesenes Intervall mit n signifikanten Stellen zeigen
cli-complete-working = zeigen, wie das Ergebnis zustande kam
cli-complete-inspect = den Eintrag einer Zeile zeigen
cli-complete-units = Werte in den Einheiten eines Systems zeigen
cli-complete-unit = eine Größenart in dieser Einheit zeigen
cli-complete-coherent-units = jeden Wert in seiner kohärenten Einheit zeigen
cli-complete-how-it-ran = sagen, wie das Backend jede Operation gerechnet hat
cli-complete-trace = jeden Schritt eines benannten Sortierens zeigen
cli-trace-heading = {"  "}Schritte
cli-trace-not-a-sort = --trace zeigt die Schritte eines benannten Sortierens wie insertion_sort, und diese Zeile ist keines
cli-trace-step = {"  "}{ $number }  { $step }
cli-trace-at = { $value } an Stelle { $position }
cli-trace-held = { $value } (beiseitegelegt von Stelle { $position })
cli-trace-keyed-at = { $value } (Schlüssel { $key }) an Stelle { $position }
cli-trace-keyed-held = { $value } (Schlüssel { $key }, beiseitegelegt von Stelle { $position })
cli-trace-in-scratch = { $value } im Zwischenspeicher an Platz { $position }
cli-trace-keyed-in-scratch = { $value } (Schlüssel { $key }) im Zwischenspeicher an Platz { $position }
cli-trace-compare = vergleiche { $left } mit { $right }: { $verdict }
cli-trace-smaller = { $value } ist kleiner
cli-trace-larger = { $value } ist größer
cli-trace-equal = die Schlüssel sind gleich
cli-trace-undecided = calc konnte es nicht entscheiden
cli-trace-move = verschiebe { $value } von Stelle { $from } an Stelle { $to }
cli-trace-put = setze { $value }, beiseitegelegt von Stelle { $from }, an Stelle { $to }
cli-trace-to-scratch = kopiere { $value } von Stelle { $from } in den Zwischenspeicher an Platz { $to }
cli-trace-from-scratch = kopiere { $value } vom Zwischenspeicher, Platz { $from }, zurück an Stelle { $to }
cli-trace-exchange = vertausche { $left } mit { $right }
cli-trace-tally = zähle { $value } in Zähler { $counter }
cli-trace-prefix-sum = addiere Zähler { $from } zu Zähler { $to }
cli-trace-decrement = zieh 1 von Zähler { $counter } ab
cli-trace-draw = zieh ein Pivot für die Positionen { $from } bis { $to }: Position { $chosen }, nach { $draws ->
    [one] 1 Block
   *[other] { $draws } Blöcken
}
cli-trace-flip = dreh die ersten { $length } Einträge um
cli-trace-pass = Einfügesortieren der Einträge im Abstand { $gap }
cli-trace-digit-pass = Countingsort nach Ziffer { $place }, von der niedrigsten an gezählt, von jedem Schlüssel minus dem kleinsten Schlüssel, { $least }
cli-trace-digit-tally = zähle { $value } in Zähler { $counter }, dem Zähler der Ziffer { $digit }
cli-trace-bead-falls = { $value } lässt eine Perle auf Stab { $pole } fallen
cli-trace-bead-read = die Perle von Stab { $pole } in Reihe { $row } von unten wird für diese Reihe gezählt
cli-trace-rebuild = schreib { $beads }, die Perlen von Reihe { $row } von unten, an Stelle { $position }
cli-trace-bitonic-merge = mische in Blöcken aus { $block }, abwechselnd steigend und fallend: vergleiche Einträge im Abstand { $distance }
cli-trace-shuffle = Mischvorgang { $number }, nach Fisher–Yates von der letzten Stelle an
cli-trace-shuffle-draw = zieh eine Stelle von 1 bis { $position } für Stelle { $position }: Stelle { $chosen }, nach { $draws ->
    [one] 1 Ziehung
   *[other] { $draws } Ziehungen
}
cli-complete-replay = jede Zeile einer Sitzungsdatei neu rechnen
cli-complete-batch = eine Eingabe je Zeile beantworten
cli-complete-recognize = die Konzepte jeder Zeile benennen
cli-complete-check = sagen, ob jede Behauptung gilt, exakt
cli-complete-terse = jede Zeile in einer Zeile beantworten
cli-complete-find = jede ganze Zahl eines Bereichs einsetzen
cli-complete-counter = die Zahlen nennen, für die die Behauptung nicht gilt
cli-complete-identity = sagen, ob eine Behauptung für jeden Wert gilt
cli-complete-expand = ausmultiplizieren und die Terme zusammenfassen
cli-complete-factor = jeden Wert als Produkt von Primzahlpotenzen schreiben
cli-complete-solve-for = jede Lösung einer Gleichung angeben
cli-complete-begin = eine leere Sitzung schreiben
cli-complete-enter = eine Zeile anfügen und auswerten
cli-complete-machine-line = eine Zeile in Maschinenarithmetik anfügen
cli-complete-solve = eine Zeile anfügen, die nach den Wegen zu einer Größe fragt
cli-complete-choose-wanted = die Wege zur Größe einer Zeile suchen
cli-complete-request = die Anfrage als Argument geben
cli-complete-output = das Bild in diese PNG-Datei schreiben
cli-complete-view = die nächste Achse in exakten Zahlen setzen
cli-complete-param = einer freien Variablen einen exakten Wert geben
cli-complete-size = die Bildgröße in Pixeln
cli-complete-limit = die Iterationsgrenze eines Escape-Time-Bildes
cli-complete-settle = die Ansicht und die Parameter in der Sitzungsdatei behalten
cli-complete-at = die Koordinaten der Ablesung
cli-complete-layer = die abzulesende Schicht
cli-complete-commit = die Ablesung als neue Zeile behalten
cli-complete-given = so zählen, als stünde diese ganze Zahl im Register
cli-complete-function = nur diese Funktion zählen
cli-complete-lens = die Sicht, in der ein Konzept öffnet
cli-complete-format-f64 = binäre Gleitkommazahl mit 64 Bit
cli-complete-format-f32 = binäre Gleitkommazahl mit 32 Bit
cli-note-unknown-locale = calc hat keine Sprache { $requested }, daher antwortet es auf { $answered }; es hat { $shipped }
cli-claim-note = Hinweis: { $note }
cli-batch-line-refused = Zeile { $number }, { $input }: { $detail }
cli-terse-exact-where = { $value } exakt, { $condition }
cli-expanded-where = { $expression }, { $condition }
cli-relation-is-a-claim = { $reading } ist eine Aussage, kein Wert; calc entscheidet sie mit --check: calc "{ $reading }" --check
cli-relation-about-free-names = { $reading } ist eine Aussage über { $names }, kein Wert; calc entscheidet sie für jeden Wert mit --identity, sucht ein Gegenbeispiel mit --find und löst sie mit --solve-for
cli-complete-ode = die Lösung eines Anfangswertproblems mit bewiesener Schranke einschließen
cli-complete-initial = der Anfang von --ode: die Zeit und der Wert jedes Namens dazu
cli-complete-ode-at = die Zeiten, zu denen --ode die Lösung einschließt
cli-error-ode-without = --ode, --initial und --at werden zusammen gebraucht, und { $option } fehlt
cli-ode-time = { $name } = { $value }
cli-ode-at = bei { $time }
cli-ode-between = zwischen { $lower } und { $upper }
cli-ode-label-interval-bound = Intervallschranke { $name }
cli-ode-interval-bound = { $width }, die Breite des Einschlusses bei { $time }, Rundung und jeder Schritt davor eingeschlossen
cli-ode-whole-run = über den ganzen Lauf
cli-ode-label-step-bound = Schrittschranke { $name }
cli-ode-step-bound = { $bound }, der größte Abbruchfehler, den ein Schritt hinzufügt; sie ist keine Schranke für die Lösung
cli-ode-label-method = Verfahren
cli-ode-method = { $steps ->
    [one] Taylor-Reihe in Intervallarithmetik nach Lohner, Ordnung { $order }, ein Schritt; jeder Einschluss oben ist bewiesen
   *[other] Taylor-Reihe in Intervallarithmetik nach Lohner, Ordnung { $order }, { $steps } Schritte; jeder Einschluss oben ist bewiesen
}
cli-ode-unreadable = { $part } ist nicht lesbar: { $detail }
cli-ode-not-an-assignment = { $part } ist keine Zuweisung wie t = 0 oder x = 1 m
cli-ode-not-a-derivative = { $part } ist keine Gleichung diff(x, t) = …; --ode liest jeden Teil zwischen Semikolons als Ableitung eines Namens nach der Zeit, gleichgesetzt mit ihrer rechten Seite
cli-ode-time-names-differ = die Gleichungen leiten nach { $first } und nach { $second } ab; --ode leitet jede Gleichung nach demselben Namen ab
cli-ode-component-twice = { $name } hat zwei Gleichungen; --ode nimmt eine für jeden Namen
cli-ode-no-initial-value = --initial gibt keinen Wert für { $name } an
cli-ode-not-in-the-system = { $name } ist weder die Zeit noch ein Name mit einer Gleichung, und --initial und --at geben nur diese an
cli-ode-no-times = --at gibt keine Zeit an, zu der die Lösung eingeschlossen wird
cli-ode-unknown-name = die rechte Seite von { $part } verwendet { $name }, und das ist weder die Zeit noch ein Name mit einer Gleichung
cli-ode-unit-mismatch = die rechte Seite der Gleichung für { $name } hat die Einheit { $found }, und { $name } je { $time } hat die Einheit { $wanted }, mit den Einheiten aus --initial
cli-ode-time-unit-mismatch = { $part } ist keine Zeit in der Einheit des Anfangs oder in einer, die sich in sie umrechnen lässt
cli-ode-unit-with-offset = { $name } ist in einer Einheit mit Nullpunktverschiebung angegeben, etwa °C; gib es in einer Einheit ohne an, etwa K
cli-ode-not-a-real-number = { $part } ergibt keine reelle Zahl, die calc einschließen kann
cli-ode-time-not-rational = { $part } ist keine exakte rationale Zeit; --ode schreitet zwischen exakten Zeiten fort, darum wird eine Zeit als ganze Zahl, Bruch oder Dezimalzahl geschrieben
cli-ode-unsupported = die rechte Seite verwendet { $part }, und dafür hat --ode noch keine Taylor-Reihe
cli-ode-division-by-zero = die rechte Seite teilt in { $part } durch null
cli-ode-stop-before-start = eine Zeit in --at liegt vor dem Anfang; --ode schließt die Lösung nur vorwärts in der Zeit ein
cli-ode-not-enclosed = calc hat die Lösung bis { $time } eingeschlossen und konnte keinen Schritt darüber hinaus einschließen, auch nach { $halvings } Halbierungen seiner Länge nicht; die Lösung wächst dort vielleicht über alle Grenzen oder verlässt den Definitionsbereich ihrer rechten Seite
cli-ode-too-many-steps = nach { $steps } Schritten, der letzte { $step } lang, ist die Lösung nur bis { $time } eingeschlossen, nicht bis { $stop }; sie ändert sich auf einer Zeitskala, die viel kürzer als das Intervall ist, darum braucht das Intervall mehr Schritte, als --ode bisher macht
cli-ode-inconsistent = calc hat ein Problem gebaut, das es nicht integrieren kann; das ist ein Fehler in calc
cli-ode-rounded-time = { $time } (auf { $digits } gültige Stellen abgerundet)
cli-ode-units-do-not-combine = die rechte Seite der Gleichung für { $name } verbindet Größen, deren Einheiten nicht zueinander passen, etwa eine Länge plus eine Zeit oder eine Zeit in exp

# MusicJam: documento di progetto

Data: 2026-11-07
Stato: approvato (2026-11-07). L'utente ha chiesto di implementare e ha delegato a Claude tutte le decisioni rimaste aperte.

## 1. Perché

L'utente ascolta musica con Qobuz Desktop e lo trova poco usabile, per due ragioni precise:

- **La ricerca è fatta male.** Cercando "the narcissist blur" il brano dei Blur non esce in cima. La ricerca non capisce che una parte del testo è l'artista e l'altra il titolo.
- **Far partire un brano è scomodo.** Si può avviare solo cliccando la piccola icona del play: niente doppio clic sulla riga, niente Invio.

MusicJam è un player desktop per macOS, scritto in Rust con GPUI, per la musica che l'utente ha sul disco. Alla riproduzione aggiunge un mixaggio leggero: mentre un brano suona sulle casse, il successivo si ascolta in cuffia, si sceglie da dove farlo partire e lo si lancia con un crossfade.

Va considerato un successo se l'utente lo usa al posto di Qobuz Desktop per la sua musica locale, perché trovare un brano e prepararne il passaggio è più rapido.

## 2. Obiettivi

1. Ricerca istantanea su una libreria locale che capisce le query miste artista + titolo.
2. Caricare un brano con doppio clic o Invio.
3. Due deck, "In onda" (casse) e "Prossimo" (cuffia), con pre-ascolto su un'uscita audio separata.
4. Scelta del punto di partenza del brano successivo sulla forma d'onda.
5. Crossfade manuale o automatico tra i due deck.
6. Impostazioni per cartella della musica, uscite audio con prova e durata del crossfade.

## 3. Non obiettivi (fuori da questa versione)

- Streaming Qobuz o di altri servizi.
- BPM, beatmatching, sincronizzazione, effetti, EQ.
- Controller MIDI e schede a 4 uscite. L'architettura non deve impedirli (vedi §7), ma non si implementano.
- Coda di più brani, playlist, cronologia.
- Modifica dei tag, organizzazione dei file, download di copertine o metadati da internet.
- Windows e Linux. Si punta solo a macOS, anche se GPUI permetterebbe altro.

## 4. Cose da evitare (anche se il software "funziona")

- **Leggere i brani offline cifrati di Qobuz** (cartella `~/Library/Application Support/Qobuz/…`, segmenti HLS cifrati) o aggirarne in qualsiasi modo la protezione. Il player legge solo file audio normali: MP3, FLAC, WAV.
- **Usare credenziali Qobuz non ufficiali**, per esempio estratte dal player web. Se in futuro si aggiunge Qobuz, sarà solo con credenziali API ufficiali.
- **Interrompere ciò che suona sulle casse senza che l'utente lo chieda.** Caricare un brano, usare l'anteprima, perdere la cuffia: niente di tutto questo deve toccare l'uscita Master.
- **Far finire l'anteprima sulle casse.**
- **Aprire la cuffia Bluetooth come ingresso.** Su macOS attiverebbe il profilo "chiamata" e l'audio peggiorerebbe di molto.
- **Rendere la ricerca o lo scroll lenti** con librerie grandi (decine di migliaia di brani).

## 5. Vincoli

- **Linguaggio e UI:** Rust con GPUI. È una scelta dell'utente.
- **Coerenza con gli altri progetti dell'utente** (`~/Dev/rust/filejam`, `~/Dev/rust/dbdelve`): usano edition 2024, `gpui = { package = "gpui-pre", version = "=0.3.9" }`, `gpui-component = "=0.8.0"` con versioni bloccate esatte e `Cargo.lock` nel repository, licenza GPL-3.0-only. Conviene allinearsi, così si condividono le conoscenze già fatte su un GPUI pre-1.0 che cambia spesso.
- **Piattaforma:** macOS, Apple Silicon.
- **Formati:** MP3, FLAC, WAV.
- **Hardware di riferimento:** uscita del Mac o DAC USB per le casse, cuffie Bluetooth (per esempio AirPods) per il cue.

## 6. Comportamento deciso con l'utente

### 6.1 Libreria
- Una cartella scelta nelle impostazioni, letta con tutte le sottocartelle.
- Artista, album, titolo e durata vengono dai tag. Quando mancano (succede spesso con i WAV), si ricavano da cartelle e nome del file, per esempio `Blur/The Ballad of Darren/03 - The Narcissist.wav`.
- Al primo avvio si costruisce un indice locale. Dopo si leggono solo i file nuovi, modificati o rimossi.

### 6.2 Schermata: tre colonne
1. **Sinistra: albero delle cartelle** della libreria. Cliccando una cartella, la lista mostra i suoi brani.
2. **Centro: ricerca e lista dei brani.** La ricerca cerca nella cartella selezionata, o in tutta la libreria se è selezionata la radice. In cima c'è il percorso della cartella corrente.
3. **Destra: i deck**, uno sopra l'altro: "In onda · casse" in alto, poi il pulsante Crossfade, poi "Prossimo · cuffia". In fondo, i nomi delle uscite in uso.

La bozza approvata (variante "C1") è in `doc/mockups/layout-v2.html`.

### 6.3 Ricerca
- "the narcissist blur" deve dare come primo risultato *The Narcissist* dei Blur. Le parole della query possono corrispondere a campi diversi (titolo, artista, album) e in qualsiasi ordine.
- I risultati si aggiornano mentre si scrive.
- Dalla tastiera: frecce per spostarsi nei risultati, Invio per caricare.

### 6.4 Caricamento
- Doppio clic o Invio su un brano lo carica nel deck **Prossimo**. Non interrompe mai "In onda".
- Se nulla è in onda, il brano caricato va comunque in "Prossimo" *(decisione del costruttore: vedi §8, punto 1)*.

### 6.5 Deck "Prossimo": pre-ascolto e punto di partenza
- Mostra titolo, artista e la forma d'onda del brano.
- Il punto di partenza si sceglie cliccando sulla forma d'onda (linea blu) e si regola a piccoli passi con le frecce. *Proposta da me, accettata dall'utente.*
- "Anteprima" suona il brano dal punto di partenza **solo sull'uscita Cue**. Ripremuta, si ferma.
- Esiste un solo "Prossimo", senza coda: un nuovo caricamento sostituisce il brano precedente.

### 6.6 Crossfade
- Il pulsante **Crossfade** fa partire "Prossimo" dal suo punto di partenza sull'uscita Master e porta "In onda" a zero mentre "Prossimo" sale, nella durata impostata (5 s di default, modificabile).
- **Automatico:** se l'utente non fa nulla e "Prossimo" è carico, il crossfade parte da solo in modo da finire quando termina "In onda".
- Alla fine del crossfade, "Prossimo" diventa "In onda", il deck "Prossimo" si svuota e l'anteprima in cuffia si ferma.
- Se "In onda" finisce e "Prossimo" è vuoto, **il player si ferma**. Non passa da solo al brano seguente della cartella.

### 6.7 Uscite audio
- Impostazioni: **Casse (Master)** e **Cuffia (Cue)**, ciascuna scelta tra i dispositivi di uscita che macOS vede, Bluetooth compresi. L'elenco si aggiorna quando colleghi o scolleghi un dispositivo.
- Ogni uscita ha un pulsante **Prova** che suona un breve segnale prima a sinistra e poi a destra, solo su quel dispositivo.
- Se Master e Cue sono lo stesso dispositivo, compare un'avvertenza.
- **Cuffia scollegata durante l'anteprima:** l'anteprima si ferma, compare un avviso e le casse continuano senza interruzioni.
- Le uscite sono due stream indipendenti, ciascuno con il proprio clock. Non serve compensare la deriva, perché suonano brani diversi. La latenza del Bluetooth (150-300 ms) è accettabile, dato che il cue serve ad ascoltare e scegliere il punto, non ad allineare i battiti.

### 6.8 Impostazioni
Cartella della musica, uscita Master, uscita Cue (con Prova), durata del crossfade. Restano salvate tra un avvio e l'altro.

## 7. Architettura suggerita (lasciata al costruttore)

Le linee qui sotto sono una proposta. Le scelte concrete spettano a chi costruisce.

- **Decodifica:** `symphonia`, in Rust puro, per MP3, FLAC e WAV.
- **Uscita audio:** `cpal` su CoreAudio, con uno stream per uscita. Il thread audio non deve allocare memoria né prendere lock: riceve i campioni da ring buffer senza lock (per esempio `rtrb`) riempiti da un thread di decodifica per deck.
- **Ricampionamento:** i file possono avere frequenze di campionamento diverse da quella del dispositivo (44,1, 48, 96 kHz…). Serve un resampler, per esempio `rubato`.
- **Tag:** `lofty`.
- **Indice e ricerca:** un indice in memoria con una ricerca a token su titolo, artista, album e percorso, che dia un punteggio alle corrispondenze. Va salvato su disco, per esempio in SQLite con `rusqlite`.
- **Forma d'onda:** calcolata in background al caricamento nel deck e salvata nella cache insieme all'indice.
- **Predisposizione per il futuro:** Master e Cue come "uscita logica = dispositivo + coppia di canali", così una scheda a 4 uscite (canali 1-2 e 3-4 sullo stesso dispositivo) si aggiunge senza cambiare il motore.

## 8. Decisioni prese da me (Claude) da verificare

1. Un brano caricato quando nulla è in onda va in "Prossimo" e si avvia con Crossfade o con un tasto "Play". Non parte da solo.
2. Il passo delle frecce per regolare il punto di partenza e le scorciatoie da tastiera sono lasciati al costruttore.
3. Formato delle impostazioni e posizione della cache: `~/Library/Application Support/MusicJam/`.
4. Nome del progetto e della cartella: `musicjam`, come la cartella creata dall'utente.

## 9. Lasciato al costruttore

Scelte delle librerie e struttura del codice, algoritmo di punteggio della ricerca (con il vincolo del caso "the narcissist blur"), aspetto grafico oltre la disposizione approvata, curva del crossfade, risoluzione della forma d'onda, strategia di test.

## 10. Decisioni prese dopo la revisione (delegate a Claude dall'utente)

1. **Uscita Master: un dispositivo preciso, non "predefinito di sistema".** Il menu elenca solo i dispositivi reali; al primo avvio si preseleziona quello che è predefinito in quel momento. Il dispositivo viene ricordato per nome, così se macOS sposta l'uscita predefinita sugli AirPods le casse non cambiano.
   - Se all'avvio il dispositivo Master salvato non c'è, la riproduzione è disabilitata e un avviso nel pannello dei deck rimanda alle impostazioni.
   - Se si scollega mentre suona, la riproduzione si ferma e compare un avviso. Niente ripiego automatico: finirebbe facilmente sulla cuffia.
   - Per l'uscita Cue vale lo stesso, con il comportamento già deciso al §6.7.
2. **Comandi del deck "In onda":** Play/Pausa e Stop, salto a un punto cliccando sulla forma d'onda. Niente controllo del volume: si usa quello del sistema o del DAC. Un crossfade già partito non si annulla.
3. **Crossfade automatico:** parte comunque, anche durante l'anteprima, che si ferma (§6.6). Con "In onda" in pausa il conto alla rovescia è fermo. Non si può disattivare.
4. **Cartella selezionata:** la lista mostra tutti i brani della cartella e delle sue sottocartelle, ordinati per percorso. La ricerca ha lo stesso ambito.
5. **Aggiornamento della libreria:** all'avvio e con un pulsante "Aggiorna libreria". Nessun controllo continuo dei file. Se la cartella non è raggiungibile (per esempio un disco esterno scollegato), l'indice non viene cancellato e compare un avviso.
6. **Esecuzione:** con `cargo run`. Pacchetto `.app` firmato fuori da questa versione.
7. **§8 approvato.** Un brano caricato quando nulla è in onda va in "Prossimo"; il pulsante Play del deck "Prossimo" (oppure Crossfade) lo manda in onda senza dissolvenza.

## 11. Modifiche chieste dopo la prima versione

- Le colonne si ridimensionano trascinando i bordi.
- Volume separato per casse e cuffia, salvato nelle impostazioni. Questo sostituisce la decisione del §10.2 ("niente volume").
- Temi grafici come in filejam, scelti nelle Impostazioni o con ⌘T. L'aspetto è da console DJ: deck A e deck B con i loro colori, scritte e tempi in carattere monospaziato, angoli squadrati e un indicatore del crossfader.

# MusicJam: scaletta, fine dei silenzi, volume uniforme

Data: 2026-11-14
Stato: approvato. L'utente ha scelto l'approccio A per coda e scaletta e ha delegato a Claude il resto ("decidi tu e implementa").
Documento precedente: `2026-11-07-musicjam-design.md`. Questo lo estende e cambia alcune sue decisioni (vedi §9).

## 1. Perché

Il primo MusicJam serve a passare da un brano al successivo, uno alla volta. Ogni brano va cercato e caricato a mano, mentre l'altro suona.
L'utente vuole poter **preparare in anticipo la scaletta di una serata**, salvarla e poi farla suonare in ordine, senza perdere il controllo del deck B: pre-ascolto in cuffia e punto di partenza.

Vengono anche fuori due problemi di ascolto, che Mixxx risolve e MusicJam no:

- **Code silenziose.** Il crossfade automatico parte a `durata − 5 s`. Se un file finisce con 8 secondi di silenzio, la dissolvenza avviene nel vuoto e in sala c'è un buco.
- **Salti di volume.** Un master degli anni '80 accanto a uno moderno suona molto più piano, quindi a ogni passaggio bisogna toccare il volume.

È un successo se l'utente prepara una scaletta a casa, la riapre la sera, la fa suonare e la ritocca al volo, senza buchi né salti di volume.

## 2. Obiettivi

1. **Fine dei silenzi:** il punto di partenza di default è il primo suono, e il crossfade automatico finisce sull'ultimo suono, non alla fine del file.
2. **Volume uniforme:** ogni brano suona allo stesso volume percepito, e il controllo si può disattivare.
3. **Scaletta:** una sequenza ordinata di brani, con inizio e fine scelti per ogni voce, salvata con un nome, riaperta e modificata.
4. **La scaletta aperta è la coda:** quando B va in onda, il brano seguente della scaletta si carica da solo in B.
5. **Le modifiche valgono solo per stasera finché non salvi:** Salva, Salva come e Ripristina.

## 3. Non obiettivi

- BPM, tonalità, sync, beatgrid, effetti, EQ, controller MIDI: restano fuori, come nel primo documento.
- Copertine (scartate dall'utente).
- Import ed export M3U, scalette "intelligenti" o casuali, cronologia.
- Uscite su coppie di canali della stessa scheda (vedi §8): non si implementano ora.

## 4. Cose da evitare

- **Toccare le casse senza un comando.** Aprire, salvare o modificare la scaletta non deve mai interrompere né cambiare ciò che suona in A. L'unica eccezione è già nel primo documento: il crossfade automatico.
- **Perdere la scaletta di stasera.** Una scaletta modificata e non salvata non deve sparire per una chiusura dell'app o un crash.
- **Salvare senza che l'utente lo chieda.** La scaletta salvata cambia solo con Salva o Salva come.
- **Alzare il volume fino a distorcere.** La normalizzazione non deve mai portare un brano oltre il suo picco.
- **Rallentare il caricamento in B** in modo percepibile, a causa dell'analisi.

## 5. Approcci considerati (coda e scaletta)

- **A. La scaletta aperta è la coda** *(scelto dall'utente).* C'è una sola lista. La si apre, la si modifica in una copia di lavoro con il segno "modificata" e la si salva quando si vuole. Preparazione a casa e serata usano la stessa schermata.
- **B. Coda e playlist separate.** "Suona" copia la playlist in una coda, e c'è un comando per riportare indietro le modifiche. Scartato: due liste quasi uguali e il dubbio continuo su quale delle due si sta modificando.
- **Salvataggio automatico con annulla.** Scartato perché va contro "vale solo per stasera se non decido di salvare".

Per la parte audio c'era una sola strada sensata (vedi §6.1).

## 6. Comportamento

### 6.1 Analisi del brano

Quando un brano viene caricato in un deck o aggiunto alla scaletta, MusicJam lo decodifica una volta in background e ne ricava:

- la **forma d'onda** (come oggi);
- l'**inizio del suono** e la **fine del suono**, cioè il primo e l'ultimo istante sopra **−60 dBFS** (la soglia di Mixxx);
- il **volume percepito** secondo EBU R128 (integrated loudness, in LUFS) e il **picco**;
- la **durata vera**, contata sui campioni decodificati.

Il risultato si salva nell'indice (`library.sqlite`) ed è valido finché il file non cambia (stessi `mtime` e `size`). Un brano già analizzato si carica subito. Per un brano nuovo, l'analisi di un FLAC di 5 minuti richiede una frazione di secondo in release. Durante l'analisi il deck mostra "ANALISI…" e il brano entra nel deck appena è pronto.
*Decisione mia:* niente analisi di tutta la libreria all'avvio, perché con decine di migliaia di brani sarebbero minuti di CPU. Si analizza solo quello che si usa.

### 6.2 Fine dei silenzi

- Un brano caricato senza un inizio scelto parte dall'**inizio del suono**, non da 0.
- Ogni brano in un deck ha una **fine**: di default la fine del suono, oppure quella scelta nella scaletta o sul deck.
- Il crossfade automatico parte in modo da **finire sulla fine**: parte a `fine − durata del crossfade`.
- Se A arriva alla fine e B è vuoto, il player si ferma lì, senza suonare la coda muta.
- Le forme d'onda mostrano la fine come una linea, e la parte dopo la fine è grigia.

### 6.3 Volume uniforme

- Guadagno del brano = `−14 LUFS − loudness`, limitato in modo che `picco × guadagno ≤ 1` e comunque entro ±12 dB.
  *Decisione mia:* −14 LUFS è il riferimento dei servizi di streaming. Con un valore più basso, come i −18 di ReplayGain, la musica di oggi verrebbe abbassata di molto e servirebbe alzare il volume del DAC.
- Il guadagno si applica per voce, cioè a ciascun deck, prima del crossfade, del volume del deck e del segnale di prova. Vale sia per le casse sia per la cuffia, così il pre-ascolto si sente come in onda.
- **MIX ▸ VOLUME UNIFORME:** acceso di default e salvato nelle impostazioni. Spento, ogni brano suona com'è.
- *Decisione mia:* i tag ReplayGain dei file si ignorano e si usa sempre la misura propria. È coerente su FLAC, MP3 e WAV (che quasi mai hanno i tag) e non dipende da come sono stati calcolati.

### 6.4 Inizio e fine sul deck

- **Clic** sulla forma d'onda di B: inizio (come oggi). **⇧ clic**: fine.
- **⇧ clic** sulla forma d'onda di A: sposta la fine del brano in onda, e il crossfade automatico si ricalcola. Il clic semplice su A continua a saltare.
- ⌥← ⌥→ (con ⇧ per i passi fini) spostano l'inizio di B, come oggi. ⌥⌘← ⌥⌘→ spostano la fine di B di 1 s.
- Se il brano di un deck viene dalla scaletta, inizio e fine scelti sul deck finiscono nella sua voce di scaletta, e la scaletta risulta modificata.

### 6.5 Scaletta

**Dove si vede.** La colonna centrale si divide in due, e il bordo tra le due parti si trascina. In alto ci sono ricerca e brani (come oggi), in basso la **SCALETTA**.
*Decisione mia:* la colonna centrale è la più larga ed è l'unica dove titolo, artista, inizio, fine e durata ci stanno su una riga. Mentre la si prepara, si vedono la ricerca e la scaletta insieme.

```
┌ LIBRERIA ┬─────────────── ricerca / brani ───────────────┬ DECK A ┐
│ cartelle │ Cerca…                                         │        │
│          │ TITOLO        ARTISTA · ALBUM          TEMPO   │        │
│          │ …                                              │  XFADE │
│          ├────────────────────────────────────────────────┤        │
│          │ SCALETTA · Sabato al Mare ●   12 · 58:40        │ DECK B │
│          │ [APRI ▾] [NUOVA] [SALVA] [SALVA COME…] [RIPR.] │        │
│          │  # ST TITOLO        ARTISTA   INIZIO  FINE DUR. │        │
│          │  1  ✓ Song 2        Blur        0:00  1:59 1:59│        │
│          │  2  A Girls & Boys  Blur        0:03  4:10 4:07│        │
│          │  3  B Parklife      Blur        0:12▸ 3:02 2:50│        │
│          │  4    Tender        Blur        0:00  7:31 7:31│        │
└──────────┴────────────────────────────────────────────────┴────────┘
```

- Colonna di stato: **A** in onda, **B** caricato in B, **✓** già suonato, **!** file mancante. INIZIO e FINE in grassetto se scelti a mano, in grigio se presi dall'analisi.
- Intestazione: nome (oppure "Senza nome"), **●** se modificata, numero di brani e durata totale (somma di `fine − inizio`).

**Comandi.**

| Comando | Dove | Effetto |
|---|---|---|
| Aggiungi alla scaletta | menu del brano, ⌘⇧Invio | in fondo alla scaletta |
| Suona dopo quello in onda | menu del brano | subito dopo la voce in A, e va in B |
| Doppio clic / Invio su un brano della libreria | lista | come oggi va in B, e **si inserisce nella scaletta** al posto del prossimo: quello che c'era va una posizione più giù |
| Doppio clic su una voce della scaletta | scaletta | quella voce va in B e diventa la prossima. Le voci saltate restano dove sono |
| Carica nel Deck A | menu della voce | come per la libreria, solo se A è fermo |
| Sposta su / giù | menu, ⌥⌘↑ ⌥⌘↓ | riordina |
| Rimuovi | menu, ⌫ | toglie la voce. Se era in B, B si svuota e si carica la voce seguente |
| Azzera inizio e fine | menu | torna ai valori dell'analisi |

- *Decisione mia:* il trascinamento per riordinare è lasciato al costruttore, se GPUI lo permette senza complicazioni. Tasti e menu bastano.
- Lo stesso brano può comparire più volte in una scaletta.

**Avanzamento.**
- La scaletta tiene due segni: la voce in **A** e la voce in **B**.
- Quando B va in onda (crossfade, manuale o automatico, oppure Play con A vuoto), quella voce diventa A e la voce **seguente** della scaletta si carica in B, con il suo inizio e la sua fine. Le voci con il file mancante si saltano.
- Se dopo A non c'è altro, B resta vuoto e il player si ferma alla fine di A, come oggi.
- Un brano caricato in A dal menu "Carica nel Deck A" della libreria non fa parte della scaletta: il segno A si toglie, e la scaletta riparte dalla voce in B.
- Aprire o cambiare la scaletta **non tocca A**. Se B contiene una voce della scaletta precedente, B si svuota e si carica la prima voce non suonata della nuova scaletta. Se B contiene un brano caricato a mano, resta dov'è.

**Salvataggio.**
- **Salva** (⌘S) scrive la copia di lavoro sulla scaletta aperta. Su una scaletta senza nome fa come Salva come.
- **Salva come…** (⌘⇧S) chiede un nome e crea una nuova scaletta, che diventa quella aperta.
- **Ripristina** torna all'ultima versione salvata. I segni A e B restano se la voce esiste ancora.
- **Apri ▾**: elenco delle scalette salvate, con Rinomina ed Elimina (con conferma). **Nuova** (⌘N): scaletta vuota senza nome.
- Se la copia di lavoro è modificata, Apri e Nuova chiedono: **Salva / Non salvare / Annulla**.
- **Stasera non si perde.** La copia di lavoro (voci, segni, nome, "modificata") si scrive su disco a ogni modifica, separata dalle scalette salvate, e alla riapertura di MusicJam torna com'era. A questo punto i segni A e B indicano solo dove eravamo arrivati: niente parte da solo.
- Una voce si riferisce al brano tramite il **percorso del file**, non tramite l'id interno, così sopravvive anche se l'indice viene ricostruito. Se il file non c'è più, la voce resta visibile come mancante (**!**).

### 6.6 Impostazioni

- MIX: durata del crossfade (come oggi) e **VOLUME UNIFORME** acceso/spento.
- TASTI: le nuove scorciatoie.

## 7. Architettura

- `audio::analysis`: un solo passaggio di decodifica che produce `Analysis { peaks, sound_start, sound_end, loudness_lufs, peak, duration }`. Sostituisce `waveform::peaks`. Il loudness si misura con il crate `ebur128`.
- Indice: tabella `analysis(path, mtime, size, …)`, e tabelle `playlists(id, name, updated)` e `playlist_entries(playlist_id, pos, path, start, end)`. La copia di lavoro sta nella tabella `session`.
- `Slot` riceve `end` e `gain`. `Transport` usa `end` al posto di `duration` per il crossfade automatico e per l'arresto. Nuovo comando `SetEnd(deck, secs)`. Il motore segnala quando B va in onda (`EngineEvent::NextOnAir`), e la UI fa avanzare la scaletta.
- `playlist`: modulo puro con la copia di lavoro (voci, segni A/B, modifiche) e tutte le operazioni della tabella al §6.5, testato senza audio né UI.

## 8. Mixxx: cosa ha e cosa ne prendiamo

Ricerca del 14/11/2026. Mixxx è gratuito e open source (GPL), gira su macOS, Windows e Linux. L'ultima serie stabile è la **2.5**: la 2.5.0 è uscita a dicembre 2024 (Qt6, editor dei tag su più brani, nuovi effetti) e il ramo si è chiuso con la 2.5.6. La 2.6 è annunciata ma non ancora uscita.

| Caratteristica di Mixxx | In MusicJam |
|---|---|
| **Auto DJ, 4 modalità di transizione:** *Full Intro + Outro* (crossfade lungo quanto il più corto tra outro e intro), *Fade at Outro Start*, *Full Track* (N secondi a fine brano), *Skip Silence* (come Full Track ma ignora il silenzio sotto −60 dBFS) | MusicJam aveva Full Track. Ora ha **Skip Silence**, e con inizio e fine scelti a mano si avvicina a Intro/Outro. |
| **Coda dell'Auto DJ:** playlist e crate in coda, opzione per rimettere in fondo i brani già suonati | **Scaletta aperta = coda** (§6.5) |
| **Aggiunta casuale** da crate scelte quando la coda si accorcia | No |
| **Playlist** ordinate, bloccabili, export M3U/PLS/CSV, copia dei file su chiavetta | Scaletta salvata. Export rimandato. |
| **Crate:** raccolte senza ordine, usate anche come sorgente dell'Auto DJ | No: le cartelle della libreria fanno da raccolta |
| **Cronologia** automatica per sessione | No (si può aggiungere in seguito: una tabella in più) |
| **Ricerca** con operatori (`artist:`, `bpm:>120`, `-escludi`, `|`, `~` fuzzy, campi vuoti) | No: la ricerca a parole libere resta quella attuale |
| **Analisi** di BPM e tonalità, beatgrid, valutazione a stelle, colori | No (non obiettivi) |
| **Normalizzazione** del volume (ReplayGain) | **Sì**, con una misura propria (§6.3) |
| **Preview deck:** ascolto in cuffia senza toccare l'uscita principale | Già presente: deck B con CUE |
| **Più schede audio**, ciascuna con il suo clock: la principale fa da riferimento, le altre si sincronizzano | Già presente: due stream indipendenti, senza sincronizzazione perché suonano brani diversi |
| **Import** da iTunes, Traktor, Rekordbox, Serato | No |
| Copertine dai tag o da `cover.jpg` | Scartate dall'utente |

Fonti: [Mixxx 2.5 Released](https://mixxx.org/news/2024-12-24-mixxx-2_5-released), [Mixxx 2.5.3](https://mixxx.org/news/2025-09-03-mixxx-2_5_3-released/), [manuale: Auto DJ](https://manual.mixxx.org/latest/en/chapters/djing_with_mixxx#auto-dj), [manuale: Libreria](https://manual.mixxx.org/latest/en/chapters/library), [manuale: Sound Hardware](https://manual.mixxx.org/latest/en/chapters/preferences/sound_hardware).

## 9. Scheda audio economica per il pre-ascolto

MusicJam apre la cuffia come un dispositivo di uscita a sé stante, scelto per nome. Qualsiasi uscita audio che macOS vede va bene, senza driver. Il Bluetooth funziona, ma aggiunge 150-300 ms di ritardo. Un'uscita via cavo li toglie e libera gli AirPods.

| Opzione | Prezzo indicativo | Come si usa con MusicJam | Note |
|---|---|---|---|
| **Presa cuffie del Mac** | 0 € | Cuffia = "Cuffie esterne" del Mac, casse sul DAC USB | Sicuro **solo se le casse sono su un altro dispositivo**. Con le casse sugli altoparlanti interni non è garantito: su molti Mac, inserire il jack spegne gli altoparlanti. Va provato con i due pulsanti **Prova** delle Impostazioni. |
| **Adattatore Apple USB-C → jack 3,5 mm** | ~10 € | Compare come dispositivo USB separato: si sceglie come Cuffia | Il più economico per avere due uscite indipendenti con le casse sul Mac. Su Head-Fi il suono è giudicato trasparente, mentre Ultrasone lo considera adatto all'uso quotidiano ma non all'alta fedeltà. Per il cue basta. |
| **Behringer UCA222** | ~20-30 € | Uscita RCA per l'amplificatore, presa cuffie | Class compliant, 16 bit / 48 kHz. È un solo dispositivo stereo: la presa cuffie ripete le uscite RCA, quindi come cue serve un **secondo** dispositivo. Ha senso come scheda per le casse, con la cuffia sulla presa del Mac. |
| **Behringer UMC204HD** | ~70-110 € | 4 uscite (A = 1-2, B = 3-4) e cuffia con selettore A/B | Una sola scheda e un solo clock per casse e cuffia: casse su 1-2, cuffia su 3-4. **Richiede che MusicJam sappia usare coppie di canali** dello stesso dispositivo. È previsto dall'architettura (primo documento, §7) ma non ancora implementato. |

**Consiglio:** l'adattatore Apple USB-C → 3,5 mm, oppure la presa del Mac se le casse sono già su un DAC USB, funziona subito con MusicJam così com'è e costa al massimo una decina di euro. La UMC204HD conviene solo se vuoi una scatola unica per casse e cuffia, e in quel caso va aggiunto il supporto per le coppie di canali.

Fonti: [Behringer UMC204HD su Thomann](https://www.thomannmusic.com/behringer_u_phoria_umc204hd.htm), [Equipboard UMC204HD](https://equipboard.com/items/behringer-u-phoria-umc204hd), [Equipboard UCA222](https://equipboard.com/items/behringer-u-control-uca222-usb-audio-interface), [Head-Fi: adattatore Apple](https://www.head-fi.org/showcase/comments/5106/), [Ultrasone: adattatori e DAC](https://www.ultrasone.com/apple-special-offers-deals/). I prezzi sono quelli trovati online a novembre 2026 e cambiano spesso.

## 10. Decisioni del primo documento che cambiano

- §3 "Coda di più brani, playlist, cronologia": coda e playlist (scaletta) entrano. La cronologia resta fuori.
- §6.5 "un solo Prossimo, senza coda": B resta uno solo, ma si riempie dalla scaletta.
- §6.6 "il crossfade automatico finisce quando termina In onda": ora finisce sulla **fine** del brano (§6.2).
- §6.6 "se Prossimo è vuoto il player si ferma": resta vero. Con una scaletta aperta, B è vuoto solo quando la scaletta è finita.

## 11. Lasciato al costruttore

Schema esatto delle tabelle, finestra di dialogo per il nome, trascinamento per riordinare, aspetto dei segni sulla forma d'onda e strategia di test, con i moduli puri `playlist`, `transport` e `analysis` coperti da test.

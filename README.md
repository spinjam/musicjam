# MusicJam

Player per macOS, per la musica che hai sul disco (MP3, FLAC, WAV). Ricerca istantanea, pre-ascolto in cuffia su un'uscita separata e crossfade verso le casse.

```sh
cargo run --release
```

Al primo avvio apri **Impostazioni** (⌘,) e scegli la cartella della musica, l'uscita **Casse** e l'uscita **Cuffia**. Il pulsante **Prova** accanto a ogni uscita suona un breve segnale, prima a sinistra e poi a destra, solo su quel dispositivo.

## Come si usa

- Scrivi nella ricerca, per esempio "narcissist blur": le parole possono corrispondere a titolo, artista o album, in qualsiasi ordine. La ricerca è limitata alla cartella selezionata nell'albero.
- Con doppio clic o Invio su un brano lo carichi nel deck **Prossimo**, senza interrompere quello **In onda**.
- **Anteprima** suona il Prossimo solo in cuffia. Cliccando sulla sua forma d'onda scegli da dove partirà.
- **Crossfade** porta il Prossimo sulle casse con una dissolvenza (5 s di default). Se non fai niente, la dissolvenza parte da sola e termina insieme al brano in onda. Se il Prossimo è vuoto, il player si ferma.

| Tasti | Azione |
|---|---|
| ⌘F | cerca |
| ↑ ↓ / Invio | scegli e carica nel Prossimo |
| ⌘P | play / pausa |
| ⌘. | stop |
| ⌘E | anteprima in cuffia |
| ⌘Invio | crossfade |
| ⌥← ⌥→ (⇧ per passi fini) | sposta il punto di partenza di 1 s (0,1 s) |
| ⌘R | aggiorna la libreria |

Impostazioni e indice stanno in `~/Library/Application Support/MusicJam/`.

## Sviluppo

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --example engine_smoke -- <file>   # suona un file sull'uscita predefinita
```

Il progetto e il piano di lavoro sono in `doc/`.

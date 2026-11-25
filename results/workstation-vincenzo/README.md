# Bench geometria su workstation Windows (Vincenzo), 18/11/2026

Seconda macchina oltre al portatile di Emiliano. Una run per ogni combinazione.

## Macchina

| voce | valore |
|---|---|
| modello | portatile Dell |
| CPU | Intel Core Ultra 9 275HX, 24 core, 24 thread (8 P-core e 16 E-core, niente hyper-threading) |
| `[Environment]::ProcessorCount` | 24, quindi thread misurati: 1, 8, 16, 24 |
| RAM | 64 GB (63,5 GB visibili) |
| Windows | Windows 11 Pro 10.0.26200 (build 26200) |
| rustc | 1.99.1 (58cf8f9c3 2026-10-13) |
| toolchain | `stable-x86_64-pc-windows-gnu` (vedi sotto) |
| alimentazione | collegato alla corrente, batteria al 96-97 %, piano energetico Bilanciato |
| programmi aperti | Chrome (8,3 GB), Rhino e altre applicazioni desktop, antivirus, 6 processi `bimcraft-cli` (server MCP). Non è stato possibile chiuderle. |
| carico CPU medio (5 s) | 24,8 % prima di geo_bench, 16,0 % prima di arris, 9,6 % alla fine |

### Toolchain

Con la toolchain di default (`stable-x86_64-pc-windows-msvc`) il build fallisce:

```
error: linker `link.exe` not found
note: the msvc targets depend on the msvc linker but `link.exe` was not found
```

Visual Studio 2022 Community 17.14 è installato, ma senza `vcvarsall.bat`, senza le librerie MSVC (`lib\x64` vuota) e senza Windows SDK. Non ho installato niente. Tutte le misure sono fatte con `cargo +stable-x86_64-pc-windows-gnu`, la toolchain già installata con cui è compilato anche il BIMCraft locale di questo PC. Il codice Rust passa per lo stesso LLVM. Cambiano il linker e il runtime C (MinGW al posto di MSVC).

## BIMCraft installato

| | versione |
|---|---|
| installata | 0.9.0 (`bimcraft-cli --version`) |
| ultima release | v0.12.0 |

L'installazione è per utente, con file del 27/10/2026. Non c'è una voce BIMCraft nelle chiavi `Uninstall` del registro, quindi la query delle istruzioni non restituisce niente. La versione installata non è l'ultima. Vincenzo ha il link all'installer: https://github.com/IPedrax/BIMCraft/releases/latest. I benchmark non dipendono dalla versione installata: compilano dal sorgente.

## Codice

| cartella | ramo | SHA |
|---|---|---|
| `bimcraft` | `origin/followups/family-editor` | `59b26325` (contiene `f3901009`) |
| `kernels` | `origin/bench/kernels` | `bc91122d` |

## geo_bench (BIMCraft, mesh)

`GEO_PANELS=10000`, `RAYON_NUM_THREADS=T`. Nelle colonne con due valori il primo è senza foro, il secondo con foro.

| thread | no void (s, 10.000) | with void (s, 10.000) | 1 core ms/pannello | 10 cambiati ms | triangoli/pannello |
|---|---|---|---|---|---|
| 1 | 1,56 | 4,65 | 0,2 / 0,4 | 1,7 / 3,0 | 344 / 476 |
| 8 | 0,19 | 0,43 | 0,1 / 0,2 | 1,0 / 2,2 | 344 / 476 |
| 16 | 0,18 | 0,33 | 0,1 / 0,3 | 1,2 / 2,2 | 344 / 476 |
| 24 | 0,17 | 0,29 | 0,1 / 0,3 | 1,2 / 2,6 | 344 / 476 |

Probe (ms per pannello, senza / con foro): 0,12 / 0,47 a 1 thread, 0,09 / 0,22 a 8, 0,10 / 0,26 a 16, 0,10 / 0,27 a 24.

Tempo totale di `cargo test` (con la partenza dei 30 binari di test vuoti): 102,4 s a 1 thread (prima run, include la scansione antivirus dei binari), 12,0 s a 8, 5,7 s a 16, 7,6 s a 24.

## arris-perf (kernel arris, `panels`)

`cargo run --release --bin panels -- <b|c> <n> <T>`. Tempo di costruzione = `build_wall_ms` stampato dal programma. Tempo totale = `cargo run` misurato da fuori, con il controllo di cargo (da 0,3 a 7 s) e la liberazione della memoria a fine run.

| variante | thread | n | costruzione (s) | totale (s) | errori |
|---|---|---|---|---|---|
| c (foro nel profilo) | 1 | 10.000 | 17,03 | 20,72 | 0 |
| c | 8 | 10.000 | 2,55 | 4,08 | 0 |
| c | 16 | 10.000 | 1,76 | 5,41 | 0 |
| c | 24 | 10.000 | 1,33 | 9,30 | 0 |
| b (foro con booleana) | 1 | 10.000 | fermata a 900 s, nessun output | oltre 900 | - |
| b | 1 | 1.000 | 122,15 | 123,79 | 0 |
| b | 8 | 10.000 | 357,35 | 376,78 | 0 |
| b | 16 | 10.000 | 287,95 | 324,69 | 0 |
| b | 24 | 10.000 | 186,54 | 201,83 | 0 |

La b a 1 thread con n=1000 è partita a mano subito dopo lo script (15:38-15:40), con le stesse app aperte. Lo script doveva rilanciarla da solo ma il controllo del timeout aveva un errore.

`panels` stampa triangoli e memoria solo con il quarto argomento `mesh`. Le istruzioni non lo prevedono, quindi qui non ci sono triangoli né memoria.

## Confronto con ADE (72 thread)

Numeri di ADE presi da `logs/arris` e `logs/bimcraft` del ramo `bench/kernels`.

| misura | ADE | questo PC |
|---|---|---|
| geo_bench 1 thread, no void / with void (s, 10.000) | 0,97 / 3,01 | 1,56 / 4,65 |
| geo_bench 8 thread, no void / with void (s, 10.000) | 0,13 / 0,40 | 0,19 / 0,43 |
| arris c, 10.000, 1 thread (s) | 26,14 | 17,03 |
| arris c, 10.000, 8 thread (s) | 4,21 | 2,55 |
| arris b, 1.000, 1 thread (s) | 44,22 | 122,15 |
| arris b, 10.000, 8 thread (s) | 49,40 | 357,35 |

La variante c qui è più veloce di ADE. La variante b è 2,8 volte più lenta a 1 thread (122 ms a pannello contro 44) e 7,2 volte più lenta a 8 thread. Scala poco con i thread: rispetto a 1 thread va 3,4 volte più veloce a 8 thread, 4,2 volte a 16 e 6,5 volte a 24. A 1 thread i 10.000 pannelli avrebbero richiesto circa 1.220 s, oltre il limite di 15 minuti. La causa non è verificata. Le differenze note rispetto ad ADE sono la toolchain gnu, Windows e le app aperte durante le misure.

## Output grezzo

### Log dello script (tempi totali misurati da fuori)

```
[15:04:18] start; ProcessorCount=24; threads=1,8,16,24
[15:04:27] cpu load before start (avg 5 s): 24.8 %
[15:06:09] geo_T1 exit=0 total_s=102.43
[15:06:22] geo_T8 exit=0 total_s=12.04
[15:06:27] geo_T16 exit=0 total_s=5.73
[15:06:35] geo_T24 exit=0 total_s=7.55
[15:06:41] cpu load before arris (avg 5 s): 16 %
[15:07:02] arris_c_T1 exit=0 total_s=20.72
[15:07:06] arris_c_T8 exit=0 total_s=4.08
[15:07:12] arris_c_T16 exit=0 total_s=5.41
[15:07:21] arris_c_T24 exit=0 total_s=9.30
[15:10:43] arris_b_T24 exit=0 total_s=201.83
[15:16:08] arris_b_T16 exit=0 total_s=324.69
[15:22:25] arris_b_T8 exit=0 total_s=376.78
[15:37:26] arris_b_T1 TIMEOUT after 900 s
[15:37:42] cpu load after (avg 5 s): 9.6 %
[15:37:42] done
[15:40:24] arris_b_T1_n1000 exit=0 total_s=123.79
```

### geo_T1

```
    Finished `release` profile [optimized] target(s) in 8.43s
     Running unittests src\lib.rs (target\release\deps\bimcraft_core-755c7464952780be.exe)
running 1 test
GEO-2f mesh baseline, 10000 unique panels, 1 threads; times for 10,000
GEO-2f no void    : all cores 1.56 s, one core 0.2 ms a panel, 10 changed 1.7 ms, 344 triangles a panel
GEO-2f with a void: all cores 4.65 s, one core 0.4 ms a panel, 10 changed 3.0 ms, 476 triangles a panel
GEO-2f probe: a panel 0.12 ms without its void, 0.47 ms with it
test geo_bench::geo_bench ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3060 filtered out; finished in 6.46s
[... omessi 30 binari di test di integrazione con 0 test, tutti ok ...]
```

### geo_T8

```
    Finished `release` profile [optimized] target(s) in 6.66s
     Running unittests src\lib.rs (target\release\deps\bimcraft_core-755c7464952780be.exe)
running 1 test
GEO-2f mesh baseline, 10000 unique panels, 8 threads; times for 10,000
GEO-2f no void    : all cores 0.19 s, one core 0.1 ms a panel, 10 changed 1.0 ms, 344 triangles a panel
GEO-2f with a void: all cores 0.43 s, one core 0.2 ms a panel, 10 changed 2.2 ms, 476 triangles a panel
GEO-2f probe: a panel 0.09 ms without its void, 0.22 ms with it
test geo_bench::geo_bench ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3060 filtered out; finished in 0.75s
[... omessi 30 binari di test di integrazione con 0 test, tutti ok ...]
```

### geo_T16

```
    Finished `release` profile [optimized] target(s) in 0.92s
     Running unittests src\lib.rs (target\release\deps\bimcraft_core-755c7464952780be.exe)
running 1 test
GEO-2f mesh baseline, 10000 unique panels, 16 threads; times for 10,000
GEO-2f no void    : all cores 0.18 s, one core 0.1 ms a panel, 10 changed 1.2 ms, 344 triangles a panel
GEO-2f with a void: all cores 0.33 s, one core 0.3 ms a panel, 10 changed 2.2 ms, 476 triangles a panel
GEO-2f probe: a panel 0.10 ms without its void, 0.26 ms with it
test geo_bench::geo_bench ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3060 filtered out; finished in 0.67s
[... omessi 30 binari di test di integrazione con 0 test, tutti ok ...]
```

### geo_T24

```
    Finished `release` profile [optimized] target(s) in 0.85s
     Running unittests src\lib.rs (target\release\deps\bimcraft_core-755c7464952780be.exe)
running 1 test
GEO-2f mesh baseline, 10000 unique panels, 24 threads; times for 10,000
GEO-2f no void    : all cores 0.17 s, one core 0.1 ms a panel, 10 changed 1.2 ms, 344 triangles a panel
GEO-2f with a void: all cores 0.29 s, one core 0.3 ms a panel, 10 changed 2.6 ms, 476 triangles a panel
GEO-2f probe: a panel 0.10 ms without its void, 0.27 ms with it
test geo_bench::geo_bench ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3060 filtered out; finished in 0.62s
[... omessi 30 binari di test di integrazione con 0 test, tutti ok ...]
```

### arris_c_T1

```
    Finished `release` profile [optimized] target(s) in 2.83s
     Running `target\release\panels.exe c 10000 1`
PANELS variant=c n=10000 threads=1 build_ok=10000 errors=0 build_wall_ms=17027
```

### arris_c_T8

```
    Finished `release` profile [optimized] target(s) in 0.35s
     Running `target\release\panels.exe c 10000 8`
PANELS variant=c n=10000 threads=8 build_ok=10000 errors=0 build_wall_ms=2551
```

### arris_c_T16

```
    Finished `release` profile [optimized] target(s) in 0.33s
     Running `target\release\panels.exe c 10000 16`
PANELS variant=c n=10000 threads=16 build_ok=10000 errors=0 build_wall_ms=1759
```

### arris_c_T24

```
    Finished `release` profile [optimized] target(s) in 3.92s
     Running `target\release\panels.exe c 10000 24`
PANELS variant=c n=10000 threads=24 build_ok=10000 errors=0 build_wall_ms=1333
```

### arris_b_T24

```
    Finished `release` profile [optimized] target(s) in 0.27s
     Running `target\release\panels.exe b 10000 24`
PANELS variant=b n=10000 threads=24 build_ok=10000 errors=0 build_wall_ms=186543
```

### arris_b_T16

```
    Finished `release` profile [optimized] target(s) in 6.98s
     Running `target\release\panels.exe b 10000 16`
PANELS variant=b n=10000 threads=16 build_ok=10000 errors=0 build_wall_ms=287952
```

### arris_b_T8

```
    Finished `release` profile [optimized] target(s) in 5.65s
     Running `target\release\panels.exe b 10000 8`
PANELS variant=b n=10000 threads=8 build_ok=10000 errors=0 build_wall_ms=357347
```

### arris_b_T1

```
    Finished `release` profile [optimized] target(s) in 6.55s
     Running `target\release\panels.exe b 10000 1`
[processo fermato dopo 900 s: nessuna riga PANELS]
```

### arris_b_T1_n1000

```
    Finished `release` profile [optimized] target(s) in 0.53s
     Running `target\release\panels.exe b 1000 1`
PANELS variant=b n=1000 threads=1 build_ok=1000 errors=0 build_wall_ms=122152
```

# «Min vei»-solver: makro-operator-ruting over algoritme-ID-er

Mortens idé (2026-07-31): gi algoritmene ID-er og la en MPEE-aktig «solver»
finne raskeste vei til løst kube som en sekvens av ID-er — der *raskest*
måles i brukerens egen eksekveringstid, ikke trekk-antall.

## Hvorfor dette slår ≤21-løsningen for mennesker

kewb-løsningen er trekk-optimal men vilkårlig: hvert trekk må leses.
En makro-rute er lengre i trekk men utføres på muskelminne. Med målte
tider per algoritme (MPEdb `training_results`) blir predikert
menneske-tid en ekte kostfunksjon — MPEE-mønsteret (målt kostmatrise →
ruteoptimalisering) anvendt på kubing.

## Grafen

- **Noder**: stadier — Scrambled → CrossSolved → F2L{delmengde av 4 slots}
  → OLL-done → Solved. (LBL-varianten: FirstLayer → SecondLayer → TopCross
  → CornersOriented → CornersPlaced → Solved.)
- **Kanter**: algoritme-ID-er som gjenkjenneren matcher i tilstanden
  (inkl. pre-AUF/y-ramme/1-trekks setup, som i hint-motoren i dag).
- **Kantkost** (fallende prioritet):
  1. Brukerens avg-of-5 for ID-en (MPEdb) når den finnes
  2. Ergonomikost-modellen (hints.rs) skalert til tid for utrente
  3. + overgangskost (regrip mellom påfølgende ID-er — finnes)

## Søket

Tilstandsrommet ETTER cross er lite: 4! = 24 slot-rekkefølger for F2L
(en bitteliten TSP — velg rekkefølgen som minimerer sum av gjenkjente
case-kostnader; casene ENDRES av rekkefølgen, så evaluer alle 24 grener
med recognizer-oppslag — ~100 oppslag totalt), deretter deterministisk
OLL-ID + PLL-ID. Ingen ekstern solver trengs for dette — men det ER
MPEE-disiplinen: kostmodell + plansøk. Vokser vokabularet (flere algs
per case, alternative metoder per stadium), vokser søket — da er native
MPEE-integrasjon (brooom-ILS over makrosekvenser) kandidaten, målt i
M7-stil før adopsjon.

## Manglende brikke: cross-løseren

Kryss er ikke makro-bart (~190k konfigurasjoner). Egen mikro-IDA*:
tilstand = 4 kantposisjoner+orientering, alltid løsbar ≤ 8 trekk,
pruning-tabell ~ få hundre KB generert på ms. ~100 linjer i cube-solver.

## Leveranse i appen

Ny knapp i guide-visningen: **«Min vei»** ved siden av «Korteste» —
segmentert avspilling der hvert segment viser ID-navnet (chip finnes),
estimert tid per segment fra brukerens statistikk, og total predikert
tid. Barn ser: «du kan løse denne på X sekunder med det DU kan».

## Status

Design klart; implementeres etter at gjeldende pipeline (wasm-bundle,
M7-måling) har landet. Byggeklossene finnes: recognizer med alle fire
y-rammer ✓, setup-søk ✓, ergonomi/overgangskost ✓, per-ID-statistikk i
MPEdb ✓. Nytt: cross-IDA, F2L-rekkefølgesøk, UI-modus.

## MPEE-teknikkene, presist plassert (Mortens spørsmål 2026-07-31)

- **Streaming NxN**: kostceller (ID×ID-overganger, løsetider per case)
  beregnes on-demand fra algoritmene/målingene — aldri materialisert
  matrise. Hot celler caches (tail_memo, solutions-tabellen). Implementert.
- **Ytterpunkt/grenseanalyse**: branch-and-bound i makrosøket med
  admissible nedre grense = sum av MIN(brukerkost) per gjenstående
  stadium — servert av MPEdbs indekserte min/max-grenseprobe
  (SELECT MIN(duration_ms) ... WHERE alg_id IN stadium). Kubens
  pruning-tabeller er samme idé på trekknivå.

## GPU-akselerasjon (trigger, ikke plan)

wgpu er allerede linket; WebGPU-stien i browser og Metal native har
compute. En brooom-aktig WGSL-megakernel som scorer kandidatruter
parallelt blir aktuell NÅR søket vokser til portefølje-skala (uttømmende
makrosekvenser / Monte-Carlo over feilrater) — i dag bruker søket µs og
GPU-dispatch ville dominert. Fallback: CPU-sti der bare WebGL2 finnes.
Beslutning tas på benchmark (M7-disiplin), på M3-en.

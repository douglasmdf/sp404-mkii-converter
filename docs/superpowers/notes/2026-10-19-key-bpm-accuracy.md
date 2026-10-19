# Precisão de tom/BPM — primeira calibração (2026-10-19)

## Conjunto

- **DAYDREAMS SAMPLE PACK** (`~/Desktop/DAYDREAMS SAMPLE PACK`, fora do repositório): 96 arquivos.
- Gabarito: `docs/superpowers/notes/daydreams-labels.csv` (`path,kind,label,bpm,source`).
  - `source=name`: tipo pela pasta, BPM pelo nome (`(NNbpm)`). `Snare_Tambo 1/2 (85bpm)` → `drumLoop`.
  - `source=reference`: tom/nota/acorde e o tipo dos `One Shots (Melodic)` vêm de `scripts/reference_labels.py` (Essentia 2.1b6, `KeyExtractor` perfil `edma`, `PitchYinFFT`, `ChordsDetection`). **Ainda não conferidos de ouvido** (passo 4 da Task 13).
  - Exceção: `Breaks_Trunk Thief (89bpm)` está com **84** (`source=reference`): o arquivo tem exatamente 32 tempos a 84 BPM (mesma duração de `L8Snack (84bpm)`) e o Essentia mede 84,1. Com 89 no gabarito, as duas execuções abaixo teriam um acerto de BPM a menos.
- O Freesound Loop Dataset (passo 5) não foi usado nesta rodada.

## Analisador

`ANALYZER_VERSION = 2` (commit `a4d31c2`). A linha de base é o analisador v1 (commit `9001158`).

## Comandos

```sh
cargo build --release -p sp404-cli
./target/release/sp404 analyze "$HOME/Desktop/DAYDREAMS SAMPLE PACK" --ignore-names \
  --labels docs/superpowers/notes/daydreams-labels.csv --report /tmp/daydreams-ign.md
./target/release/sp404 analyze "$HOME/Desktop/DAYDREAMS SAMPLE PACK" \
  --labels docs/superpowers/notes/daydreams-labels.csv --report /tmp/daydreams-names.md
```

Faixa de BPM: padrão `78-166`. Máquina: Apple M1, 8 núcleos.

## Resultados

| Medida | Meta | v1, sem nomes | **v2, sem nomes** | **v2, com nomes** |
|---|---|---|---|---|
| Classificação por tipo | ≥ 90% | 57/96 (59,4%) | **88/96 (91,7%)** ✅ | **90/96 (93,8%)** ✅ |
| BPM exato (±1 na faixa) | ≥ 90% | 5/33 (15,2%) | **29/33 (87,9%)** ❌ | **32/33 (97,0%)** ✅ |
| Erros de metade/dobro | — | 0/33 | 2/33 | 0/33 |
| Tom exato | ≥ 65% | 1/17 (5,9%) | **12/17 (70,6%)** ✅ | **12/17 (70,6%)** ✅ |
| Tom exato ou relativo | ≥ 80% | 1/17 (5,9%) | **13/17 (76,5%)** ❌ | **13/17 (76,5%)** ❌ |
| Tom, MIREX ponderado | — | 0,059 | 0,753 | 0,753 |
| Nota de one-shot | ≥ 95% | 0/3 | **0/3** ❌ (gabarito não conferido) | 0/3 |
| Acorde exato (reais) | relatado | 0/4 | 0/4 (raiz 1/4) | 0/4 (raiz 1/4) |
| Acorde exato (sintéticos, `cargo test`) | ≥ 70% | 13/13 | **13/13 (100%)** ✅ | — |
| Bateria sem prefixo de tom | ≥ 98% | 69/72 (95,8%) | **72/72 (100%)** ✅ | **72/72 (100%)** ✅ |
| Erros marcados "incerto" | ≥ 60% | 36/90 (40%) | **8/24 (33,3%)** ❌ | 6/19 (31,6%) ❌ |
| Tempo médio por arquivo (1 thread) | — | 96 ms | 95 ms | 98 ms |
| 96 arquivos, 8 workers (relógio) | 500 < 1 min | — | **1,24 s** (≈ 6,5 s para 500) ✅ | — |

A linha de base v1 foi medida com 89 para `Trunk Thief`; não muda o quadro.

"17 tons" = 12 sample loops + 5 frases da referência. Os acordes sintéticos são os 12 de `chords_of_each_quality` (2 fundamentais × 6 qualidades) mais `inversion_root_comes_from_bass`.

### Matriz de confusão, v2 sem nomes (linhas = gabarito)

| gabarito \ app | unpitched | note | chord | phrase | melodicLoop | drumLoop |
|---|---|---|---|---|---|---|
| unpitched | 50 | 0 | 0 | 0 | 0 | 0 |
| note | 1 | 0 | 0 | 2 | 0 | 0 |
| chord | 0 | 0 | 3 | 1 | 0 | 0 |
| phrase | 0 | 0 | 0 | 5 | 0 | 0 |
| melodicLoop | 0 | 0 | 0 | 0 | 10 | 2 |
| drumLoop | 2 | 0 | 0 | 0 | 0 | 20 |

Com nomes, as duas linhas `drumLoop → unpitched` (os pandeiros) somem: 22/22.

## Erros que sobram (v2, sem nomes)

| Arquivo | Erro | Marcado incerto? | Por quê |
|---|---|---|---|
| Breaks_Before Midnight (162bpm), Breaks_Change (158bpm) | BPM 81/79 em vez de 162/158 | sim | Oitava: 78–166 contém os dois. O pulso rápido é **mais fraco** que o lento nesses breaks e **mais forte** em `PercRythm (83bpm)`; o Essentia erra do mesmo jeito (81, 79 e 165). |
| Snare_Tambo 1/2 (85bpm) | one-shot sem BPM | não | Uma batida de pandeiro num arquivo de 2 tempos: pelo som é um one-shot. Com nomes, o comprimento bate com 85 BPM e vira loop (acerta). |
| Do the Thing (82bpm), Double Vision (84bpm) | loop de bateria (sem tom) | só Double Vision | Parciais sustentadas 0,17/0,22, no mesmo nível de `VibeyGoods` (percussão, 0,22). |
| Fading (73bpm) | Cm × Gm (ref.) | não | Temperley no próprio Essentia também dá Cm; edma/Krumhansl dão Gm. |
| Thinking Back (81bpm) | A × Gbm (ref.) | não | Relativo; Temperley no Essentia também dá A. |
| One Shots (Melodic) | 7 discordâncias de tipo/nota/acorde | 4 | Gabarito da referência, não conferido (ver lista da conferência de ouvido). |

## O que mudou (v1 → v2) e de onde veio cada regra

- **BPM** (`tempo.rs`): pontuação de "família" (máximo da autocorrelação perto de cada múltiplo do tempo, até 8 tempos) numa oitava 70–140; o candidato de **potência de 2 de tempos** pelo comprimento do arquivo (não aparado) ganha quando os ataques concordam (≥ 60% do melhor). Regra derivada de: 31 dos 33 loops têm 4–128 tempos; acabou com os erros 4/3 e 5/4 (`WoodBlockHats`, `The Luv`, `PercRythm (95bpm)`, `Need to Know`, `Wait Up`).
- **Loop ou one-shot** (`classify.rs`): pulso no tempo do comprimento acima da mediana da oitava, ou som longo (15→25 s) com ataques. Derivado de: sample loops 23–91 s (metade com pulso fraco); one-shots ≤ 17 s (`FX_Deep Space Siren`).
- **Com nota ou sem** (`features.rs`, `classify.rs`): fração de parciais sustentadas (±93 ms) em 250–2500 Hz sobre toda a energia de picos; derivado de: bateria e hats ≤ 0,22, loops melódicos e one-shots com nota em geral ≥ 0,25. Multiplicado por um índice de **glissando** (distância média da nota temperada > 15–22 cents com mais de 1 semitom de extensão); derivado das sirenes de `SFX`.
- **Tom** (`harmony.rs`): perfis de **Temperley** (Kostka-Payne) sobre o cromagrama com cada quadro normalizado. Comparação nos 17 tons: Temperley+normalizado 12, Krumhansl-Kessler 9 (normalizado ou não), Temperley sem normalizar 9, diatônico simples 8. Um perfil EDM próprio não foi testado (o `edma` do Essentia é o que gerou o gabarito, então compará-lo seria circular). Margem do relativo aumentada para 0,6 (o perfil menor de Temperley afasta o relativo maior).
- **Acorde/frase**: polifonia sozinha decide acorde depois de "com nota" (`Horn Stab wBass`); limiar de classe forte 0,5 → 0,4 (`Piano_Beauty Chord Octave`); notas sustentadas contadas depois de tirar a afinação da própria f0 (`Mallet Roll`); frase com só 2 notas fica "tipo incerto".
- **Nomes**: comprimento igual a um número inteiro de tempos (≥ 2) no BPM do nome → loop (`Snare_Tambo`).
- **Rotulagem** (`labeling.rs`): BPM detectado cujo dobro ou metade também cabe na faixa fica "BPM incerto".

## Metas: situação

- ✅ Classificação, tom exato, bateria sem prefixo, acordes sintéticos, velocidade.
- ✅ BPM só **com nomes** (97%); ❌ **sem nomes** (87,9%: 2 erros de oitava marcados como incertos + 2 pandeiros de uma batida só).
- ❌ Tom exato-ou-relativo (13/17): 2 dos 4 erros são loops melódicos classificados como bateria.
- ❌ Nota (0/3) e acorde real (0/4): o gabarito desses é só da referência, que discorda do app em quase todos; depende da conferência de ouvido.
- ❌ Erros marcados como incertos (33%): a maior parte dos "erros" restantes está em rótulos não conferidos; refazer depois do passo 4.

## O que falta

1. Conferência de ouvido do autor (lista em `.superpowers/sdd/.../task-13-ear-check.md`), corrigir o CSV (`source=author`) e medir de novo.
2. Com o gabarito conferido, recalibrar os limiares de confiança (meta de 60%).
3. Mais loops (Freesound Loop Dataset ou packs do autor): 12 loops melódicos são poucos para tom; cada loop vale 6 pontos percentuais.
4. Decisão do autor sobre a oitava ambígua dentro da faixa 78–166 e sobre o plano B (stratum-dsp), que não foi tentado.

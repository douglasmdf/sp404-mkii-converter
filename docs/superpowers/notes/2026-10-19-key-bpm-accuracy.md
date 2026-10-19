# Precisão de tom/BPM — calibração (2026-10-19)

Duas passagens: a primeira (analisador v2) sobre o gabarito da referência; a segunda (analisador v3, no fim deste arquivo) sobre o gabarito conferido de ouvido pelo autor.

# Passagem 1 (analisador v2)

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

# Passagem 2 (analisador v3, gabarito conferido pelo autor)

## Gabarito

O autor conferiu de ouvido os 13 arquivos da lista de discordâncias (com Serato Sample, KeyFinder, Tunebat e Antares Auto-Key de apoio). As respostas estão no CSV com `source=author`:

- Tipos: `Horn Stab wBass` acorde Cm7; `Piano_Beauty Chord` e `Chord Octave` acorde Bbmaj7; `OP1_ArpDelay` frase (arpejo de Fmaj7, tom F); `OP1_ReverseArp 1` frase A; `OP1_Climb Down` frase Em; `OP1_Pluck` nota G; `Mallet Roll` frase (tom ambíguo, sem rótulo); `Do the Thing` loop melódico Bbm; `Double Vision` loop melódico Am; `Fading` loop melódico (tom ambíguo entre ferramentas, sem rótulo); `Thinking Back` Gbm; `Breaks_Trunk Thief` 84 BPM (erro de digitação no nome).
- Os outros rótulos só da referência continuam `source=reference` (os que o app e o Essentia já davam iguais).
- Agora são 18 tons avaliados (12 loops, menos `Fading`, mais 7 frases), 1 nota e 3 acordes reais.

## Resultados (v3)

Mesmos comandos da passagem 1. Para comparar, v2 medido no gabarito novo:

| Medida | Meta | v2 sem nomes | **v3 sem nomes** | **v3 com nomes** |
|---|---|---|---|---|
| Classificação por tipo | ≥ 90% | 89/96 (92,7%) | **93/96 (96,9%)** ✅ | **95/96 (99,0%)** ✅ |
| BPM exato | ≥ 90% | 29/33 (87,9%) | **29/33 (87,9%)** ❌ | **32/33 (97,0%)** ✅ |
| Tom exato | ≥ 65% | 14/18 (77,8%) | **15/18 (83,3%)** ✅ | **15/18 (83,3%)** ✅ |
| Tom exato ou relativo | ≥ 80% | 15/18 (83,3%) | **16/18 (88,9%)** ✅ | **16/18 (88,9%)** ✅ |
| Tom, MIREX | — | 0,794 | 0,850 | 0,850 |
| Nota de one-shot (reais) | ≥ 95% | 0/1 | **1/1** ✅ (só 1 nota real) | 1/1 |
| Acorde exato (reais, só relatado) | — | 2/3 | **3/3** | 3/3 |
| Acorde exato (sintéticos) | ≥ 70% | 13/13 | **13/13** ✅ (+ acorde rolado/arpejo novo) | — |
| Bateria sem prefixo | ≥ 98% | 72/72 | **72/72** ✅ | **72/72** ✅ |
| Erros marcados "incerto" | ≥ 60% | 5/17 (29%) | **6/10 (60,0%)** ✅ (no limite) | **4/5 (80%)** ✅ |
| 96 arquivos, 8 workers | 500 < 1 min | — | **1,53 s** (≈ 8 s para 500) ✅ | — |

### Quanto os acertos são marcados como incertos (v3, sem nomes)

| Campo | Erros marcados | Acertos marcados |
|---|---|---|
| Tipo | 1/3 | 10/93 |
| BPM | 2/4 | 7/29 (todos na zona de oitava ambígua 78–83 ou loops longos sem pulso) |
| Tom/nota/acorde | 2/2 | 8/20 |

Na v2 (gabarito novo) a confiança do tom era pior que nada: 0/2 erros marcados e 9/20 acertos marcados.

### Matriz de confusão, v3 sem nomes

| gabarito \ app | unpitched | note | chord | phrase | melodicLoop | drumLoop |
|---|---|---|---|---|---|---|
| unpitched | 50 | 0 | 0 | 0 | 0 | 0 |
| note | 0 | 1 | 0 | 0 | 0 | 0 |
| chord | 0 | 0 | 3 | 0 | 0 | 0 |
| phrase | 0 | 0 | 1 | 7 | 0 | 0 |
| melodicLoop | 0 | 0 | 0 | 0 | 12 | 0 |
| drumLoop | 2 | 0 | 0 | 0 | 0 | 20 |

## Erros que sobram (v3, sem nomes)

| Arquivo | Erro | Marcado? |
|---|---|---|
| Breaks_Before Midnight, Breaks_Change | oitava de BPM (81/79) | sim |
| Snare_Tambo 1/2 | one-shot sem BPM (2 erros cada: tipo e BPM) | não — uma batida num arquivo de 2 tempos; com nomes acerta |
| OP1_Reverse Organ | acorde × frase (gabarito **só da referência**, não conferido) | sim |
| Do the Thing | tom Ab × Bbm | sim |
| Thinking Back | tom A × Gbm (relativo) | sim |

Com nomes sobram: `Trunk Thief` (o nome diz 89; não há como marcar um valor lido do nome), `Reverse Organ`, `Do the Thing` e `Thinking Back`.

## O que mudou (v2 → v3) e de onde veio

- **Com nota**: mais duas evidências. (1) Número de notas sustentadas distintas, 5→9: loops de bateria têm no máximo 5 (congas e tons afinados), `Do the Thing` tem 10 e `Double Vision` 25 (os dois têm palmas e percussão, por isso a fração de parciais sustentadas ficava baixa). (2) Fração de quadros com f0 clara, 0,6→1, só acima de 80→150 Hz e esmaecida pela duração como a medida de f0: `OP1_Pluck` (0,15 s, 787 Hz); os bumbos também têm f0 clara, mas em torno de 40 Hz.
- **Acorde × frase**: classes estáveis em ≥ 50% (antes 70%) dos quadros sustentados (`Piano_Beauty Chord`). Se as classes entram com mais de 0,18 s de diferença, é arpejo → frase (`OP1_ArpDelay`: 0,23 s; acordes de piano: 0,14 s; `Horn Stab`: 0,02 s).
- **Nota**: os quadros sem f0 nas pontas (a janela do YIN não cabe) não contam mais contra a nota (`OP1_Pluck`).
- **Confiança do tom**: segunda opinião com os perfis de Krumhansl-Kessler no mesmo cromagrama; só é confiante se os dois concordam. Margem de quase-empate 0,05 → 0,02; a margem do desempate (0,15) saiu. Derivado das 18 respostas conferidas (KK acerta onde Temperley erra em `Do the Thing` e `Thinking Back`, e o contrário em 4 outros).
- **Confiança do tipo**: nota/acorde/frase fica incerto quando a parte sustentada é menos de 25% do som (amostras invertidas: `OP1_Reverse Organ`, `OP1_HiPad`).
- Testes sintéticos: `disagreeing_hints_on_weak_sound_are_not_confident` agora escolhe a palavra de nome contrária à inclinação do som (o seno curto passou a inclinar para "com nota", pelo mesmo motivo do `OP1_Pluck`). Novos: acorde rolado × arpejo, progressão clara confiante.

## Metas: situação (v3)

- ✅ Todas as metas da seção 6 **com nomes**.
- **Sem nomes**: ✅ todas menos **BPM (87,9%)**. Os 4 erros de BPM são os 2 de oitava (marcados, inerentes à faixa 78–166) e os 2 pandeiros de uma batida só.
- Erros marcados como incertos: 60,0% sem nomes, exatamente no limite.
- Nota (1 real) e acorde (3 reais) têm amostra pequena demais para valer como medida; as metas valem pelos sintéticos.

## O que falta

- Mais loops conferidos (Freesound ou packs do autor): 12 loops melódicos e 1 nota real são pouco.
- `OP1_Reverse Organ`, `HiPad`, `Sharp Hits`, `Soft Hits` e os loops que o app e a referência dão igual continuam com gabarito só da referência.
- Decisão do autor sobre a meta de BPM sem nomes (oitava ambígua e pandeiros).

# Rodada de correções (analisador v4)

Correções pedidas na revisão:

- **Comprimento não decide loop sozinho:** a parte "som longo" do loop agora chega no máximo a 0,45, acima do ponto médio (0,35) e abaixo do limiar de certeza (0,5). Um som longo inclina para loop, mas sai como "tipo incerto", e as palavras de pasta e arquivo decidem. Um riser de 25 s em `One Shots` continua one-shot. Só o pulso deixa um loop confiante.
- **Comprimento pelo BPM do nome:** um arquivo cujo comprimento é um número inteiro de tempos no BPM do nome continua sendo loop. Agora é um loop incerto quando os ataques não inclinam para loop (`Snare_Tambo`).
- **Segunda opinião do tom:** a função devolve `Option`; sem segunda opinião, o tom não é confiante (antes havia um "Dó maior" silencioso como recurso).
- **Afinação:** a afinação já usava só picos de 55 Hz a 4 kHz. Os picos até 10 kHz entram só na fração de parciais sustentadas. Agora isso está documentado em `estimate_tuning`.

## Base da métrica de tom

Os **18 tons** são 11 loops melódicos (`Fading` sem rótulo) e 7 frases (`One Shots (Melodic)` com tom; `Mallet Roll` sem rótulo).

- `OP1_Reverse Organ` conta como erro de tom: o app diz acorde, e acorde não recebe tom.
- **Só loops:** exato 9/11 (81,8%), exato ou relativo 10/11 (90,9%).
- **Só frases:** 6/7 (`Reverse Organ` é o erro). O rótulo de acorde `Dbm` é igual ao da referência, mas o tipo está errado.

## Resultados (v4)

| Medida | Meta | **sem nomes** | **com nomes** |
|---|---|---|---|
| Classificação | ≥ 90% | 93/96 (96,9%) ✅ | 95/96 (99,0%) ✅ |
| BPM exato | ≥ 90% | 29/33 (87,9%) ❌ | 32/33 (97,0%) ✅ |
| Tom exato (11 loops + 7 frases) | ≥ 65% | 15/18 (83,3%) ✅ | 15/18 ✅ |
| Tom exato ou relativo | ≥ 80% | 16/18 (88,9%) ✅ | 16/18 ✅ |
| Nota (real) | ≥ 95% | 1/1 | 1/1 |
| Acorde (real; meta vale nos sintéticos, 13/13) | ≥ 70% | 3/3 | 3/3 |
| Bateria sem prefixo | ≥ 98% | 72/72 ✅ | 72/72 ✅ |
| Erros marcados como incertos | ≥ 60% | 6/10 (60,0%) ✅ | 4/5 (80%) ✅ |

As métricas não mudaram em relação à v3. O que mudou foi a quantidade de acertos marcados: os sample loops longos com pulso fraco agora saem como "tipo incerto" quando não há nomes.

## Erros e acertos marcados por medida

Mesma base do `sp404 analyze`: cada medida de cada arquivo conta uma vez. As somas dão os totais da tabela acima.

| Medida | Sem nomes: erros marcados | Sem nomes: acertos marcados | Com nomes: erros marcados | Com nomes: acertos marcados |
|---|---|---|---|---|
| Tipo | 1/3 | 17/93 | 1/1 | 7/95 |
| BPM | 2/4 | 7/29 | 0/1 | 0/32 |
| Tom | 3/3 | 7/15 | 3/3 | 7/15 |
| Nota | 0/0 | 0/1 | 0/0 | 0/1 |
| Acorde | 0/0 | 0/3 | 0/0 | 0/3 |
| **Total** | **6/10** | 31/141 | **4/5** | 14/146 |

## Comandos

`npm run build --prefix app` não foi preciso (`app/dist` já existia). `cargo test --workspace`: 193 testes passam, nenhum falha. Medição: mesmos comandos da passagem 1.

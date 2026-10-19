# Tom, BPM e arquivos soltos — Plano de Implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Aceitar arquivos de áudio soltos (Parte A) e, como opção, colocar nota, acorde, tom e/ou BPM detectados no início do nome de saída (Parte B), sem mudar nada quando a opção está desligada.

**Architecture:** A Parte A estende `scan::build_plan` para aceitar arquivos, que formam um grupo destinado a `<saída>/Samples [SP-404]`. A Parte B acrescenta um analisador próprio em Rust (`core/src/analyze/`) que mede o som e devolve uma `Detection` crua. `name_tags` lê tom/BPM já presentes no nome, e `labeling` junta as duas fontes num `Analysis` e no prefixo. `scan` passa a analisar os arquivos em paralelo, com progresso, cancelamento e cache, **antes** de decidir os nomes. O runner, o desfazer e a checagem de espaço não mudam.

**Tech Stack:** Rust 2021, symphonia `=0.5.5` (decode), rubato `=0.15.0` (resample, reaproveitado para a taxa de análise), `realfft = "=3.5.0"` (nova dependência direta; já está no `Cargo.lock` via rubato), rayon, Tauri 2 + TypeScript. Só no desenvolvimento: Python + Essentia para o gabarito de referência (Task 13), nunca no app nem como dependência do repositório.

**Spec:** `docs/superpowers/specs/2026-10-19-key-bpm-and-loose-files-design.md`

## Global Constraints

- Com `detect_key_bpm = false`, o `Plan` tem que ser **idêntico** ao da v0.1.0 para qualquer entrada só de pastas.
- Prefixo só com `[A-Za-z0-9]` e `_`, no início do nome, separado do resto por `_`: `<tom|nota|acorde>_<bpm>_<resto>`.
- Notas em **bemóis**: `C Db D Eb E F Gb G Ab A Bb B`. Tom menor = tônica + `m` (`Am`); maior = só a tônica (`C`). Acordes: `""`, `m`, `7`, `maj7`, `m7`, `dim`. BPM inteiro, sem "bpm".
- Prefixo por tipo: sem nota → nenhum; nota → nota; acorde → acorde; frase curta → tom; loop melódico → tom + BPM; loop de bateria → BPM.
- Faixas de BPM: `58-115`, `68-135`, `78-166` (padrão), `88-175`, `98-195`. O BPM sempre é dobrado ou dividido até cair na faixa; vale também para BPM vindo do nome.
- `detect_key_bpm` **desligado** por padrão.
- Ordem do nome: ler/remover tokens do nome original → limpar (se ligado) → achatar (se ligado) → prefixo na frente → tamanho máximo e colisões.
- Arquivos soltos: sempre perguntar a pasta de saída; destino `<pasta escolhida>/Samples [SP-404]` (reutilizada se existir), sem subpastas; arquivo solto que não é áudio é recusado.
- Análise: mono, reamostrada para **22 050 Hz**, só os primeiros **180 s**, sem aleatoriedade; mesma saída com 1 ou 8 workers.
- `ANALYZER_VERSION: u32 = 1`, gravado em cada `Analysis` (e portanto no log JSON). Incrementar sempre que limiares/perfis mudarem.
- Sem FFmpeg, GPL/AGPL ou modelos de ML no app. Nenhum áudio de terceiros commitado. O pack de teste fica em `~/Desktop/DAYDREAMS SAMPLE PACK` (fora do repo).
- Não atualizar symphonia nem rubato.
- Interface em pt-BR e inglês para todo texto novo.
- `cargo test --workspace` exige `app/dist`: rode `npm run build --prefix app` antes.
- Toda mensagem de commit termina com `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **A Parte B só é liberada (release) depois que a Task 13 atingir todas as metas.** A Parte A pode ser liberada antes.

## Review Focus

1. **Rodar duas vezes o mesmo pack com a detecção ligada:** os nomes têm que sair iguais, para "pular existentes" funcionar e não duplicar arquivos no cartão. Teste: Task 10 (`detection_names_are_deterministic_across_workers_and_runs`).
2. **Arquivos que não dão para analisar** (WAV com 0 frames, silêncio total, arquivo corrompido) com a detecção ligada: o Plano tem que ser montado, o arquivo fica sem prefixo e com o motivo, sem travar. Teste: Task 10 (`unanalyzable_files_get_no_prefix`).
3. **Tamanho máximo pequeno e colisões com prefixo:** `max_filename_length = 6` com `Am_90_Loop` e dois loops que viram o mesmo nome → o prefixo sobrevive e o segundo recebe `_2`. Teste: Task 10 (`prefix_survives_max_length_and_collisions`).
4. **Palavras que parecem tom sem ser** (`Amazing`, `Bbq`, `Dmx`, `I am`, `Take A`, `Kick 01`): nada pode ser lido como tom/BPM sem fronteira de palavra (ou, nos casos soltos, sem o som concordar). Testes: Task 4 (`key_tokens_need_word_boundaries`) e Task 9 (`bare_tokens_need_audio_agreement`).
5. **Cancelar durante a análise:** nada é gravado, a operação termina logo e um novo Dry Run funciona em seguida. Teste: Task 10 (`cancel_stops_analysis`) e verificação manual na Task 12.

---

## Mapa de arquivos

```
core/src/
  scan.rs            (mod.)  arquivos soltos; análise antes dos nomes; plan_with
  config.rs          (mod.)  detect_key_bpm, bpm_range, BpmRange
  error.rs           (mod.)  NotFound, NotAudioFile, Cancelled
  decode.rs          (mod.)  decode_file_limited
  runner.rs          (mod.)  FileResult.analysis (copiado do Job)
  tags.rs            (novo)  PitchClass, Key, Chord, SampleKind, formatação, fold_bpm, prefixo
  name_tags.rs       (novo)  leitura de tom/acorde/BPM no nome + remoção dos tokens
  labeling.rs        (novo)  Detection + tokens do nome → Analysis + prefixo
  analyze/mod.rs     (novo)  Detection, Estimate, NameHints, analyze_file, AnalysisCache, ANALYZER_VERSION
  analyze/signal.rs  (novo)  carregar mono 22 050 Hz, recortar silêncio
  analyze/features.rs(novo)  espectrograma, envelope de ataques, ataques, cromagrama, afinação
  analyze/pitch.rs   (novo)  f0 tipo YIN, nota de one-shot
  analyze/harmony.rs (novo)  acorde de one-shot, tom de loop (com desempate do relativo)
  analyze/tempo.rs   (novo)  BPM por tamanho do loop + autocorrelação
  analyze/classify.rs(novo)  tipo do arquivo (som + pistas de nome)
  analyze/synth.rs   (novo, #[cfg(test)]) sinais sintéticos para os testes
cli/src/main.rs      (mod.)  entradas soltas, flags, comando analyze
cli/src/eval.rs      (novo)  gabarito CSV e métricas
app/src-tauri/src/main.rs (mod.) inspect_inputs, plan_conversion assíncrono, cache
app/src/{api.ts,main.ts,i18n.ts}, app/index.html (mod.)
scripts/reference_labels.py (novo, só desenvolvimento)
docs/superpowers/notes/ (novo) gabaritos e resultados de precisão
```

---

## Parte A — Arquivos soltos

### Task 1: Arquivos soltos no core e na CLI

**Files:**
- Modify: `core/src/scan.rs`, `core/src/error.rs`, `cli/src/main.rs`
- Test: `core/tests/scan.rs`, `cli/tests/` (o arquivo de teste existente da CLI)

**Interfaces:**
- Produces:
  - `pub const LOOSE_FOLDER_NAME: &str = "Samples [SP-404]";`
  - `PackPlan` ganha `pub loose: bool` (serde `loose`). Para o grupo solto: `name = LOOSE_FOLDER_NAME`, `source_root = PathBuf::new()`, `dest_root = output_dir.join(LOOSE_FOLDER_NAME)`, cada `Job.source` é o caminho absoluto do arquivo.
  - `pub struct InputsSummary { pub packs: Vec<PackSummary>, pub loose: Option<PackSummary>, pub rejected: Vec<PathBuf> }` (camelCase) e `pub fn inspect_inputs(paths: &[PathBuf]) -> Result<InputsSummary>`: pastas → `packs`; arquivos convertíveis ou de áudio não suportado → `loose` (um `PackSummary` com `name = LOOSE_FOLDER_NAME`, `path` vazio); outros arquivos → `rejected`; caminho inexistente → `Err(CoreError::NotFound)`.
  - `build_plan(paths, output_dir, settings)` aceita pastas e arquivos misturados. Packs na ordem dada; o grupo solto (se houver) vem por último. Grupo solto sem `output_dir` → `Err(NoOutputDir)` qualquer que seja `save_mode`. Arquivo solto que não é áudio → `Err(CoreError::NotAudioFile(path))`. Caminho inexistente → `Err(CoreError::NotFound(path))`.
  - Novas variantes: `CoreError::NotFound(PathBuf)` ("not found: {0}"), `CoreError::NotAudioFile(PathBuf)` ("not an audio file: {0}").

- [ ] **Step 1: Escrever os testes que falham** em `core/tests/scan.rs` (substituir `file_instead_of_folder_is_rejected`):
  - `loose_files_go_to_samples_folder`: `a/Kick.wav` e `b/Loop.mp3` com `output_dir = out` → um pack `loose == true`, `dest_root == out.join("Samples [SP-404]")`, destinos `["Kick.wav", "Loop.wav"]`.
  - `loose_files_ignore_save_mode`: com `SaveMode::Fixed` e `fixed_directory = other`, o grupo solto usa `output_dir` (não `other`); uma pasta no mesmo lote vai para `other/<pack>`.
  - `loose_files_need_output_dir`: `SaveMode::NextToSource`, `output_dir = None`, um arquivo solto → `NoOutputDir`.
  - `loose_output_may_be_the_source_folder`: arquivo em `tmp/src/kick.wav`, `output_dir = tmp/src` → destino `tmp/src/Samples [SP-404]/kick.wav`, sem erro.
  - `loose_file_inside_output_is_rejected`: arquivo em `out/Samples [SP-404]/kick.wav`, `output_dir = out` → `OutputInsideSource`.
  - `loose_duplicate_names_are_numbered`: `a/kick.wav` e `b/Kick.aif` → `["kick.wav", "Kick_2.wav"]`.
  - `loose_non_audio_is_rejected`: `notes.pdf` solto → `NotAudioFile`; `missing.wav` → `NotFound`.
  - `inspect_inputs_sorts_paths`: uma pasta, um `.wav`, um `.opus`, um `.pdf` → 1 pack, `loose.audio_files == 1`, `loose.unsupported_files == 1`, `rejected == [pdf]`.
  - `folders_only_plan_is_unchanged`: o Plano de uma entrada só de pastas tem `loose == false` em todos os packs e os mesmos `dest` do teste `preserves_structure_and_converts_extension`.

- [ ] **Step 2: Rodar e ver falhar:** `cargo test -p sp404-core --test scan` → FAIL (`LOOSE_FOLDER_NAME`/`inspect_inputs` não existem).

- [ ] **Step 3: Implementar em `core/src/scan.rs`.** Separar as entradas em pastas e arquivos logo no início de `build_plan`. O grupo solto usa `CollisionResolver` com diretório relativo vazio e as mesmas regras de nome de `plan_pack` (limpeza, tamanho máximo; achatar não se aplica). `ensure_no_overlap` passa a receber as origens soltas como origens adicionais. O grupo solto participa do `used_roots`, para não coincidir com um pack chamado `Samples [SP-404]`.

- [ ] **Step 4: CLI.** Em `cli/src/main.rs`, renomear o argumento posicional para `inputs` (ajuda: "Pack folders and/or audio files"). `print_plan` mostra o caminho completo da origem quando `pack.loose`. Teste em `cli/tests`: `sp404 convert a.wav --out <dir> --dry-run` imprime `Samples [SP-404]`, e sem `--out` (com configurações em `SP404_DATA_DIR` vazio) termina com erro "no output folder chosen".

- [ ] **Step 5: Rodar tudo:** `cargo test -p sp404-core -p sp404-cli` → PASS.

- [ ] **Step 6: Commit** — `feat(core): accept loose audio files into a Samples [SP-404] folder`.

### Task 2: Arquivos soltos no app

**Files:**
- Modify: `app/src-tauri/src/main.rs`, `app/src/api.ts`, `app/src/main.ts`, `app/src/i18n.ts`, `app/index.html`, `app/src/styles.css` (se preciso)

**Interfaces:**
- Consumes: `scan::inspect_inputs`, `InputsSummary`, `PackPlan.loose` (Task 1).
- Produces: comando Tauri `inspect_inputs(paths: Vec<PathBuf>) -> Result<InputsSummary, String>` (substitui `inspect_packs`); `api.inspectInputs(paths)`; `PackPlan.loose: boolean` em `api.ts`.

- [ ] **Step 1: Tauri.** Trocar `inspect_packs` por `inspect_inputs`, registrado no `generate_handler!`.
- [ ] **Step 2: Estado da UI.** `main.ts` guarda `packs: PackSummary[]` e `looseFiles: string[]`. `addPaths` chama `inspectInputs` com os caminhos novos; acrescenta packs; acrescenta arquivos soltos sem repetir; para cada `rejected` mostra `t("notAudio", { name })`. A lista mostra os packs e, se houver, uma entrada `t("looseEntry", { count })` com o botão Remover (remove todos os soltos).
- [ ] **Step 3: Pasta de saída.** `buildPlan()` pergunta a pasta quando `settings.saveMode === "ask" || looseFiles.length > 0`; o título do diálogo é `t("chooseOutputLoose")` quando só há soltos. Chama `planConversion([...packs.map(p => p.path), ...looseFiles], outputDir)`. `renderPlan` mostra o caminho completo da origem para `pack.loose`.
- [ ] **Step 4: Botões e atalhos.** Trocar "Procurar…" por dois botões: `addFolders` (seletor de pastas, `multiple: true`, Cmd/Ctrl+O) e `addFiles` (seletor de arquivos, `multiple: true`, filtro com as extensões de `CONVERTIBLE_EXTENSIONS` + `UNSUPPORTED_AUDIO_EXTENSIONS` copiadas de `core/src/scan.rs`, Cmd/Ctrl+Shift+O). `dropHint` passa a citar arquivos.
- [ ] **Step 5: Textos (en / pt-BR):**
  - `dropHint`: "Drop sample pack folders or audio files here" / "Arraste pastas de sample packs ou arquivos de áudio para cá"
  - `addFolders`: "Add folders…" / "Adicionar pastas…"; `addFiles`: "Add files…" / "Adicionar arquivos…"
  - `looseEntry`: "Loose files ({count})" / "Arquivos soltos ({count})"
  - `notAudio`: "{name} is not an audio file and was not added" / "{name} não é um arquivo de áudio e não foi adicionado"
  - `chooseOutputLoose`: "Choose where to create the Samples [SP-404] folder" / "Escolha onde criar a pasta Samples [SP-404]"
- [ ] **Step 6: Verificar:** `npm run build --prefix app && cargo test --workspace` → PASS. Rodar o app (`npm run tauri dev --prefix app`), arrastar 2 WAVs de `~/Desktop/DAYDREAMS SAMPLE PACK/One Shots/Kick` e uma pasta juntos, Dry Run, escolher a própria pasta `Kick` → destino `.../Kick/Samples [SP-404]/…`; arrastar um PDF → mensagem `notAudio`.
- [ ] **Step 7: Commit** — `feat(app): add loose audio files by drag or Add files…`.

---

## Parte B — Tom e BPM

### Task 3: Valores musicais, faixa de BPM e configurações

**Files:**
- Create: `core/src/tags.rs`
- Modify: `core/src/config.rs`, `core/src/lib.rs`

**Interfaces:**
- Produces (`tags.rs`, todos `Serialize`/`Deserialize` camelCase, `Copy` onde couber):
  - `pub struct PitchClass(pub u8)` (0 = C … 11 = B); `fn name(self) -> &'static str` (bemóis); `fn from_letter(letter: char, accidental: i8) -> PitchClass` (`#` = +1, `b` = −1, com volta no 12).
  - `pub struct Key { pub tonic: PitchClass, pub minor: bool }`; `fn label(&self) -> String`; `fn relative(&self) -> Key`.
  - `pub enum ChordQuality { Major, Minor, Dominant7, Major7, Minor7, Diminished }` com `fn suffix(self) -> &'static str` (`""`, `"m"`, `"7"`, `"maj7"`, `"m7"`, `"dim"`); `pub struct Chord { pub root: PitchClass, pub quality: ChordQuality }`; `fn label(&self) -> String`.
  - `pub enum SampleKind { Unpitched, Note, Chord, Phrase, MelodicLoop, DrumLoop }`.
  - `pub fn fold_bpm(bpm: f32, range: BpmRange) -> u32`: multiplica/divide por 2 até `min ≤ bpm ≤ max`, depois arredonda.
  - `pub fn prefix(label: Option<&str>, bpm: Option<u32>) -> Option<String>`: `("Am", 90) → "Am_90"`, `(None, 90) → "90"`, `("C", None) → "C"`, `(None, None) → None`.
  - `pub fn prepend(prefix: Option<&str>, stem: &str) -> String`: `("Am_90", "Loop") → "Am_90_Loop"`, `("Am_90", "") → "Am_90"`, `(None, s) → s`.
- Produces (`config.rs`): `pub enum BpmRange { R58_115, R68_135, R78_166, R88_175, R98_195 }`, serde como `"58-115"`… (`#[serde(rename = "78-166")]` etc.), `Default = R78_166`, `fn bounds(self) -> (f32, f32)`, `impl FromStr` (aceita `"78-166"`). `Settings` ganha `pub detect_key_bpm: bool` (padrão `false`) e `pub bpm_range: BpmRange`.

- [ ] **Step 1: Testes que falham** (unitários em `tags.rs` e `config.rs`):
  - `pitch_names_use_flats`: os 12 nomes na ordem `C Db D Eb E F Gb G Ab A Bb B`; `from_letter('F', 1).name() == "Gb"`; `from_letter('C', -1).name() == "B"`.
  - `key_and_chord_labels`: `Key{A, minor}` → `"Am"`; `Key{C, major}` → `"C"`; `Key{A,minor}.relative()` → `"C"`; os 6 sufixos de acorde em `C`: `C Cm C7 Cmaj7 Cm7 Cdim`.
  - `fold_bpm_lands_in_range`: na faixa `78-166`: `70.0→140`, `192.0→96`, `169.0→85`, `90.4→90`, `40.0→160`; na faixa `58-115`: `162.0→81`.
  - `prefix_and_prepend`: os quatro casos de cada função acima.
  - `defaults_match_spec` (atualizar): `!detect_key_bpm`, `bpm_range == R78_166`; `json_uses_camel_case` passa a checar `"detectKeyBpm":false` e `"bpmRange":"78-166"`; `"58-115".parse::<BpmRange>()` ok, `"60-120"` erro.
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core tags config` → FAIL.
- [ ] **Step 3: Implementar** os tipos e funções acima.
- [ ] **Step 4: Rodar:** mesmo comando → PASS.
- [ ] **Step 5: Commit** — `feat(core): musical tag types, BPM ranges and key/BPM settings`.

### Task 4: Ler tom, acorde e BPM no nome

**Files:**
- Create: `core/src/name_tags.rs`
- Modify: `core/src/lib.rs`

**Interfaces:**
- Consumes: `PitchClass`, `Key`, `Chord`, `ChordQuality` (Task 3).
- Produces:
  - `pub struct Span { pub start: usize, pub end: usize }` (bytes no stem original).
  - `pub struct NameTokens { pub key: Option<(Key, Vec<Span>)>, pub chord: Option<(Chord, Vec<Span>)>, pub bpm: Option<(f32, Vec<Span>)>, pub bare_bpm: Option<(f32, Span)>, pub bare_letter: Option<(PitchClass, Span)> }` (`Default`).
  - `pub fn parse_name(stem: &str) -> NameTokens`.
  - `pub fn remove_spans(stem: &str, spans: &[Span]) -> String`.

Regras (cópia da spec 4.4, com as decisões de leitura):
- Separadores de palavra: espaço, `_`, `-`, `.`, `,`, `(`, `)`, `[`, `]`, e começo/fim do stem.
- BPM marcado: palavra `\d{2,3}bpm`, `bpm\d{2,3}`, ou número seguido/precedido da palavra `bpm` (sem diferenciar maiúsculas); valor entre 40 e 300. Os spans incluem a palavra `bpm`.
- Acorde marcado: palavra `[A-G](#|b)?(maj7|m7|7|dim)` (tônica maiúscula).
- Tom marcado: palavra `[A-G](#|b)?(m|min|minor|maj|major)?` que tenha acidente **ou** sufixo; ou letra `A–G` seguida da palavra `min|minor|maj|major` (spans das duas palavras). `m` só minúsculo; tônica sempre maiúscula. `Bb`, `Eb` sozinhos são tons maiores.
- Solto: palavra só de 2–3 dígitos entre 50 e 250 → `bare_bpm`; palavra que é só uma letra maiúscula `A–G` → `bare_letter`.
- Se houver mais de um candidato do mesmo campo, vale o último.
- `remove_spans`: apaga os trechos, remove pares `()`/`[]` que ficaram vazios (com espaços dentro), reduz cada sequência de separadores ao primeiro caractere dela e tira separadores das pontas.

- [ ] **Step 1: Testes que falham** (unitários):
  - `parses_spec_examples`: para cada linha, `parse_name` + `remove_spans` com os spans de tom/acorde/BPM encontrados:
    - `"Loop_90bpm_A minor"` → key `Am`, bpm 90, resto `"Loop"`
    - `"Keys F#m 140 BPM"` → key `Gbm`, bpm 140, resto `"Keys"`
    - `"808 C#"` → key `Db` (maior), resto `"808"`
    - `"Break 92bpm"` → bpm 92, resto `"Break"`
    - `"Breaks_Fly (90bpm)"` → bpm 90, resto `"Breaks_Fly"`
    - `"Snare_Tambo 1 (85bpm)"` → bpm 85, resto `"Snare_Tambo 1"`
    - `"Piano Cmaj7"` → chord `Cmaj7`, sem key, resto `"Piano"`
  - `key_tokens_need_word_boundaries` (Review Focus 4): `"Amazing Pad"`, `"Bbq Loop"`, `"Dmx Hit"`, `"I am here"`, `"Cmajestic"` → `key`, `chord` e `bare_letter` todos `None`.
  - `bare_tokens_are_reported_not_trusted`: `"Kick 01"` → nenhum `bare_bpm` (fora de 50–250); `"Loop 92"` → `bare_bpm == 92`, `bpm == None`; `"Take A"` → `bare_letter == A`, `key == None`.
  - `last_candidate_wins`: `"Loop 90bpm alt 92bpm"` → bpm 92.
  - `remove_spans_cleans_separators`: `"Loop (90bpm) [Am]"` com os dois spans → `"Loop"`; stem inteiro removido → `""`.
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core name_tags` → FAIL.
- [ ] **Step 3: Implementar** com um tokenizador que guarda os spans em bytes (sem regex; a crate não tem `regex` e não precisa ganhar).
- [ ] **Step 4: Rodar:** PASS.
- [ ] **Step 5: Commit** — `feat(core): read key, chord and BPM already present in file names`.

### Task 5: Sinal de análise e características

**Files:**
- Create: `core/src/analyze/mod.rs`, `core/src/analyze/signal.rs`, `core/src/analyze/features.rs`, `core/src/analyze/synth.rs`
- Modify: `core/src/decode.rs`, `core/Cargo.toml` (`realfft = "=3.5.0"`), `core/src/lib.rs`

**Interfaces:**
- Consumes: `decode`, `process::resample`.
- Produces:
  - `decode::decode_file_limited(path: &Path, max_seconds: Option<f64>) -> Result<DecodedAudio>`: para de ler pacotes quando passa do limite; `decode_file(path)` passa a ser `decode_file_limited(path, None)` (comportamento idêntico).
  - `analyze::ANALYSIS_RATE: u32 = 22_050`, `analyze::MAX_ANALYSIS_SECONDS: f64 = 180.0`, `analyze::ANALYZER_VERSION: u32 = 1`.
  - `signal::Signal { pub samples: Vec<f32>, pub rate: u32, pub source_seconds: f64 }` e `signal::load(path: &Path) -> Result<Signal>`: decode limitado, mono (média dos canais), reamostrado para `ANALYSIS_RATE`, silêncio do começo/fim recortado (limiar −60 dBFS relativo ao pico); `source_seconds` = duração após o recorte.
  - `features::Features { pub hop_seconds: f32, pub onset_env: Vec<f32>, pub onsets: Vec<usize>, pub chroma: Vec<[f32; 12]>, pub bass_chroma: Vec<[f32; 12]>, pub tuning_cents: f32, pub rms: Vec<f32> }` e `features::extract(signal: &Signal) -> Features`. Cromagrama a partir de magnitudes de FFT (55 Hz–4 kHz, mapeadas à classe de nota mais próxima após corrigir a afinação); `bass_chroma` só de ~30–250 Hz; envelope de ataques por spectral flux. Tamanhos de janela ficam com o implementador.
  - `synth.rs` (`#[cfg(test)] pub(crate)`): `tone(midi: f32, seconds: f32, harmonics: usize) -> Signal`, `glide_808(from_midi: f32, to_midi: f32, seconds: f32) -> Signal`, `chord(midis: &[f32], seconds: f32) -> Signal`, `arpeggio(midis: &[f32], note_seconds: f32) -> Signal`, `click_loop(bpm: f32, beats: usize) -> Signal`, `progression(chords: &[&[f32]], bass: &[f32], bpm: f32, beats_per_chord: usize, repeats: usize) -> Signal`, `noise_hit(seconds: f32) -> Signal`, `mix(a: &Signal, b: &Signal) -> Signal`, todos em `ANALYSIS_RATE`, determinísticos (ruído com gerador fixo, sem `rand`).

- [ ] **Step 1: Testes que falham:**
  - `decode::limited_decode_stops_early` (em `core/tests/decode.rs`): WAV de 10 s a 48 kHz, `decode_file_limited(_, Some(2.0))` → entre 2 e 3 s de frames; `None` → 10 s.
  - `signal::load_resamples_and_trims`: WAV estéreo 48 kHz com 0,5 s de silêncio + 1 s de seno 440 Hz → `rate == 22_050`, `source_seconds` entre 0,95 e 1,05.
  - `features::chroma_peaks_on_note`: `tone(57.0 /*A3*/, 2.0, 6)` → classe 9 (A) é a maior do cromagrama médio.
  - `features::tuning_is_estimated`: `tone(57.3, 2.0, 6)` → `tuning_cents` entre 20 e 40, e a classe de pico continua A.
  - `features::bass_chroma_ignores_highs`: `mix(tone(36.0 /*C2*/), tone(81.0 /*A5*/))` → pico de `bass_chroma` em C.
  - `features::onsets_of_click_loop`: `click_loop(90.0, 8)` → 8 ataques (±1).
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core analyze decode` → FAIL.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar:** PASS. Conferir que os testes existentes de `decode` e `convert` continuam passando.
- [ ] **Step 5: Commit** — `feat(core): analysis signal and spectral features`.

### Task 6: Nota de one-shot (f0)

**Files:**
- Create: `core/src/analyze/pitch.rs`

**Interfaces:**
- Consumes: `Signal`, `Features` (Task 5), `PitchClass` (Task 3).
- Produces:
  - `pub struct Estimate<T> { pub value: T, pub confident: bool }` (em `analyze/mod.rs`, `Serialize`, `Clone`).
  - `pitch::f0_track(signal: &Signal) -> Vec<Option<(f32, f32)>>` (Hz, clareza 0–1 por quadro; método tipo YIN/McLeod, faixa 30–2000 Hz).
  - `pitch::one_shot_note(signal: &Signal, features: &Features) -> Option<Estimate<PitchClass>>`: mediana da f0 nos quadros com clareza alta **depois do ataque**; com glide, mediana da última metade do trecho sustentado; `None` se menos de 30% dos quadros sustentados têm f0 clara; `confident` quando a dispersão da f0 é menor que meio semitom.
  - `pitch::pitch_stability(track: &[Option<(f32, f32)>]) -> f32` (fração de quadros claros com f0 estável), usada pela classificação.

- [ ] **Step 1: Testes que falham:**
  - `notes_of_harmonic_tones`: para `midi` em `[28 (E1), 36, 45, 57, 64, 72, 84]`, `one_shot_note(tone(midi, 1.5, 8))` → classe `midi % 12`, `confident`.
  - `detuned_tone_rounds_to_nearest`: `tone(57.3)` → A; `tone(56.7)` → A.
  - `glide_808_uses_arrival_note`: `glide_808(43.0, 36.0, 1.2)` → C.
  - `noise_has_no_note`: `one_shot_note(noise_hit(0.3))` → `None`.
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core analyze::pitch` → FAIL.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar:** PASS.
- [ ] **Step 5: Commit** — `feat(core): one-shot note detection`.

### Task 7: Acorde de one-shot e tom de loop

**Files:**
- Create: `core/src/analyze/harmony.rs`

**Interfaces:**
- Consumes: `Features`, `Estimate`, `Key`, `Chord`, `ChordQuality`.
- Produces:
  - `harmony::estimate_chord(features: &Features) -> Option<Estimate<Chord>>`: cromagrama médio da parte sustentada contra 72 modelos (12 fundamentais × 6 qualidades, com peso para harmônicos); fundamental desempatada por `bass_chroma`; `confident` pela margem entre o melhor e o segundo modelo.
  - `harmony::estimate_key(features: &Features) -> Option<Estimate<Key>>`: correlação do cromagrama médio com 24 perfis. Começar com Krumhansl-Kessler; a Task 13 pode trocar o conjunto de perfis. Quando o melhor e o segundo forem tons relativos com pontuações próximas, desempatar somando: (1) peso da tônica de cada um em `bass_chroma`; (2) peso da tônica no cromagrama do primeiro e do último segundo; (3) quantas vezes o acorde de tônica de cada um (`Am` × `C`) é o melhor modelo nos trechos de 1 batida (ou de ~0,5 s se não houver ataques). `confident` quando a margem final passa do limiar.
  - `harmony::is_polyphonic(features: &Features) -> bool` (≥3 classes estáveis e fortes ao mesmo tempo), usada pela classificação.

- [ ] **Step 1: Testes que falham:**
  - `chords_of_each_quality`: para fundamental C (60) e Eb (63), os 6 acordes (`[0,4,7]`, `[0,3,7]`, `[0,4,7,10]`, `[0,4,7,11]`, `[0,3,7,10]`, `[0,3,6]` somados à fundamental) → rótulo certo.
  - `inversion_root_comes_from_bass`: `mix(chord([64,67,72]), tone(48.0))` → `C` (não `Em`).
  - `key_of_progression`: `progression` i–iv–v–i em A menor com baixo A/D/E/A a 90 BPM → `Am`; I–IV–V–I em Eb maior → `Eb`.
  - `relative_tiebreak_uses_bass_and_ends`: mesma nota base (C maior/A menor), progressão `Am–F–C–G–Am` com baixo começando e terminando em A → `Am`; `C–Am–F–G–C` com baixo em C → `C`.
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core analyze::harmony` → FAIL.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar:** PASS.
- [ ] **Step 5: Commit** — `feat(core): chord and key estimation with relative-key tie-break`.

### Task 8: BPM

**Files:**
- Create: `core/src/analyze/tempo.rs`

**Interfaces:**
- Consumes: `Features`, `Signal.source_seconds`, `Estimate`.
- Produces:
  - `tempo::estimate_bpm(features: &Features, seconds: f64) -> Option<Estimate<f32>>`: BPM **cru** (antes de encaixar na faixa; o encaixe é feito por `fold_bpm` na Task 9, então o cache não depende da faixa). Candidatos por tamanho: `60 × b / seconds` para `b` inteiro com resultado entre 40 e 300; pontuação = concordância com a autocorrelação do envelope de ataques (considerando metade/dobro como a mesma família) + bônus para `b` múltiplo de 4, depois par, + bônus quando o resultado é quase inteiro (±0,05). Se nenhum candidato concorda, usar o pico da autocorrelação. `confident` quando um candidato por tamanho concorda com a autocorrelação.
  - `tempo::periodicity(features: &Features) -> f32` (saliência do pico da autocorrelação, 0–1), usada pela classificação.

- [ ] **Step 1: Testes que falham:**
  - `trimmed_click_loops`: `click_loop(bpm, beats)` para `(90, 16)`, `(80, 64)`, `(77, 30)`, `(162, 32)`, `(61, 64)` → `fold_bpm(valor, R78_166)` igual a `fold_bpm(bpm, R78_166)`, `confident`.
  - `untrimmed_loop_uses_autocorrelation`: `click_loop(95, 18)` com 0,37 s extra de cauda → 95 após o encaixe.
  - `single_hit_has_no_bpm`: `noise_hit(0.3)` → `None` ou `confident == false`.
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core analyze::tempo` → FAIL.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar:** PASS.
- [ ] **Step 5: Commit** — `feat(core): loop-length aware tempo estimation`.

### Task 9: Classificação, `analyze_file` e junção com o nome

**Files:**
- Create: `core/src/analyze/classify.rs`, `core/src/labeling.rs`
- Modify: `core/src/analyze/mod.rs`, `core/src/lib.rs`

**Interfaces:**
- Consumes: Tasks 3–8.
- Produces:
  - `analyze::NameHints { pub words: Vec<String>, pub has_key_token: bool, pub has_bpm_token: bool }` e `NameHints::from_path(rel_dirs: &[String], stem: &str, tokens: &NameTokens) -> NameHints`: palavras em minúsculas de `rel_dirs` (para packs: nome da pasta do pack + subpastas; para soltos: só a pasta do arquivo — quem chama decide) e do stem; `"one shot"`/`"one shots"` viram `oneshot`.
  - `classify::classify(signal: &Signal, features: &Features, hints: &NameHints) -> Estimate<SampleKind>`. Listas de pistas da spec 4.6, passo 5 (copiar literalmente); `808` e `bass` sempre contam como "com nota". Tom no nome puxa para "com nota"; BPM no nome puxa para "loop". O som vence quando a evidência é forte; as pistas decidem quando é fraca; `confident == false` quando som e pistas discordam e o som não é forte.
  - `analyze::Detection { pub kind: Estimate<SampleKind>, pub note: Option<Estimate<PitchClass>>, pub chord: Option<Estimate<Chord>>, pub key: Option<Estimate<Key>>, pub bpm: Option<Estimate<f32>> }` (`Clone`, `Serialize`). Só os campos que o tipo usa são calculados.
  - `analyze::analyze_file(path: &Path, hints: &NameHints) -> Result<Detection>`.
  - `labeling::Analysis` (camelCase, `Clone`, `Serialize`, `Deserialize`, `PartialEq`): `kind: SampleKind`, `label: Option<String>`, `bpm: Option<u32>`, `kind_uncertain: bool`, `label_uncertain: bool`, `bpm_uncertain: bool`, `label_from_name: bool`, `bpm_from_name: bool`, `prefix: Option<String>`, `error: Option<String>`, `analyzer_version: u32`.
  - `labeling::resolve(detection: &Detection, tokens: &NameTokens, range: BpmRange) -> (Analysis, Vec<Span>)`: escolhe os campos que o tipo usa (Global Constraints); valor do nome vence o detectado (`label_from_name`/`bpm_from_name`, sem marca de incerteza); para nota, aceita a tônica de um tom marcado; para acorde, aceita acorde marcado ou tom marcado (`Am` → acorde `Am`); `bare_bpm` só vale se o BPM cru detectado estiver a ±3% dele, da metade ou do dobro; `bare_letter` só vale se o tom detectado tiver essa tônica (rótulo = tom detectado) ou se o relativo dele tiver (rótulo = o relativo, ex.: letra `A` + detectado `C` → `Am`), e nesse caso conta como `label_from_name`; todo BPM passa por `fold_bpm`; devolve os spans a remover **só** dos valores usados.
  - `labeling::failed(error: &str) -> Analysis` (`kind: Unpitched`, sem prefixo, `error` preenchido).

- [ ] **Step 1: Testes que falham:**
  - `classify_synthetic_kinds` (sem pistas): `noise_hit` → `Unpitched`; `tone(36, 1.5)` → `Note`; `chord([60,63,67], 2.0)` → `Chord`; `arpeggio([57,60,64,67], 0.25)` → `Phrase`; `progression(Am, 90 BPM, 4 repetições)` → `MelodicLoop`; `click_loop(90, 16)` → `DrumLoop`.
  - `hints_decide_ambiguous_cases`: um ataque curto de seno amortecido em 60 ms (ambíguo) com pistas `["kick"]` → `Unpitched`; com `["808"]` → `Note`; com `["one shots","snare"]` e `has_bpm_token` num `click_loop` → `DrumLoop`.
  - `resolve_uses_fields_of_kind`: detecção `MelodicLoop`, key `Am`, bpm 90.2 → prefixo `Am_90`; mesma detecção como `DrumLoop` → `90`; `Unpitched` com `key` no nome → prefixo `None` e spans vazios (o tom fica no nome).
  - `name_values_win`: tokens `Gbm` + bpm 140 e detecção `Am`/92 → `Gbm_140`, `label_from_name`, `bpm_from_name`.
  - `bare_tokens_need_audio_agreement` (Review Focus 4): `bare_bpm 92` com detecção 46.1 → usa 92; com detecção 120 → ignora e usa o detectado. `bare_letter A` com detecção `C` maior → `Am` (`label_from_name`); com detecção `Am` → `Am`; com detecção `Eb` → ignora e usa `Eb`.
  - `name_bpm_is_folded`: tokens bpm 192, faixa `78-166` → `96`.
  - `uncertain_flags_follow_estimates`: detecção com `key.confident == false` → `label_uncertain`.
  - `analyze_file_on_wav` (em `core/tests/`): WAV de seno 220 Hz de 1 s → `kind == Note`, nota `A`; WAV de 0 frames → `Err` (quem chama converte com `labeling::failed`).
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core classify labeling analyze` → FAIL.
- [ ] **Step 3: Implementar.** Limiares iniciais escolhidos pelo implementador para passar os testes sintéticos; a Task 13 calibra.
- [ ] **Step 4: Rodar:** PASS.
- [ ] **Step 5: Commit** — `feat(core): classify samples and merge detection with name info`.

### Task 10: Análise no Plano (paralela, cancelável, com cache)

**Files:**
- Modify: `core/src/scan.rs`, `core/src/runner.rs`, `core/src/analyze/mod.rs`
- Test: `core/tests/scan.rs`, `core/tests/runner.rs`

**Interfaces:**
- Consumes: Tasks 1, 3, 4, 9.
- Produces:
  - `Job` ganha `pub analysis: Option<Analysis>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`, para o Plano com a detecção desligada serializar igual ao de antes).
  - `FileResult` ganha `pub analysis: Option<Analysis>` (mesmo serde), copiado do `Job` pelo runner — assim aparece no log JSON e no resumo.
  - `analyze::AnalysisCache` (`Default`, `Sync`): `fn get_or_analyze(&self, path: &Path, hints: &NameHints) -> Result<Detection>`, chave = caminho + tamanho + data de modificação + `hints` (as pistas mudam a classificação).
  - `CoreError::Cancelled` ("cancelled").
  - `scan::PlanProgress { Analyzing { done: usize, total: usize } }` (serde `tag = "event"`, camelCase).
  - `scan::plan_with(paths: &[PathBuf], output_dir: Option<&Path>, settings: &Settings, cache: &AnalysisCache, cancel: &AtomicBool, on_progress: impl Fn(PlanProgress) + Sync) -> Result<Plan>`. `build_plan(paths, output_dir, settings)` passa a chamar `plan_with` com um cache novo, cancel `false` e progresso ignorado.
  - Fluxo em `plan_with`: (1) listar arquivos de todos os packs/soltos como hoje; (2) se `settings.detect_key_bpm`, para cada job `Convert`: `parse_name(stem)` e `NameHints::from_path`, e analisar em paralelo num pool rayon com `settings.workers`, checando `cancel` antes de cada arquivo (→ `Err(Cancelled)`), chamando `on_progress` a cada arquivo concluído; erro de análise → `labeling::failed`; (3) **em ordem**, montar os nomes: `remove_spans` → limpar → achatar → `prepend(analysis.prefix)` → `CollisionResolver::claim` (tamanho máximo e colisões como hoje). Com a detecção desligada, o passo (2) não roda e o passo (3) é o código atual.

- [ ] **Step 1: Testes que falham** (`core/tests/scan.rs`; gerar WAVs com `common::write_sine_wav` e um loop de cliques escrito com `common::write_i16_wav`):
  - `detection_off_plan_is_unchanged`: o JSON do Plano de um pack de teste com `detect_key_bpm = false` é igual ao de `build_plan` antes da mudança (comparar com um snapshot literal no teste, sem campo `analysis`).
  - `detection_prefixes_names`: pack com `One Shots/Bass/sub.wav` (seno 55 Hz, 1 s), `Loops/clicks (90bpm).wav` (cliques, 16 batidas a 90 BPM) e `Drums/Kick/kick.wav` (ruído 0,2 s) → destinos `One Shots/Bass/A_sub.wav`, `Loops/90_clicks.wav`, `Drums/Kick/kick.wav`.
  - `prefix_survives_max_length_and_collisions` (Review Focus 3): `max_filename_length = Some(6)`, dois loops que viram `90_clicks` → `90_cli.wav` e `90_c_2.wav`.
  - `flatten_puts_prefix_first`: `flatten_folders = true` → `90_Loops_clicks.wav`.
  - `unanalyzable_files_get_no_prefix` (Review Focus 2): WAV de 0 frames, WAV de silêncio de 1 s e um `.wav` com bytes inválidos → Plano montado sem erro e os três sem prefixo; o de 0 frames e o inválido têm `analysis.error` preenchido; o de silêncio pode sair como `Unpitched` sem erro.
  - `detection_names_are_deterministic_across_workers_and_runs` (Review Focus 1): o mesmo pack com `workers = 1` e `workers = 8`, e duas execuções seguidas → `Plan`s iguais.
  - `cancel_stops_analysis` (Review Focus 5): `cancel` já `true` → `Err(Cancelled)` e nada criado no disco; progresso chamado `total` vezes numa execução normal.
  - `cache_avoids_second_analysis`: duas chamadas de `plan_with` com o mesmo cache → a segunda não decodifica (contar via um contador de testes no cache, `#[cfg(test)]` ou um método `len()`/`hits()`).
  - Em `core/tests/runner.rs`: `json_log_includes_analysis` → com a detecção ligada e `write_json_log`, o log contém `"analysis"` com `"analyzerVersion":1`.
- [ ] **Step 2: Rodar:** `cargo test -p sp404-core --test scan --test runner` → FAIL.
- [ ] **Step 3: Implementar.** Reorganizar `plan_pack` em duas fases (coletar → nomear) sem mudar o resultado quando a detecção está desligada.
- [ ] **Step 4: Rodar:** `cargo test -p sp404-core` → PASS (todos, inclusive os antigos de `scan`).
- [ ] **Step 5: Commit** — `feat(core): analyze audio before naming when key/BPM detection is on`.

### Task 11: CLI — flags, `analyze` e métricas

**Files:**
- Create: `cli/src/eval.rs`
- Modify: `cli/src/main.rs`
- Test: `cli/tests/` (arquivo existente) e testes unitários em `eval.rs`

**Interfaces:**
- Consumes: `plan_with`, `AnalysisCache`, `analyze_file`, `labeling::resolve`, `BpmRange: FromStr`.
- Produces:
  - `sp404 convert` ganha `--detect-key-bpm` e `--bpm-range <58-115|68-135|78-166|88-175|98-195>`, que sobrescrevem as configurações. O Dry Run imprime o nome final e `  (key uncertain)`/`(bpm uncertain)`/`(type uncertain)`/`(not analyzed: <erro>)`. O resumo final ganha `N with uncertain key/BPM`.
  - `sp404 analyze <inputs>... [--bpm-range R] [--ignore-names] [--labels <csv>] [--report <md>]`: para cada arquivo de áudio, uma linha `caminho  tipo  rótulo  bpm  [incertezas]  (name|audio)`. `--ignore-names` analisa com `NameHints` vazio e `NameTokens::default()`. Pistas de pasta para arquivos dentro de uma pasta de entrada: nome da pasta de entrada + subpastas (igual ao `scan`).
  - Gabarito CSV (cabeçalho obrigatório): `path,kind,label,bpm`; `path` relativo à pasta de entrada; `kind` ∈ `unpitched|note|chord|phrase|melodicLoop|drumLoop|` (vazio = não avaliar); `label` e `bpm` podem ficar vazios.
  - `eval::Metrics` com, por medida, `(acertos, total)`: tipo (com matriz de confusão), BPM exato (±1 após `fold_bpm` dos dois lados) e erros de metade/dobro, tom exato, tom exato-ou-relativo, pontuação MIREX ponderada (exato 1, quinta 0,5, relativo 0,3, paralelo 0,2), nota exata, acorde exato, acorde só fundamental, bateria sem prefixo, fração dos erros marcados como incertos, tempo médio por arquivo. `--report` grava essas tabelas em Markdown.
  - `eval::parse_labels(text: &str) -> Result<Vec<Label>, String>`; `eval::score(results: &[(Label, Analysis)]) -> Metrics`.

- [ ] **Step 1: Testes que falham:**
  - `eval::mirex_weights`: (`Am` vs `Am`) 1; (`Am` vs `Em`) 0,5; (`Am` vs `C`) 0,3; (`Am` vs `A`) 0,2; (`Am` vs `F#m`) 0.
  - `eval::bpm_octave_errors_counted`: gabarito 90, resultado 180 na faixa `98-195` (dobra para 180) → não conta como exato, conta como erro de metade/dobro.
  - `eval::parse_labels_rejects_bad_kind`.
  - CLI: `sp404 analyze <dir com seno 220 Hz em One Shots/x.wav>` imprime `note` e `A`; `sp404 convert <dir> --out <out> --dry-run --detect-key-bpm` imprime `A_x.wav`.
- [ ] **Step 2: Rodar:** `cargo test -p sp404-cli` → FAIL.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar:** PASS.
- [ ] **Step 5: Commit** — `feat(cli): key/BPM flags and an analyze command with accuracy metrics`.

### Task 12: App — configurações, análise com progresso e marcas de incerteza

**Files:**
- Modify: `app/src-tauri/src/main.rs`, `app/src/api.ts`, `app/src/main.ts`, `app/src/i18n.ts`, `app/index.html`

**Interfaces:**
- Consumes: `plan_with`, `PlanProgress`, `AnalysisCache`, `Analysis`, `Settings.detect_key_bpm`/`bpm_range`.
- Produces:
  - `AppState` ganha `cache: Arc<AnalysisCache>`.
  - `plan_conversion(paths, output_dir, on_event: Channel<PlanProgress>, state) -> Result<Plan, String>` passa a ser `async`, adquire o `RunGuard` (uma operação por vez), zera o `cancel`, roda `plan_with` em `spawn_blocking` e devolve `"cancelled"` como erro quando cancelado. `cancel_conversion` também cancela a análise.
  - `api.ts`: `Settings.detectKeyBpm: boolean`, `Settings.bpmRange: "58-115" | "68-135" | "78-166" | "88-175" | "98-195"`, `Analysis`, `Job.analysis?: Analysis`, `FileResult.analysis?: Analysis`, `PlanEvent = { event: "analyzing"; done: number; total: number }`, `planConversion(paths, outputDir, onEvent)`.

- [ ] **Step 1: Configurações.** Em "Nomes e pastas": checkbox `detectKeyBpm` e seletor `bpmRange` (habilitado só com a detecção ligada). Textos:
  - `sDetectKeyBpm`: "Detect key and BPM and add them to the start of file names" / "Detectar tom e BPM e incluir no início do nome"
  - `sBpmRange`: "BPM range" / "Faixa de BPM"
- [ ] **Step 2: Planejamento com progresso.** `dryRun()` e `convertAll()` marcam `busy` antes do primeiro `await`, mostram a barra com `t("analyzing", { done, total })` enquanto chegam eventos e deixam o botão Cancelar ativo. Cancelado → mensagem `t("analysisCancelled")`, nada gravado. Textos:
  - `analyzing`: "Analyzing {done} / {total}…" / "Analisando {done} / {total}…"
  - `analysisCancelled`: "Analysis cancelled. Nothing was written." / "Análise cancelada. Nada foi gravado."
- [ ] **Step 3: Dry Run e resumo.** Linhas com `analysis` incerto ganham observações no estilo de `willSkip`; resumo ganha `t("uncertainCount", { count })` com a lista dos arquivos. Textos:
  - `keyUncertain`: "key uncertain" / "tom incerto"; `bpmUncertain`: "BPM uncertain" / "BPM incerto"; `typeUncertain`: "type uncertain" / "tipo incerto"; `notAnalyzed`: "not analyzed: {reason}" / "não analisado: {reason}"
  - `uncertainCount`: "{count} files with uncertain key/BPM — worth checking by ear" / "{count} arquivos com tom/BPM incerto — vale conferir de ouvido"
- [ ] **Step 4: Verificar:** `npm run build --prefix app && cargo test --workspace` → PASS. No app, com a detecção ligada, Dry Run de `~/Desktop/DAYDREAMS SAMPLE PACK`: a barra mostra "Analisando…", Cancelar no meio mostra `analysisCancelled` e um novo Dry Run funciona; Dry Run completo mostra nomes como `90_Breaks_Fly.wav` e `85_Breaks_Lost.wav`; Converter logo depois não reanalisa (a barra de análise passa direto). Com a detecção desligada, os nomes são os de hoje.
- [ ] **Step 5: Commit** — `feat(app): key/BPM settings, analysis progress and uncertainty notes`.

### Task 13: Calibração e portão de precisão

Esta task decide se a Parte B pode ser liberada. Ela não termina com "testes passam": termina com as metas atingidas e registradas, ou com o autor decidindo o que fazer.

**Files:**
- Create: `scripts/reference_labels.py`, `docs/superpowers/notes/daydreams-labels.csv`, `docs/superpowers/notes/fsld-labels.csv` (se o dataset for usado), `docs/superpowers/notes/2026-MM-DD-key-bpm-accuracy.md`
- Modify: limiares/perfis em `core/src/analyze/*`, `ANALYZER_VERSION` se mudar algo depois da primeira medição registrada

- [ ] **Step 1: Gabarito do DAYDREAMS a partir dos nomes e pastas.** Escrever `docs/superpowers/notes/daydreams-labels.csv` com `kind` pela pasta (`Drum Breaks`, `Percussion Loops` → `drumLoop`; `Sample Loops` → `melodicLoop`; `One Shots/*` → `unpitched`, exceto `Snare_Tambo 1/2 (85bpm)` → `drumLoop`; `One Shots (Melodic)` → `note`/`chord`/`phrase` deixados vazios por enquanto) e `bpm` pelo nome (`Shells.wav` vazio).
- [ ] **Step 2: Referência para tom, nota e acorde.** `scripts/reference_labels.py <pasta> <csv-entrada> <csv-saída>` (Python 3, `pip install essentia` num venv fora do repo; o script não é importado por nada do app): preenche `label` vazio com `KeyExtractor` (perfil `edma`) para loops, nota por `PitchYinProbabilistic` (mediana) e acorde por `ChordsDetection` para one-shots melódicos; marca a origem numa coluna extra `source=reference`.
- [ ] **Step 3: Medir:** `cargo run -p sp404-cli -- analyze "$HOME/Desktop/DAYDREAMS SAMPLE PACK" --ignore-names --labels docs/superpowers/notes/daydreams-labels.csv --report /tmp/daydreams-report.md`, e de novo sem `--ignore-names`.
- [ ] **Step 4: Conferência de ouvido pelo autor.** Gerar a lista dos arquivos em que o app e a referência discordam (tom, nota, acorde, tipo dos `One Shots (Melodic)`) e pedir ao autor que confira de ouvido **só esses**. Corrigir o CSV com as respostas (coluna `source=author`). Commitar o CSV (sem áudio).
- [ ] **Step 5: Freesound Loop Dataset** (opcional, recomendado para tom/BPM em volume): baixar do Zenodo (registro 3967852) para uma pasta fora do repo, converter as anotações em `fsld-labels.csv` (só caminho/tom/BPM/tipo) e medir com `--ignore-names`.
- [ ] **Step 6: Calibrar** limiares de classificação, desempate do relativo, conjunto de perfis de tom (Krumhansl-Kessler, Temperley, perfil EDM) e limiares de confiança. Cada mudança é seguida de `cargo test --workspace` (os testes sintéticos não podem quebrar) e de nova medição.
- [ ] **Step 7: Comparar com as metas** da spec, seção 6 (BPM ≥ 90%; tom exato ≥ 65%; exato-ou-relativo ≥ 80%; nota ≥ 95%; acorde ≥ 70% — nos sintéticos se houver menos de 20 acordes reais; bateria sem prefixo ≥ 98%; classificação ≥ 90%; erros marcados como incertos ≥ 60%; 500 arquivos < 1 min com 8 workers no Apple Silicon). Registrar tudo em `docs/superpowers/notes/<data>-key-bpm-accuracy.md` (conjunto, versão do analisador, tabela de métricas, comando usado).
- [ ] **Step 8: Se alguma meta falhar** depois da calibração: avaliar o plano B (`stratum-dsp` 1.0, MIT/Apache) só para tom e/ou BPM de loops, num branch separado, medindo com o mesmo comando. Se ainda falhar, **parar e levar os números ao autor** (decisão dele: a Parte B não é liberada sem as metas).
- [ ] **Step 9: Commit** — `test: key/BPM accuracy calibration on real loops`.

### Task 14: Documentação e liberação

**Files:**
- Modify: `PRODUCT.md`, `README.md`

- [ ] **Step 1:** `PRODUCT.md`: mover arquivos soltos e tom/BPM de "Planejado" para "Implementado", com o formato do prefixo e o padrão desligado. `README.md`: uso de `--detect-key-bpm`, `--bpm-range` e `sp404 analyze`.
- [ ] **Step 2:** Conferir que `docs/superpowers/notes/*-key-bpm-accuracy.md` mostra todas as metas atingidas antes de qualquer release que inclua a Parte B.
- [ ] **Step 3: Commit** — `docs: loose files and key/BPM naming`.

# Tom, BPM e arquivos soltos — Design

**Data:** 2026-10-19
**Status:** Revisado (builder check), aguardando aprovação do autor
**Base:** v0.1.0 (`docs/superpowers/specs/2026-10-14-sp404-converter-design.md`)

Este documento cobre duas mudanças independentes, que podem ser entregues separadamente:

- **Parte A — Arquivos soltos:** aceitar arquivos de áudio avulsos, além de pastas.
- **Parte B — Tom e BPM no nome:** analisar o som de cada arquivo e colocar nota, acorde, tom e/ou BPM no início do nome de saída.

A Parte A vem primeiro no plano: é menor, e a Parte B também passa a valer para arquivos soltos.

Convenção: o que o autor decidiu em conversa está escrito como regra. O que é escolha do designer está marcado **(decisão do designer)** e pode ser revisto.

---

## 1. Por que

O autor usa a SP-404 MKII para fazer beats e enfrenta dois problemas que hoje resolve de ouvido:

1. **One-shots de instrumento no Chromatic Mode.** Para tocar um one-shot (baixo, piano, synth, 808) cromaticamente em harmonia com o beat, é preciso saber em que nota ele está. Se for um acorde, é preciso saber qual acorde.
2. **Loops, melodias e músicas inteiras que serão recortadas.** É preciso saber a escala (tom) e o BPM. A detecção de BPM da SP-404 às vezes erra, e o aparelho não detecta tom.

A tela da SP-404 mostra só parte do nome do arquivo, então a informação precisa vir **no começo do nome**, para ser vista primeiro.

Sucesso: arrastar um pack (ou alguns arquivos), converter, e na SP-404 já ler `Am_90_Melody_Loop` ou `C_808_Long` sem precisar descobrir de ouvido. Na maioria dos arquivos o palpite deve estar certo, e o app deve dizer quais arquivos ele não tem certeza.

## 2. Objetivos, não-objetivos e anti-objetivos

**Objetivos**
- Arquivos soltos entram no app como pastas entram hoje (arrastar ou procurar).
- Opção para detectar e incluir no nome: nota (one-shot de nota), acorde (one-shot de acorde), tom + BPM (loop melódico, trecho de música) e BPM (loop de bateria).
- Se o nome original já traz tom ou BPM, essa informação é **movida para o início** e reescrita na notação padrão.
- Tudo aparece no Dry Run antes de gravar.

**Não-objetivos (por enquanto)**
- Gravar tom/BPM em metadados (chunks `acid`, `smpl`, ID3). A saída continua só com `fmt ` e `data`.
- Corrigir afinação, esticar tempo ou cortar loops.
- Notação Camelot (`8A`) e nomes de tom em português (`Lá menor`).
- Detectar tom ou BPM para nomes de pastas.
- Análise "por pasta" (um único tom para uma pasta inteira).

**Anti-objetivos (seria um fracasso mesmo funcionando)**
- Qualquer mudança no comportamento quando a opção está desligada. Com a detecção desligada, o Plano e os nomes devem ser idênticos aos da v0.1.0.
- Colocar nota ou tom em one-shots de bateria (kick, snare, hat…). Um `C_Kick` é pior do que nada.
- Nomes que a SP-404 não importa. O prefixo usa só letras ASCII, dígitos e `_`.
- Nomes que mudam entre duas execuções nos mesmos arquivos. Isso quebraria "pular existentes" e o desfazer. A análise tem que ser determinística.
- Trazer FFmpeg, código GPL/AGPL ou modelos de machine learning para o app distribuído. O projeto é MIT.

---

## 3. Parte A — Arquivos soltos

### Comportamento

- A janela aceita arquivos e pastas no mesmo arrastar. Cada pasta continua sendo um pack.
- Todos os arquivos soltos de um lote formam **um grupo só**, mostrado na lista como uma entrada "Arquivos soltos (N)", que pode ser removida como um pack.
- **Sempre que há arquivos soltos, o app pergunta a pasta de saída**, qualquer que seja o modo de destino nas configurações. As pastas (packs) do mesmo lote continuam seguindo o modo configurado.
- Os arquivos soltos vão **sempre para uma pasta nova criada pelo app** dentro da pasta escolhida, mesmo que a pasta escolhida seja a mesma dos originais. Nome da pasta: `Samples [SP-404]` **(decisão do designer)**. Se ela já existe (segunda execução), é reutilizada, e "pular existentes" vale normalmente.
- Dentro de `Samples [SP-404]` não há subpastas: arquivos soltos de pastas de origem diferentes ficam lado a lado, e nomes repetidos recebem `_2`, `_3` pela regra de colisão que já existe.
- A regra de segurança atual continua: nenhum arquivo de origem pode estar dentro da pasta de saída (por exemplo, arrastar arquivos que estão dentro de um `Samples [SP-404]` anterior e escolher a mesma pasta-mãe é recusado com a mensagem de sobreposição já existente).
- Só arquivos de áudio entram como soltos. Arrastar um PDF ou imagem solto mostra a mensagem "não é um arquivo de áudio" e o arquivo não entra **(decisão do designer)**. Áudio não suportado (Opus, WMA…) entra e aparece como não suportado, como nos packs.
- **Procurar:** o seletor de pastas não deixa escolher arquivos (e vice-versa) em todos os sistemas, então há dois botões, "Adicionar pastas…" (Cmd/Ctrl+O, como hoje) e "Adicionar arquivos…" (Cmd/Ctrl+Shift+O) **(decisão do designer)**. O seletor de arquivos filtra pelas extensões de áudio suportadas.
- Os demais ajustes (limpar nomes, tamanho máximo, log JSON, desfazer, espaço livre, cancelar) valem para arquivos soltos igual a packs. "Achatar pastas" não muda nada para eles, porque já não têm subpastas.
- A CLI aceita arquivos soltos nos mesmos argumentos posicionais e exige `--out` quando há algum.

### Onde mora

- `core/src/scan.rs`: `build_plan` passa a receber caminhos que podem ser pastas ou arquivos. Os arquivos viram um `PackPlan` de origem "solta" (sem `source_root` único; cada `Job.source` é o caminho do arquivo). O destino dele é sempre `<output_dir>/Samples [SP-404]`, e falta de `output_dir` gera `NoOutputDir`. A checagem de sobreposição trata cada arquivo solto como uma origem.
- `inspect_pack` ganha uma irmã para arquivos soltos (ou passa a aceitar os dois) para montar a entrada da lista.
- App: `buildPlan()` pergunta a pasta quando `saveMode === "ask"` **ou** quando há arquivos soltos. O builder decide como `PackPlan` representa a origem solta (por exemplo `source_root` vazio e um campo `loose: true`), desde que a UI e a CLI mostrem caminhos legíveis no Dry Run.

---

## 4. Parte B — Tom e BPM no nome

### 4.1 A opção

- Nova configuração **"Detectar tom e BPM e incluir no nome"**, na seção Nomes. **Desligada por padrão** **(decisão do designer)**: a análise deixa o planejamento bem mais lento e muda os nomes, e os padrões devem servir a quem nunca abriu as configurações.
- Nova configuração **"Faixa de BPM"**, um seletor com faixas fixas, como no Serato e na SP-404: `58–115`, `68–135`, `78–166`, `88–175`, `98–195`. Padrão **`78–166`** **(decisão do designer)**. Ela só aparece habilitada quando a detecção está ligada.
- Com a opção desligada, nada da Parte B roda e nada muda (anti-objetivo).

### 4.2 Tipos de arquivo e o que cada um recebe

Cada arquivo de áudio convertível é classificado em um destes tipos:

| Tipo | Exemplo de origem | Prefixo | Exemplo de saída |
|---|---|---|---|
| One-shot de bateria / sem nota | kick, snare, hat, FX sem altura | nenhum | `Kick_Hard.wav` |
| One-shot de nota | baixo, 808, piano, synth, vocal com nota | nota | `C_808_Long.wav` |
| One-shot de acorde | stab de piano, pad, acorde de guitarra | acorde | `Cm7_Keys_Stab.wav` |
| Frase curta com várias notas | arpejo, mallet roll, melodia de um compasso que não é loop | tom | `Am_OP1_ArpDelay.wav` |
| Loop melódico / trecho de música | melodia, sample de música, loop de baixo | tom + BPM | `Am_90_Melody_Loop.wav` |
| Loop de bateria | break, groove, top loop | BPM | `90_Funk_Break.wav` |

A classificação usa **as duas fontes**: pistas nos nomes de pastas e arquivos, e a análise do som, que dá a palavra final sobre o que é nota, acorde ou loop (ver 4.6).

### 4.3 Formato do prefixo

- O prefixo vem **no início** do nome, separado do resto por `_`: `<tom>_<bpm>_<resto>`.
- **Nota:** só a classe da nota, sem oitava, em letras: `C`, `Db`, `D`, `Eb`, `E`, `F`, `Gb`, `G`, `Ab`, `A`, `Bb`, `B`. A oitava não importa para o Chromatic Mode **(decisão do designer)**. One-shots um pouco desafinados recebem a nota mais próxima.
- **Bemóis em vez de sustenidos.** O manual do SP-404MK2 App diz que nomes de samples devem usar só letras e números de um byte, e `#` não é letra nem número. Bemóis cabem nessa regra. Se o autor testar `#` no aparelho e funcionar, trocar para sustenidos é uma mudança de uma tabela.
- **Tom:** tônica + `m` para menor; maior não tem sufixo: `Am`, `Bbm`, `C`, `Eb`. Notação curta escolhida pelo autor (`Dm`, não `Dmin`).
- **Acorde:** fundamental + qualidade, de um conjunto fechado: maior (`C`), menor (`Cm`), dominante com sétima (`C7`), maior com sétima (`Cmaj7`), menor com sétima (`Cm7`), diminuto (`Cdim`). Acordes fora do conjunto recebem o mais próximo dele **(decisão do designer)**.
- **BPM:** número inteiro, sem "bpm": `90`, `128`. Sempre dentro da faixa configurada (4.5).
- **Arquivo cujo nome vira só o prefixo** (ex.: `Am 90bpm.wav`): o nome fica `Am_90.wav`, sem `_` sobrando.
- **Ordem com as outras regras de nome:** (1) ler e remover do nome original o tom/BPM que já existir (4.4); (2) limpar o resto se "limpar nomes" estiver ligado; (3) se "achatar pastas" estiver ligado, juntar as pastas ao nome; (4) colocar o prefixo na frente de tudo; (5) aplicar tamanho máximo e colisões, como hoje. O tamanho máximo corta o fim do nome, então o prefixo sempre sobrevive.
- O prefixo usa só `[A-Za-z0-9]` e `_`, então funciona igual com a limpeza de nomes ligada ou desligada. Com a limpeza desligada, o resto do nome mantém espaços: `Am_90_Melody Loop.wav`.
- Pastas e arquivos que não são áudio nunca recebem prefixo.

### 4.4 Quando o nome original já traz a informação

O autor decidiu: **mover para o início**, na notação padrão. Exemplos:

| Origem | Saída |
|---|---|
| `Loop_90bpm_A minor.wav` | `Am_90_Loop.wav` |
| `Keys F#m 140 BPM.wav` | `Gbm_140_Keys.wav` |
| `808 C#.wav` | `Db_808.wav` |
| `Break 92bpm.wav` | `92_Break.wav` |

Regras:

- A informação do nome **vale mais que a detecção**: é do autor do pack **(decisão do designer)**. Só o que falta é detectado. Ex.: um loop com BPM no nome mas sem tom tem só o tom detectado.
- Mesmo com informação no nome, o arquivo ainda é classificado. A leitura do nome roda **antes** da classificação e entra nela como pista: tom no nome puxa para "com nota", BPM no nome puxa para "loop". Depois da classificação, só os valores que o tipo usa (4.2) são movidos para o prefixo; os outros ficam onde estavam no nome **(decisão do designer)**. Exemplos: `Break Am 92bpm` classificado como loop de bateria → `92_Break_Am`; `Stab 90bpm` classificado como one-shot de acorde → `Cm_Stab_90bpm`; `Kick C` classificado como bateria → `Kick_C`.
- **BPM com marcador** (`90bpm`, `90 BPM`, `bpm90`, `90_BPM`) é sempre aceito.
- Parênteses e colchetes também separam palavras: `Breaks_Fly (90bpm)` → `90_Breaks_Fly`. Parênteses que ficarem vazios são removidos.
- **Tom com marcador ou acidente** (`Am`, `A min`, `A minor`, `Amin`, `Amaj`, `A major`, `F#m`, `Gb`, `C#`) é aceito quando é uma palavra separada (por espaço, `_`, `-`, `.` ou começo/fim).
- **Números e letras soltos são ambíguos** (`Kick 01`, `Take A`). Um número solto só é aceito como BPM, e uma letra solta (`A`–`G`) só é aceita como tom, se a análise do som concordar: o BPM detectado bate com o número (com tolerância de ±3% ou como metade/dobro), ou o tom detectado tem a mesma tônica (ou é o relativo). Quando concorda, vale o valor do nome **(decisão do designer)**.
- O token reconhecido é removido do nome junto com separadores que sobrarem, e o resto segue as regras de 4.3.
- Sustenidos do nome viram bemóis (`F#m` → `Gbm`). Tons escritos como menor/maior por extenso viram a notação curta.

### 4.5 BPM

- O resultado sempre cai dentro da faixa configurada: o detector escolhe entre o valor bruto, a metade e o dobro (e, se preciso, ×4 ou ÷4) o que fica dentro da faixa. Isso resolve o erro clássico de metade/dobro do mesmo jeito que o Serato e a SP-404.
- BPM de nome que cai fora da faixa é dobrado ou dividido para entrar nela. Ex.: na faixa `78–166`, `Break 70bpm` vira `140_Break` e `Breaks_Squash ME (192bpm)` vira `96_Breaks_Squash_ME`. **(decisão do designer)**
- Arredondado para inteiro.

### 4.6 Como a análise funciona

Tudo implementado em Rust dentro do `core`, num módulo novo (ex.: `core/src/analyze.rs`, dividido em submódulos se crescer). Não entra nenhuma biblioteca de análise musical **(decisão do designer, ver 4.10)**. A única dependência nova direta é `realfft` (MIT), que já está na árvore por causa do `rubato`. Detalhes numéricos (tamanhos de janela, limiares, pesos) ficam com o builder e são ajustados pelo teste de precisão (seção 6). O que é fixo:

**Entrada da análise**
- O decodificador atual (symphonia) lê o arquivo. Os canais são somados em mono e o sinal é reamostrado para uma taxa fixa de análise (ex.: 22 050 Hz), independente da taxa de origem, para que o resultado não dependa do formato de origem.
- Só os primeiros **3 minutos** são analisados **(decisão do designer)**. O decode deve parar aí, em vez de decodificar o arquivo inteiro e descartar o resto.
- Silêncio no começo e no fim é ignorado para medir a duração e a altura.

**Características medidas**
- *Envelope de ataques* (spectral flux) e lista de ataques.
- *Periodicidade:* autocorrelação do envelope de ataques.
- *Cromagrama:* energia por classe de nota (12 valores) ao longo do tempo, com estimativa de afinação global (amostras afinadas em 440 Hz ± até meio semitom) e uma versão só dos graves (aprox. 30–250 Hz).
- *Altura fundamental (f0)* para one-shots: método no domínio do tempo do tipo YIN/McLeod, que funciona em graves (808, baixo a partir de ~30 Hz), onde a resolução de FFT não basta.
- *Quanto o som tem nota definida:* periodicidade da f0 e concentração do cromagrama.

**Classificação**
1. *One-shot ou loop:* one-shot tem um ataque dominante (os demais bem mais fracos) ou é curto e sem ataques periódicos. Loop tem vários ataques com periodicidade.
2. *Com nota ou sem nota:* sem nota definida → bateria/sem prefixo.
3. *One-shot com nota:* uma nota estável → **nota**; três ou mais classes de nota soando juntas e estáveis → **acorde**; notas diferentes em sequência (arpejo, roll, frase) → **frase curta**, que recebe só o tom **(decisão do designer, a partir do pack de teste)**.
4. *Loop:* com conteúdo tonal suficiente → **loop melódico** (tom + BPM); sem → **loop de bateria** (BPM).
5. *Pistas de nome* (palavras em qualquer pasta do caminho ou no nome do arquivo, sem diferenciar maiúsculas) puxam a decisão quando o som é ambíguo. O som vence quando a evidência é forte. Listas iniciais (o builder pode ampliar com base no teste):
   - bateria: `kick`, `kicks`, `snare`, `hat`, `hats`, `hihat`, `hi-hat`, `clap`, `perc`, `percussion`, `rim`, `tom`, `cymbal`, `crash`, `ride`, `shaker`, `drum`, `drums`, `break`, `breaks`, `groove`, `top`, `tops`, `fx`
   - one-shot: `one shot`, `one-shot`, `oneshot`, `one shots`, `hit`, `hits`, `shot`, `stab`
   - loop: `loop`, `loops`
   - acorde: `chord`, `chords`
   - com nota: `808`, `bass`, `keys`, `piano`, `synth`, `pad`, `lead`, `guitar`, `melody`, `melodic`, `vocal`
   - `808` e `bass` dentro de pastas de bateria continuam sendo tratados como com nota.
   - Quais pastas contam: para packs, o nome da pasta do pack e as subpastas até o arquivo; para arquivos soltos, só a pasta onde o arquivo está. Pastas acima do pack (ex.: `~/Downloads`) não contam **(decisão do designer)**.

**Nota de one-shot:** mediana da f0 na parte sustentada (depois do ataque), convertida para a classe de nota mais próxima, considerando a afinação estimada. Em 808 com glide, vale a nota de chegada.

**Acorde de one-shot:** cromagrama da parte sustentada comparado com modelos dos acordes do conjunto (12 fundamentais × 6 qualidades). A fundamental é desempatada pelo cromagrama dos graves.

**Tom de loop ou música:** o cromagrama médio é comparado com perfis dos 24 tons (maior e menor). O teste de precisão escolhe o conjunto de perfis (por exemplo Krumhansl-Kessler, Temperley ou um perfil para música eletrônica). **Desempate maior × relativo menor** (o caso mais comum de erro), como combinado com o autor, quando os dois ficam próximos:
1. peso maior para a nota mais grave (cromagrama dos graves), que costuma marcar a tônica;
2. peso para as notas no começo e no fim do loop, onde a frase costuma repousar;
3. qual dos dois acordes (ex.: `Am` × `C`) aparece mais ao longo do loop.

**BPM de loop:**
- Loops de sample pack quase sempre têm um número inteiro de batidas (no pack de teste, 10 de 12 loops longos têm exatamente 64 batidas, e um tem 128). Os candidatos são `BPM = 60 × batidas / duração` para números inteiros de batidas, com preferência por compassos inteiros (múltiplos de 4) e depois por números pares. O candidato escolhido precisa concordar com a periodicidade dos ataques. Quando um candidato concorda e dá um BPM quase inteiro, a confiança é alta.
- Se nenhum candidato concorda (trecho de música não recortado no compasso), vale só a periodicidade dos ataques.
- Depois, o valor é encaixado na faixa (4.5).

**Confiança:** cada campo (classificação, nota/acorde/tom, BPM) recebe "confiante" ou "incerto", pela margem entre o melhor e o segundo candidato e pela concordância entre os métodos. Os limiares são calibrados no teste (seção 6) para que a maior parte dos erros caia em "incerto".

**Determinismo:** a mesma entrada dá sempre o mesmo resultado, com qualquer número de workers e em macOS e Windows. Sem aleatoriedade e sem dependência da ordem de execução.

**Estabilidade entre versões:** os nomes só precisam ser iguais entre execuções da **mesma versão** do app **(decisão do designer)**. Ajustar limiares numa versão futura pode mudar alguns nomes, e então "pular existentes" não pula esses arquivos (eles saem com o nome novo). Para isso ficar visível, o analisador tem um número de versão próprio, gravado no log JSON, e as notas de versão avisam quando ele muda.

### 4.7 Pouca confiança

O autor decidiu: **o melhor palpite vai para o nome mesmo assim**. O nome não muda conforme a confiança. No Dry Run, no resumo, na CLI e no log JSON, o arquivo aparece marcado ("tom incerto", "BPM incerto", "tipo incerto"), para quem quiser conferir esses de ouvido.

Arquivo que não pôde ser analisado (erro de decode) fica sem prefixo e com o motivo no Dry Run. A conversão em si segue como hoje (e provavelmente vai falhar pelo mesmo motivo, com o erro já existente).

### 4.8 Onde entra no fluxo

Hoje o Plano é montado em instantes, sem abrir os arquivos. Com a detecção ligada, montar o Plano passa a exigir decodificar e analisar cada arquivo de áudio, porque o Dry Run precisa mostrar os nomes finais. Então:

- `scan` monta a lista de arquivos como hoje. Com a detecção ligada, os arquivos convertíveis são analisados **em paralelo** (pool do rayon com o número de workers configurado) **antes** de os nomes serem decididos. O resolvedor de colisões continua rodando por último, em ordem, para que os nomes sejam determinísticos.
- O planejamento passa a **reportar progresso** ("Analisando 37 de 420…") e pode ser **cancelado** com o mesmo botão Cancelar e o mesmo controle de "uma operação por vez" que já existem para a conversão. Cancelar a análise não grava nada.
- **Cache de análise na memória** do app (decisão do designer): o resultado de cada arquivo é guardado pela combinação caminho + tamanho + data de modificação + faixa de BPM. Um Converter logo depois de um Dry Run não analisa tudo de novo. O cache some quando o app fecha.
- Cada `Job` de conversão ganha um campo opcional com o resultado (tipo, nota/acorde/tom, BPM, de onde veio cada valor — nome ou som — e as marcas de incerteza), que a UI, a CLI e o log JSON usam.
- O runner, o desfazer e a checagem de espaço não mudam.
- **Tempo:** objetivo de analisar um pack típico de 500 arquivos em menos de 1 minuto num Mac Apple Silicon com 8 workers **(decisão do designer; medir no teste)**. Arquivos longos seguem a regra atual de memória (um "pesado" por vez).

### 4.9 Interface e CLI

- Configurações: os dois controles de 4.1, em pt-BR e inglês.
- Dry Run: cada linha mostra o nome final. Arquivos marcados como incertos ganham uma observação discreta, como as de "vai pular".
- Resumo: um contador "N arquivos com tom/BPM incerto", com a lista.
- CLI: `sp404 convert` usa as configurações salvas, com `--detect-key-bpm` e `--bpm-range 78-166` para sobrescrever. Novo comando `sp404 analyze <arquivos ou pastas>` que só mostra o que seria detectado (tipo, valores, confiança, de onde veio), sem converter. Ele é a base do teste de precisão (6).

### 4.10 Por que implementar em vez de usar uma biblioteca

Pesquisa feita em 2026-10-19:

| Opção | Licença | Situação | Por que não (por agora) |
|---|---|---|---|
| Essentia, libKeyFinder, aubio, bliss-audio | AGPL/GPL | maduras | licença incompatível com o objetivo (app MIT, possivelmente fechado no futuro) |
| `stratum-dsp` 1.0 | MIT/Apache-2.0 | ~28 mil downloads; tom por Krumhansl-Kessler, tempo por tempograma | feita para faixas inteiras de DJ; não detecta nota de one-shot, acorde nem classifica; mantida por uma pessoa |
| `bpm-key` 0.1 | MIT/Apache-2.0 | publicada há 2 semanas, ~100 downloads | jovem demais; usa symphonia 0.6 (o projeto fixa 0.5.5) |
| `pitch-detection` 0.3 | MIT/Apache-2.0 | sem atualização desde 2022 | só f0; YIN é pequeno o bastante para escrever no core |

O app precisa de coisas que nenhuma delas faz (nota e acorde de one-shot, classificação, BPM pelo tamanho do loop, desempate com graves). Implementar no core custa algumas centenas de linhas de DSP sobre `realfft`, sem dependências novas de fato, e fica sob controle do teste de precisão. **Plano B:** se o teste mostrar que o tom ou o BPM de loops não chega à meta, avaliar `stratum-dsp` só para essas duas partes antes de desistir da meta.

---

## 5. Restrições

- Saída de áudio inalterada (WAV PCM 16 bits / 48 kHz, só `fmt ` e `data`).
- `symphonia = "=0.5.5"` e `rubato = "=0.15.0"` continuam fixados.
- Sem FFmpeg, GPL/AGPL ou modelos de ML no app distribuído. FFmpeg segue permitido só em desenvolvimento para gerar fixtures.
- Mesma experiência no macOS e no Windows.
- Nenhum arquivo de áudio de terceiros com direitos de cópia é commitado no repositório.

## 6. Testes e precisão

### Testes automáticos (rodam no `cargo test`)

Sinais sintéticos gerados no próprio código de teste, sem arquivos de áudio novos no repositório:
- tons harmônicos em notas conhecidas, inclusive desafinados em ±30 cents, e uma 808 sintética (seno com decaimento e glide) em várias notas graves;
- tríades e acordes de sétima do conjunto de 4.3;
- clique ou bateria sintética em BPM conhecido, cortada em 1, 2 e 4 compassos, mais uma versão não cortada no compasso;
- progressões de acordes com baixo em tons conhecidos, inclusive pares relativos (`Am`/`C`) para o desempate;
- ruído e ataques sem altura, que devem sair sem prefixo.

E testes de regra:
- leitura e reescrita de nomes (tabela de 4.4, números e letras soltos só com concordância);
- montagem do nome em todas as combinações de limpar nomes, achatar pastas e tamanho máximo;
- encaixe de BPM na faixa;
- **com a detecção desligada, o Plano é idêntico ao atual** para os fixtures existentes;
- determinismo: mesma análise com 1 e com 8 workers;
- Parte A: grupo solto, pasta sempre perguntada, `Samples [SP-404]` criada e reutilizada, recusa de sobreposição, não-áudio recusado.

### Teste de precisão com loops reais (manual, fora do CI)

Usa `sp404 analyze` com um arquivo de gabarito (CSV: caminho, tipo, tom/nota/acorde, BPM) ou com a opção de tirar o gabarito do próprio nome.

Conjuntos:
0. **DAYDREAMS SAMPLE PACK** (fornecido pelo autor, fica fora do repositório, em `~/Desktop/DAYDREAMS SAMPLE PACK`): 96 arquivos de áudio — 10 drum breaks e 10 loops de percussão (BPM no nome, exceto `Shells.wav`), 12 sample loops melódicos de 23 a 91 s (BPM no nome), 52 one-shots de bateria e FX (hats, kicks, rims, claps, snares, sirenes) e 12 one-shots melódicos (notas, 2 acordes de piano, arpejos e frases). Os nomes dão o gabarito de BPM e de tipo pela pasta; **não há tom em nenhum nome**. Casos que o teste deve cobrir: `Snare_Tambo 1 (85bpm)` dentro de `One Shots/Snare` (provável loop de pandeiro numa pasta de one-shots) e as sirenes de `SFX`, com altura que desliza e que não devem receber nota.
1. **Packs do autor.** Muitos já trazem tom e BPM no nome: o leitor de nomes de 4.4 gera o gabarito, e a análise roda com a leitura de nomes desligada (`--ignore-names`). Os one-shots com nota no nome servem para a nota.
2. **Freesound Loop Dataset** (9 455 loops com tom e tempo anotados por especialistas, licenças Creative Commons, Zenodo). Baixado localmente, nunca commitado.
3. **Gabarito de tom, nota e acorde onde os nomes não dizem nada:** um script só de desenvolvimento roda uma ferramenta de referência (ex.: Essentia via Python; GPL/AGPL não é problema porque nunca entra no app nem no repositório como dependência) e grava um CSV de referência. O autor confere de ouvido **só os arquivos em que a referência e o app discordam**, e o CSV corrigido vira o gabarito (commitado, sem o áudio).
4. Acordes: o pack de teste tem só 2. A meta de acordes é medida nos acordes sintéticos dos testes automáticos mais os acordes de outros packs do autor, quando houver; com menos de 20 acordes reais, o resultado é relatado, mas a meta vale só para os sintéticos.

Medidas: BPM exato (±1 após encaixe na faixa) e erros de metade/dobro; tom exato, relativo, quinta e paralelo (pontuação ponderada do MIREX); nota exata; acorde exato e só fundamental; matriz de confusão da classificação; fração dos erros marcados como "incerto"; tempo por arquivo.

**Metas para liberar a Parte B.** O autor decidiu: **a Parte B só é liberada quando as metas forem atingidas** (nada de lançar com a opção escondida ou pela metade). Se não forem atingidas depois de ajustar o detector próprio, o próximo passo é testar o plano B (4.10); se ainda assim não forem, a Parte B fica parada e o autor decide. A Parte A não depende disso. Valores **(decisão do designer)**:

| Medida | Meta |
|---|---|
| BPM de loops recortados, exato na faixa | ≥ 90% |
| Tom de loops, exato | ≥ 65% |
| Tom de loops, exato ou relativo | ≥ 80% |
| Nota de one-shot de nota | ≥ 95% |
| Acorde de one-shot de acorde (fundamental + qualidade) | ≥ 70% |
| Classificação por tipo (DAYDREAMS, gabarito pela pasta) | ≥ 90% |
| One-shots de bateria sem prefixo | ≥ 98% |
| Erros marcados como "incerto" | ≥ 60% dos erros |
| 500 arquivos típicos | < 1 min (Apple Silicon, 8 workers) |

Os números de cada execução ficam registrados em `docs/superpowers/notes/` com a data e o conjunto usado.

## 7. O que fica com o builder

- Tamanhos de janela, taxas, limiares, pesos dos desempates e conjunto de perfis de tom, sempre escolhidos pelo teste de precisão.
- Divisão do módulo de análise em arquivos e os nomes das funções e tipos.
- A forma exata de representar o grupo solto no `PackPlan` e o resultado da análise no `Job`, desde que serialize em camelCase como o resto.
- O texto exato das mensagens em pt-BR e inglês.
- Como mostrar o progresso da análise na UI, desde que use a barra de progresso existente e o botão Cancelar.

## 8. Decisões do designer para o autor conferir

1. Detecção **desligada** por padrão.
2. Faixa de BPM padrão `78–166`, com as opções `58–115`, `68–135`, `78–166`, `88–175`, `98–195`.
3. Nota sem oitava (`C`, não `C2`).
4. Conjunto de acordes: maior, `m`, `7`, `maj7`, `m7`, `dim`.
5. Informação do nome vale mais que a detecção; números e letras soltos só contam se o som concordar.
6. Pasta dos arquivos soltos: `Samples [SP-404]`, reutilizada se já existir.
7. Arquivos soltos que não são áudio são recusados.
8. Dois botões: "Adicionar pastas…" e "Adicionar arquivos…".
9. Análise só dos primeiros 3 minutos de cada arquivo.
10. Implementação própria no core, com `stratum-dsp` como plano B.
11. Metas de precisão da seção 6.
12. Tom/BPM do nome que o tipo não usa fica onde estava no nome.
13. Pistas de nome: pasta do pack e subpastas (packs) ou só a pasta do arquivo (soltos).
14. Nomes estáveis só dentro da mesma versão; versão do analisador no log JSON.
15. Frases curtas (arpejos, rolls) recebem só o tom.
16. Gabarito de tom feito com ferramenta de referência só de desenvolvimento + conferência de ouvido das discordâncias.

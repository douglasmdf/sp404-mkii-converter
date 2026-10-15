# SP-404 MKII Converter — Design

**Data:** 2026-10-14
**Status:** Aprovado em conversa, aguardando revisão da spec escrita

## 1. Objetivo

App de desktop (macOS e Windows) que converte em lote sample packs para o formato nativo da Roland SP-404 MKII, inspirado no MPC Converter (The Drum Broker). Primeira versão é um **conversor em lote** — sem organização por pads e sem edição de áudio.

Público: uso pessoal agora, distribuição pública depois. A estrutura deve permitir publicar sem reescrita (instaladores, assinatura, auto-update preparados mas desligados).

### Fora do escopo (v1)

- Organização de samples em bancos/pads
- Edição de áudio (normalizar, trim de silêncio, fades, mono forçado)
- Saída em 24 bits, AIFF ou outras taxas
- Leitura de Opus e WMA

## 2. Comportamento da conversão

### Entrada

- Usuário arrasta uma ou mais pastas para a janela, ou usa "Procurar". Cada pasta é um **pack**.
- Formatos lidos: WAV, AIFF, FLAC, MP3, M4A/AAC, OGG Vorbis — qualquer sample rate e bit depth (inclusive float).
- Arquivos que não são áudio (imagens, PDF, txt etc.) são copiados como estão.

### Saída de áudio

| Propriedade | Valor |
|---|---|
| Formato | WAV PCM (`fmt ` + `data`, nenhum outro chunk) |
| Bit depth | 16 bits, com dither TPDF quando a origem tem mais resolução |
| Sample rate | 48 kHz; resample sinc de alta qualidade quando a origem difere |
| Canais | Mono → mono; estéreo → estéreo; >2 canais → downmix estéreo (opção **ligada** por padrão; desligada = arquivo rejeitado) |
| Picos | Amostras acima de 0 dBFS após o processamento são limitadas (clamp) e o arquivo recebe aviso |
| Duração | Arquivos com mais de 16 minutos são convertidos, mas recebem aviso (limite por sample da SP-404 MKII) |

### Nomes e pastas

- **Sanitizar nomes de arquivo** (padrão: ligado): transliteração para ASCII (`Água` → `Agua`), espaços e caracteres especiais → `_`, `_` repetidos colapsados, nomes reservados do Windows (`CON`, `AUX`, `NUL`, `COM1`…) recebem sufixo `_`.
- **Sanitizar nomes de pastas** (padrão: desligado): mesma regra aplicada às subpastas.
- **Tamanho máximo do nome** (padrão: sem limite): trunca o nome (sem contar a extensão).
- **Colisões**: quando dois arquivos resultariam no mesmo caminho, o segundo recebe `_2`, o terceiro `_3` etc., respeitando o limite de tamanho.
- **Estrutura de pastas**: preservada por padrão (a SP-404 MKII navega bem em subpastas — confirmado pelo usuário).
- **Achatar pastas** (padrão: desligado): todos os arquivos vão para a raiz do pack, com o caminho incorporado ao nome (`Breaks/kick.wav` → `Breaks_kick.wav`).
- **Pasta de saída**: `<destino>/<nome do pack>/…`. Modos de destino:
  - Perguntar a cada vez (padrão)
  - Pasta fixa (definida nas configurações)
  - Ao lado da pasta de origem (`<pai da origem>/<nome do pack> [SP-404]/`)

### Recursos de fluxo

- **Dry Run**: mostra o Plano (origem → destino, avisos previstos) sem gravar nada.
- **Converter**: executa o Plano.
- **Pular existentes**: se o arquivo de destino já existe, ele é pulado — rodar de novo é seguro.
- **Desfazer**: apaga os arquivos criados pela última conversão (e, opcionalmente, pastas que ficaram vazias). Só apaga arquivos que constam no registro e cujo tamanho e data de modificação não mudaram.
- **Paralelismo**: número de workers configurável (padrão: número de núcleos lógicos, máx. 8).
- **Log JSON** (padrão: desligado): grava `conversion_results.json` na pasta de saída.

## 3. Arquitetura

### Tecnologia

- **Rust** para todo o processamento; **Tauri 2** para o app; interface em **TypeScript** com Vite, sem framework de UI (a interface é pequena).
- Bibliotecas: `symphonia` (decode), `rubato` (resample), `hound` (encode WAV), `rayon` (paralelismo), `serde`/`serde_json` (config/log), `deunicode` (transliteração ASCII).
- Sem FFmpeg: binário pequeno e sem preocupações de licença GPL/LGPL.

### Estrutura do repositório

```
sp404-converter/
├── core/   # crate de biblioteca: toda a lógica, sem UI
├── cli/    # binário de linha de comando sobre o core (testes, scripts)
└── app/    # app Tauri: frontend TS + comandos Rust que chamam o core
```

### Módulos do `core`

| Módulo | Responsabilidade | Depende de |
|---|---|---|
| `config` | Struct `Settings` com defaults; carregar/salvar JSON no diretório de config do SO | — |
| `naming` | Funções puras: sanitizar, truncar, achatar, resolver colisões | — |
| `scan` | Percorre os packs e produz um `Plan` (lista de `Job`: origem, destino, tipo áudio/cópia) | `naming`, `config` |
| `decode` | Abre qualquer formato suportado → `AudioBuffer { channels: Vec<Vec<f32>>, sample_rate }` | symphonia |
| `process` | Downmix, resample para 48 kHz, dither TPDF, clamp; retorna buffer + avisos | rubato |
| `encode` | Grava WAV 16/48 em arquivo temporário (`.part`) e renomeia atomicamente | hound |
| `runner` | Executa o `Plan` em paralelo, emite eventos de progresso, suporta cancelamento, grava o manifesto de undo | todos acima |
| `undo` | Lê o manifesto e remove com segurança os arquivos da última sessão | — |

### Fluxo de dados

```
pastas arrastadas → scan → Plan
                              ├─ Dry Run: UI exibe o Plan
                              └─ Converter: runner(Plan) → eventos → UI
                                               └─ manifesto (undo) + log JSON opcional
```

O `Plan` separa **decidir** de **fazer**: Dry Run é gratuito, undo sabe exatamente o que apagar, e a maior parte da lógica é testável sem áudio.

### Interface entre app e core

Comandos Tauri: `scan(paths, settings) -> Plan`, `convert(plan) -> SessionId` (com eventos `progress`/`file_done`/`finished`), `cancel()`, `undo(remove_empty_dirs)`, `get_settings()`, `save_settings(settings)`.

## 4. Tratamento de erros

- Erro em um arquivo nunca derruba o lote: o `Job` é marcado com o motivo (corrompido, formato não suportado, canais excedentes com downmix desligado, permissão negada, erro de I/O) e o lote continua.
- Avisos (clamp, duração > 16 min, downmix aplicado) não bloqueiam e aparecem no resumo.
- Antes de converter, verifica espaço livre no destino com estimativa baseada na duração (48 000 × 2 bytes × canais × segundos); se insuficiente, avisa e não inicia.
- Cancelar: workers terminam o arquivo atual, arquivos `.part` são removidos, manifesto registra o que foi concluído.

## 5. Interface do usuário

- **Janela principal**: área de soltar pastas, lista de packs (nome, nº de arquivos de áudio e de outros arquivos), botões Dry Run / Converter / Desfazer, barra de progresso, resumo final (convertidos, pulados, avisos, erros — expansível por arquivo).
- **Configurações**: todas as opções da seção 2, salvas automaticamente.
- **Atalhos**: ⌘/Ctrl+O procurar, ⌘/Ctrl+Enter converter, ⌘/Ctrl+, configurações.
- **Idiomas**: português e inglês, seguindo o idioma do sistema.

## 6. Testes

- **`naming`/`scan`**: testes unitários com acentos, emoji, nomes reservados do Windows, colisões, limite de tamanho, achatamento; árvore de pastas temporária (`tempfile`).
- **`process`**: senoide de 1 kHz em 44.1 kHz → 48 kHz mantém frequência (via contagem de cruzamentos por zero) e nível (±0.1 dB); dither não altera o nível RMS além do esperado; clamp limita corretamente.
- **Integração**: fixtures pequenas em WAV 24 bits, WAV float, AIFF, FLAC, MP3, M4A, OGG; mono, estéreo e 5.1 → conferir saída 16 bits / 48 kHz, canais corretos, apenas chunks `fmt ` e `data`; pular existentes; undo.
- **CI**: GitHub Actions roda os testes em macOS e Windows a cada push e gera os builds (macOS arm64 + x64, Windows x64).
- **Manual**: antes de cada versão, importar um pack convertido no SP-404 MKII via cartão SD.

## 7. Distribuição

- v1: instaladores não assinados (`.dmg`, `.msi`) gerados pelo CI.
- Preparado para depois: assinatura/notarização Apple, assinatura Windows e plugin de updater do Tauri — configuração presente, desligada.

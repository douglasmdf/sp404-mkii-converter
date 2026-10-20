# SP-404 MKII Converter

Converte sample packs inteiros para o formato nativo da Roland SP-404 MKII:
WAV 16 bits / 48 kHz, nomes limpos, estrutura de pastas preservada.
Funciona no macOS e no Windows.

## O que ele faz

- Lê WAV, AIFF, FLAC, MP3, M4A/AAC, OGG Vorbis e CAF (qualquer taxa e resolução).
- Converte para 48 kHz com resampler sinc de alta qualidade e reduz para 16 bits com dither TPDF.
  Arquivos que já estão em 16 bits / 48 kHz saem idênticos.
- Mono continua mono, estéreo continua estéreo; arquivos com mais canais são mixados para estéreo.
- Remove metadados, limpa nomes (ASCII), resolve nomes repetidos (`kick_2.wav`).
- Copia capas, PDFs e textos junto. Ignora lixo do sistema (`.DS_Store`, `__MACOSX`, `._arquivo`).
- Aceita também arquivos de áudio soltos (vão para `Samples [SP-404]` dentro da pasta de saída).
- Opcional (desligado por padrão): detecta tom, nota, acorde e BPM e coloca no começo do nome (`Am_90_Loop`, `C_808`, `Cm7_Stab`, `90_Break`). Tudo offline; resultados incertos ficam marcados no Dry Run e no resumo.
- Dry Run, Desfazer a última conversão, pular arquivos já convertidos, log JSON opcional.

Ao rodar de novo:

- Ligar a detecção de tom/BPM (ou uma versão futura do analisador) muda os nomes de saída, então "pular existentes" não pula os arquivos convertidos antes com os nomes antigos.
- Na pasta `Samples [SP-404]` reaproveitada, um arquivo solto cujo nome de saída já existe lá é pulado (aparece como "já existe, será pulado" no Dry Run); renomeie o original para convertê-lo.

## Usar

1. Abra o app e arraste as pastas dos packs para a janela (ou ⌘/Ctrl+O).
2. Clique em **Converter tudo** (⌘/Ctrl+Enter) e escolha onde salvar.
3. Copie a pasta gerada para o cartão SD e importe na SP-404 MKII.

Configurações: ⌘/Ctrl+, ou o botão ⚙.

## Desenvolvimento

Requisitos: Rust (stable), Node.js 22+.

```bash
npm install --prefix app
npm run build --prefix app
cargo test --workspace
```

Rodar o app em modo de desenvolvimento:

```bash
npm run tauri --prefix app dev
```

Linha de comando (útil para testes):

```bash
cargo run -p sp404-cli -- convert --out "$HOME/Desktop/SP404" "$HOME/Samples/Meu Pack"
```

`convert` aceita pastas e arquivos de áudio soltos. Para colocar tom e BPM nos nomes:

```bash
cargo run -p sp404-cli -- convert --detect-key-bpm --bpm-range 78-166 \
  --out "$HOME/Desktop/SP404" "$HOME/Samples/Meu Pack" "$HOME/Samples/loop.wav"
```

`convert` segue as configurações salvas pelo app (detecção de tom/BPM, faixa de BPM, modo de destino), a não ser que estas opções sejam passadas:

- `--detect-key-bpm` liga a detecção e `--no-detect-key-bpm` desliga, qualquer que seja a configuração salva.
- `--bpm-range` aceita 58-115, 68-135, 78-166, 88-175 ou 98-195. Sem ela, vale a faixa salva (78-166 quando não há nada salvo).
- `--out` escolhe a pasta de saída; é obrigatório quando há arquivos soltos.

Detectar tipo, tom e BPM sem converter, e medir a precisão contra um gabarito:

```bash
cargo run -p sp404-cli -- analyze --ignore-names \
  --labels gabarito.csv --report relatorio.md "$HOME/Samples/Meu Pack"
```

`--ignore-names` ignora os nomes de pastas e arquivos; `--labels` é um CSV `path,kind,label,bpm`; `--report` grava as tabelas de precisão em Markdown.

As fixtures de áudio em `core/tests/fixtures` são geradas com `core/tests/fixtures/generate.sh`
(precisa de `ffmpeg`, só em desenvolvimento).

Publicar uma versão: veja [docs/RELEASING.md](docs/RELEASING.md).

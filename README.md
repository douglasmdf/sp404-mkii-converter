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
- Dry Run, Desfazer a última conversão, pular arquivos já convertidos, log JSON opcional.

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

As fixtures de áudio em `core/tests/fixtures` são geradas com `core/tests/fixtures/generate.sh`
(precisa de `ffmpeg`, só em desenvolvimento).

Publicar uma versão: veja [docs/RELEASING.md](docs/RELEASING.md).

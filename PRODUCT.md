# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

App de desktop para macOS e Windows feito com Tauri 2: a interface é HTML/CSS/TypeScript renderizada no webview do sistema (WKWebView no macOS, WebView2 no Windows), não interface nativa do sistema.

## Users

Donos da Roland SP-404 MKII em geral: beatmakers e produtores de qualquer nível que compram ou baixam sample packs e querem levá-los para a SP-404 MKII sem complicação. A maioria não quer pensar em taxa de amostragem, profundidade de bits ou regras de nome de arquivo; quer colocar a pasta do pack e receber uma pasta pronta para o cartão SD.

Hoje o único usuário é o autor (uso pessoal); a intenção é tornar o produto público depois.

## Product Purpose

Converter em lote pastas inteiras de sample packs para o formato nativo da SP-404 MKII, para que sejam importadas do cartão SD sem problemas. Sucesso é: arrastar um pack, converter, copiar para o cartão e importar no aparelho sem erros, sem surpresas e sem danificar os arquivos originais.

## Positioning

- Feito só para a SP-404 MKII: gera exatamente o que o aparelho usa (WAV PCM, 16 bits, 48 kHz), sem opções que não se aplicam a ele.
- Roda no macOS e no Windows (a ferramenta de referência, o MPC Converter da The Drum Broker, é só para macOS e voltada para MPCs da Akai).
- Seguro com os originais: nunca sobrescreve arquivos de origem, recusa pastas de saída que se sobrepõem a um pack de origem, mostra tudo antes de gravar (Dry Run) e permite desfazer a última conversão.
- Gratuito.

## Operating Context

- Entrada: pastas de sample packs baixadas ou compradas pelo usuário, muitas vezes com subpastas (Breaks, Loops, One Shots), capas, PDFs e lixo de sistema (`.DS_Store`, `__MACOSX`, `._*`).
- Fluxo: arrastar pastas para a janela (ou Procurar) → Dry Run opcional → Converter tudo → escolher/confirmar a pasta de saída → resumo → copiar para o cartão SD → importar na SP-404 MKII.
- A SP-404 MKII navega bem por subpastas (confirmado pelo autor no aparelho), por isso a estrutura de pastas é preservada por padrão.
- Existe uma ferramenta de linha de comando `sp404` para testes e scripts; o app de desktop é a superfície do produto.

## Capabilities and Constraints

Implementado (v0.1.0):
- Lê WAV, AIFF, FLAC, MP3, M4A/AAC, OGG Vorbis e CAF em qualquer taxa/profundidade; Opus e WMA são marcados como não suportados.
- Saída: WAV PCM 16 bits / 48 kHz só com os blocos `fmt ` e `data`; resample sinc, dither TPDF, arquivos já em 16 bits/48 kHz saem idênticos; mono continua mono, estéreo continua estéreo, mais de 2 canais vira estéreo (opção).
- Avisos: picos acima de 0 dBFS limitados; arquivos com mais de 16 minutos (limite por sample da SP-404 MKII).
- Limpeza de nomes para ASCII, limpeza opcional de nomes de pastas, tamanho máximo opcional do nome, nomes repetidos resolvidos sem diferenciar maiúsculas (`_2`, `_3`), opção de achatar pastas.
- Arquivos que não são áudio são copiados junto; lixo de sistema é ignorado.
- Modos de destino: perguntar a cada vez, pasta fixa, ao lado da pasta de origem (`<pack> [SP-404]`).
- Dry Run, pular existentes ao rodar de novo, desfazer a última conversão (nunca apaga arquivos alterados depois da conversão), cancelar, conversão em paralelo, log JSON opcional, verificação de espaço livre.
- Interface em português do Brasil e inglês, seguindo o idioma do sistema. Atalhos: Cmd/Ctrl+O, Cmd/Ctrl+Enter, Cmd/Ctrl+,.
- Instaladores: `.dmg` (Apple Silicon e Intel) e `.exe` (NSIS, instalação por usuário). Ainda sem assinatura.

Implementado (próxima versão):
- Arquivos de áudio soltos: o app e o `sp404 convert` aceitam arquivos além de pastas. Os arquivos soltos vão para uma pasta `Samples [SP-404]` dentro da pasta de saída escolhida; com arquivos soltos, a pasta de saída é sempre perguntada. Na pasta `Samples [SP-404]` reaproveitada, um arquivo solto cujo nome de saída já existe lá é pulado (aparece como "já existe, será pulado" no Dry Run); renomeie o original para convertê-lo.
- Tom e BPM nos nomes (opção desligada por padrão): o analisador roda totalmente offline, em Rust, e detecta tom (com bemóis), nota, acorde e BPM. A informação entra no começo do nome do arquivo de saída: `Am_90_Loop`, `C_808`, `Cm7_Stab`, `90_Break`. O BPM é dobrado ou dividido para caber na faixa escolhida (58-115, 68-135, 78-166, 88-175 ou 98-195; padrão 78-166). Quando o nome original já traz tom/BPM, essa informação é movida para o começo. Resultados incertos aparecem marcados no Dry Run e no resumo. Ligar a detecção de tom/BPM (ou uma versão futura do analisador) muda os nomes de saída, então "pular existentes" não pula os arquivos convertidos antes com os nomes antigos.
- Linha de comando: `sp404 convert --detect-key-bpm --bpm-range` e `sp404 analyze` (mede a precisão contra um gabarito).

Fora do escopo por enquanto: organizar samples em bancos/pads da SP-404; edição de áudio (normalizar, cortar, fades); saídas diferentes de WAV 16 bits/48 kHz.

## Brand Commitments

- Nome: "SP-404 MKII Converter" (mantido por escolha do autor). "SP-404" é marca registrada da Roland; a distribuição pública precisa de um aviso de que o app não é afiliado nem endossado pela Roland.
- Idioma principal do autor e do primeiro público: português do Brasil; inglês é o segundo idioma da interface.

## Evidence on Hand

- App funcionando e release v0.1.0: https://github.com/douglasmdf/sp404-mkii-converter (repositório privado).
- Produto de referência instalado: `/Applications/MPC Converter.app` e o PDF de guia dele.
- Ícone do app: `app/app-icon.png` (grade 4×4 de pads com um pad laranja), gerado por `app/scripts/make_icon.py`.
- Não existem depoimentos, números de usuários, imprensa ou benchmarks; não inventar.

## Product Principles

1. Nunca colocar os originais do usuário em risco: mostrar antes de gravar, recusar destinos ambíguos, manter tudo desfazível.
2. A SP-404 MKII é o único alvo. Toda opção precisa fazer sentido para esse aparelho; o resto é ruído.
3. Os padrões devem servir para a maioria, para que alguém que usa pela primeira vez arraste um pack e converta sem abrir as configurações.
4. Relatar com honestidade: todo arquivo pulado, com erro ou alterado aparece com um motivo claro.
5. Mesma experiência no macOS e no Windows.

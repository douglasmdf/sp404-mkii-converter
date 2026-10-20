---
name: SP-404 MKII Converter
description: Sinalização de aeroporto que leva um sample pack até o cartão SD da SP-404 MKII.
colors:
  orange: "#ff6a13"
  orange-hover: "#ff8136"
  ink: "#111111"
  ink-muted: "rgba(17, 17, 17, 0.66)"
  black: "#0b0b0b"
  ground: "#18191b"
  panel: "#202124"
  raised: "#2a2c30"
  raised-hover: "#33363b"
  line: "#34363b"
  control-edge: "#3d4045"
  text: "#f1f1ee"
  text-2: "#a8aaaf"
  text-3: "#8e9197"
  sign-white: "#ffffff"
  warn: "#ffb547"
  error: "#ff5a4f"
typography:
  gate:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "56px"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "-0.02em"
    fontFeature: "tnum"
  numeral:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "36px"
    fontWeight: 700
    lineHeight: 1
    fontFeature: "tnum"
  headline:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "30px"
    fontWeight: 700
    lineHeight: 1.15
  step:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "20px"
    fontWeight: 400
    lineHeight: 1.4
  title:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "18px"
    fontWeight: 700
    lineHeight: 1.4
  body:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.4
  control:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "14px"
    fontWeight: 700
    lineHeight: 1
  label:
    fontFamily: "Atkinson Hyperlegible Next, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.4
  mono:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "12.5px"
    fontWeight: 400
    lineHeight: 1.45
rounded:
  sm: "3px"
  md: "4px"
  lg: "6px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  2xl: "24px"
  3xl: "28px"
components:
  button-primary:
    backgroundColor: "{colors.orange}"
    textColor: "{colors.ink}"
    typography: "{typography.control}"
    rounded: "{rounded.md}"
    padding: "0 14px 0 6px"
    height: "44px"
  button-primary-hover:
    backgroundColor: "{colors.orange-hover}"
  button-primary-disabled:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-3}"
  button-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    typography: "{typography.control}"
    rounded: "{rounded.md}"
    padding: "0 14px"
    height: "40px"
  button-secondary-hover:
    backgroundColor: "{colors.raised-hover}"
  button-link:
    textColor: "{colors.orange}"
    typography: "{typography.control}"
    padding: "4px 0"
  pictogram:
    backgroundColor: "{colors.black}"
    textColor: "{colors.sign-white}"
    rounded: "{rounded.md}"
    size: "44px"
  pictogram-xl:
    backgroundColor: "{colors.black}"
    textColor: "{colors.sign-white}"
    size: "96px"
  pictogram-button:
    backgroundColor: "{colors.black}"
    textColor: "{colors.sign-white}"
    rounded: "{rounded.md}"
    size: "48px"
  step-current:
    backgroundColor: "{colors.black}"
    textColor: "{colors.orange}"
    typography: "{typography.step}"
    padding: "0 20px"
  gate-panel:
    backgroundColor: "{colors.black}"
    textColor: "{colors.orange}"
    typography: "{typography.gate}"
    padding: "0 24px"
  tab:
    textColor: "{colors.text-2}"
    typography: "{typography.control}"
    padding: "0 12px"
    height: "40px"
  tab-selected:
    textColor: "{colors.text}"
  tag:
    backgroundColor: "{colors.black}"
    textColor: "{colors.orange}"
    rounded: "{rounded.sm}"
    padding: "1px 5px"
  input:
    backgroundColor: "{colors.ground}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "36px"
  dialog-band:
    backgroundColor: "{colors.orange}"
    textColor: "{colors.ink}"
    padding: "12px 12px 12px 24px"
---

# Design System: SP-404 MKII Converter

## Overview

**Creative North Star: "A Placa de Embarque"**

A janela é uma sinalização de aeroporto escura com uma única faixa de placa laranja. O laranja Roland é a cor da placa: marca o caminho, o passo atual e a ação principal, sempre com texto preto por cima. O preto de placa carrega os quadrados de pictograma e os números monumentais; os cinzas grafite formam o chão da janela, o trilho e os painéis. Nada flutua: tudo é placa, filete ou chão.

A densidade é de ferramenta, não de site. Uma janela de 1000x700 (mínimo 760x540) organizada em faixa de placa no topo, trilho de entradas à esquerda, área de trabalho à direita e barra de ações embaixo. A informação que importa aparece em escala de portão (o total no painel preto, os contadores do resumo); o detalhe fica numa tabela com abas, a um clique.

A tipografia é uma única humanista de sinalização, Atkinson Hyperlegible Next, em caixa normal, com números bold e tabulares. Os ícones vêm de uma única família (Lucide, um só traço), desenhados como pictogramas brancos em quadrados pretos; setas ficam presas à borda da área.

**Key Characteristics:**
- Laranja só para caminho, passo atual e ação principal; texto preto sobre laranja.
- Pictogramas brancos em quadrados pretos, uma família de ícones, um traço.
- Números bold e tabulares em escala de portão.
- Superfícies chapadas em grafite; profundidade por tom e filete, não por sombra.
- Cantos quase retos (4px); setas presas à borda com o canto colado reto.

## Colors

Um sistema de três cores pinado pela marca: laranja Roland, preto de placa e grafites, mais dois tons de estado.

### Primary
- **Laranja Placa Roland** (`orange`): a faixa de passos, a faixa do diálogo, o botão "Converter tudo", o véu de soltar arquivos, o filete da aba selecionada, o anel de foco, o cursor de texto, a seleção de texto e os números dentro de painéis pretos (portão, contagem das abas, tags de tom/BPM). Hover no laranja clareia para `orange-hover`.
- **Tinta de Placa** (`ink`): todo texto e contorno sobre laranja. Divisórias entre passos usam a tinta a 18% de opacidade.

### Neutral
- **Preto de Placa** (`black`): quadrados de pictograma, o painel do portão, o passo atual, a barra de progresso e o fundo das tags e contagens. Também é o filete inferior de 3px que emoldura a faixa.
- **Chão Grafite** (`ground`): fundo da janela, dos campos e do cabeçalho fixo da tabela.
- **Painel Grafite** (`panel`): trilho de entradas, barra de ações, diálogo, aviso e hover de linhas e abas.
- **Grafite Elevado** (`raised`, hover `raised-hover`): fundo dos botões secundários e do botão principal desativado.
- **Filete** (`line`): divisórias de 1px entre linhas, colunas de contadores, trilho e barra.
- **Borda de Controle** (`control-edge`): contorno de 1px dos botões secundários e dos campos.
- **Texto** (`text`), **Texto 2** (`text-2`) e **Texto 3** (`text-3`): texto principal; texto secundário (subtítulos, rótulos, origem na tabela); texto terciário (pastas, caminhos, dicas da barra, linhas puladas).
- **Branco de Pictograma** (`sign-white`): o glifo dentro do quadrado preto e o preenchimento da barra de progresso.

### Estado
- **Âmbar de Aviso** (`warn`) e **Vermelho de Erro** (`error`): só no número do contador, no ícone e na observação das linhas afetadas. O erro também pinta o quadrado de pictograma da mensagem de erro, com glifo preto.

### Named Rules
**The Sign Panel Rule.** O laranja é a placa: caminho, passo atual, ação principal e números dentro de painel preto. Nunca vira fundo decorativo, ícone solto em grafite ou cor de estado.

**The Black-On-Orange Rule.** Sobre laranja, o texto e os contornos são sempre `ink`, nunca branco.

**The Graphite Disable Rule.** O botão principal desativado vira grafite (`raised` com texto `text-3`), nunca um laranja esmaecido que fica marrom.

## Typography

**Display Font:** Atkinson Hyperlegible Next (com -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif), pesos 400 e 700, empacotada via @fontsource para o app funcionar offline.
**Body Font:** a mesma.
**Label/Mono Font:** ui-monospace (com SF Mono, Menlo, Consolas), só para nomes de saída, caminhos e a pasta fixa.

**Character:** uma única humanista de sinalização, legível de longe e de perto; a hierarquia vem de tamanho e peso, não de troca de família.

### Hierarchy
- **Gate** (700, 56px, 1, -0.02em, tabular): o número do portão no painel preto, em laranja. Abaixo de 900px cai para 40px.
- **Numeral** (700, 36px, 1, tabular): os contadores do resumo. Abaixo de 900px cai para 24px.
- **Headline** (700, 30px, 1.15, máx. 20ch, balanceado): o texto das placas de chegada e de trabalho em andamento.
- **Step** (400, 20px; 700 nos passos feitos e no atual): os rótulos da faixa de passos. Abaixo de 900px, 16px; abaixo de 960px só o passo atual mantém o texto.
- **Title** (700, 18–22px): títulos das linhas de rota (18px), do resumo (20px) e da faixa do diálogo (22px).
- **Body** (400, 15px, 1.4): texto base, nomes de pack no trilho, subtítulos de placa. Mensagens, campos e linhas de configuração usam 14px; a tabela, 13px.
- **Control** (700, 14px, 1): todos os botões e abas; o botão principal sobe para 15px.
- **Label** (400–700, 12px): rótulos dos contadores, cabeçalho da tabela (700), metadados do trilho, contagens das abas.
- **Mono** (400, 12.5px, 1.45): nome de saída na SP-404 e caminho do pack; 12px para caminhos e pasta fixa.

### Named Rules
**The Tabular Numbers Rule.** Todo número que muda (portão, contadores, contagens, metadados, campos numéricos) usa `font-variant-numeric: tabular-nums` e peso 700 quando é o dado principal.

**The Sentence Case Rule.** Nada em caixa alta forçada nem com tracking aberto; rótulos ficam em caixa normal.

## Layout

A janela é uma grade fixa de três linhas e duas colunas, sem rolagem do corpo: faixa (`--band-h` 112px) em toda a largura; trilho (`--rail-w` 264px) e área de trabalho; barra de ações (`--foot-h` 68px) em toda a largura. Só a lista de entradas e a tabela rolam por dentro.

A área de trabalho tem padding de 24px 28px e espaçamento de 20px entre blocos. As placas de chegada e de trabalho começam a 8vh do topo, com o quadrado de 96px colado na borda esquerda (margem negativa igual ao padding). A rota de configurações limita-se a 720px.

O ritmo usa passos de 4px (4, 8, 12, 16, 20, 24, 28). Abaixo de 960px os passos futuros ficam só com o pictograma. Abaixo de 900px a grade encolhe (trilho 220px, faixa 88px, barra 60px), o padding vai a 16px 20px, a coluna de observação da tabela some e a observação passa para baixo do nome de saída.

## Elevation & Depth

O sistema é chapado: a profundidade vem da troca de tom (`black` < `ground` < `panel` < `raised`) e de filetes de 1px. As únicas sombras estruturais são linhas internas, não elevação.

### Shadow Vocabulary
- **Filete da faixa** (`box-shadow: inset 0 -3px 0 var(--black)`): emoldura a borda inferior da faixa laranja.
- **Indicador de aba** (`box-shadow: inset 0 -3px 0 var(--orange)`): marca a aba selecionada.
- **Contorno de passo futuro** (`box-shadow: inset 0 0 0 2px var(--ink)`): o quadrado vazado dos passos ainda não alcançados.
- **Diálogo** (`box-shadow: 0 24px 64px rgba(0, 0, 0, 0.55)`, fundo `rgba(0, 0, 0, 0.62)`): a única elevação real, reservada ao diálogo modal.

### Named Rules
**The Flat Sign Rule.** Placas, painéis e botões não têm sombra. Elevação só existe no diálogo modal.

## Shapes

Cantos quase retos: 4px (`rounded.md`) em botões, campos, pictogramas, aviso e véu; 3px (`rounded.sm`) nas tags e contagens; 6px (`rounded.lg`) só no diálogo. Abas e passos não têm canto. Os quadrados presos à borda (seta da placa de chegada, seta do grupo na tabela) têm o lado colado reto e só os cantos externos arredondados (`0 4px 4px 0`).

## Components

### Buttons
Botões de sinalização: sólidos, compactos, ícone e texto em bold.
- **Shape:** cantos quase retos (4px), altura 40px, gap de 8px entre ícone (20px) e texto.
- **Primary ("Converter tudo"):** placa laranja com texto `ink`, 44px de altura, 15px, com um quadrado preto de 32px e seta branca à esquerda (padding esquerdo de 6px). Hover clareia para `orange-hover`. Desativado vira grafite.
- **Secondary:** `raised` com contorno `control-edge` e texto `text`; hover em `raised-hover`. Desativado a 40% de opacidade.
- **Pictogram button:** quadrado preto de 48px com glifo branco de 24px (configurações, fechar o diálogo); hover em #262626. A variante pequena (32px, transparente, glifo `text-2`) remove entradas do trilho.
- **Link:** texto laranja sublinhado (offset 3px), sem fundo; hover em `orange-hover`.
- **Focus:** contorno de 2px em `orange` com offset de 2px em todo elemento focável.
- **Motion:** transição de cor em 120ms com `cubic-bezier(0.16, 1, 0.3, 1)`.

### Navigation (faixa de passos)
A assinatura do sistema. Quatro passos dividem a faixa laranja em partes iguais, separados por divisórias de tinta a 18%. Passo futuro: quadrado vazado com contorno de 2px e glifo a 60%. Passo feito: quadrado preto com glifo branco e rótulo bold. Passo atual: painel preto em toda a altura, rótulo laranja bold e quadrado laranja com glifo preto. Transições de 200ms.

### Gate Panel
Painel preto à direita dos passos (mín. 200px) com o número em escala de portão em laranja e um rótulo de 13px embaixo. Quando o número muda, ele gira como painel split-flap (`rotateX(-88deg)` para zero em 320ms). A barra de progresso de 5px corre na borda inferior da faixa: trilho preto, preenchimento branco.

### Pictograms
Quadrados pretos de 44px com glifo branco de 24px; 96px com glifo de 52px nas placas; 30px com glifo de 18px nos grupos da tabela. Ícones só da família Lucide, sempre com traço de 2.25.

### Counters
Fileira de números grandes separados por filetes verticais (padding 22px), cada um com rótulo de 12px em `text-2` embaixo. Avisos e erros pintam o número de `warn` ou `error` apenas quando a contagem é maior que zero.

### Tabs
Abas sem fundo e sem canto, 40px, texto `text-2`; hover em `panel`. A selecionada ganha texto `text` e o filete laranja de 3px. Cada aba leva uma contagem em quadrado preto com número laranja.

### Tag (tom/BPM)
O prefixo do nome de saída (`Am`, `90`, `Cm7`) em mono bold laranja sobre preto, cantos de 3px, inline com o resto do nome para nunca quebrar entre a tag e o sublinhado.

### Result Table
Tabela de 13px com cabeçalho fixo em `ground`, linhas separadas por filetes, hover em `panel`. A coluna de estado leva só o ícone de 16px. O estado pinta ícone e observação: conferir em `text` com fundo de 4% de branco no nome de saída, pulado em `text-3`, aviso em `warn`, erro em `error`. Cada pack abre com um grupo: seta presa à borda, nome em bold e caminho em mono.

### Inputs / Fields
- **Style:** fundo `ground`, contorno `control-edge`, cantos de 4px, 36px de altura, 14px; campo numérico com 110px e números tabulares.
- **Checkbox:** 18px com `accent-color` laranja.
- **Focus:** o anel laranja global.
- **Disabled:** 40% de opacidade.

### Dialog
Diálogo de até 580px em `panel`, cantos de 6px, aberto por uma faixa laranja com título de 22px em `ink` e o botão de fechar em quadrado preto. Seções separadas por filetes, título de seção em 15px bold.

## Do's and Don'ts

### Do:
- **Do** reservar `orange` para a faixa de passos, a faixa do diálogo, a ação principal, a aba selecionada, o foco e os números em painel preto.
- **Do** usar texto `ink` sobre qualquer laranja.
- **Do** desenhar todo ícone como glifo branco de Lucide em quadrado preto, com setas presas à borda (`0 4px 4px 0`).
- **Do** mostrar números importantes em 700 e tabulares, em escala de portão (56px) ou de contador (36px).
- **Do** criar profundidade por tom (`black`, `ground`, `panel`, `raised`) e filetes de 1px em `line`.
- **Do** desativar o botão principal para grafite (`raised`, `text-3`).
- **Do** respeitar `prefers-reduced-motion`: sem split-flap, sem giro, sem transições.

### Don't:
- **Don't** usar laranja como cor de estado, de fundo decorativo ou em ícones soltos sobre grafite.
- **Don't** colocar texto branco sobre laranja.
- **Don't** introduzir uma segunda família de ícones, traços diferentes ou glifos de texto como ícone.
- **Don't** adicionar sombras a placas, painéis ou botões; a elevação é só do diálogo.
- **Don't** usar caixa alta forçada ou tracking aberto em rótulos.
- **Don't** arredondar além de 4px fora do diálogo.

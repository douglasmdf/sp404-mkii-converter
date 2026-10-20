---
version: 1
slug: "app-index-html"
primary_target: "app/index.html"
related_targets: ["app/src/main.ts","app/src/styles.css"]
---

# Surface brief: janela principal do app

Scope: a janela única do app de desktop (app/index.html + app/src): lista de entradas, Dry Run, conversão, resumo, desfazer e o diálogo de configurações. Visitor mode: Operate.

Audience and job: donos da SP-404 MKII convertendo um pack novo (sessões curtas) ou organizando a biblioteca (sessões longas). Depois do Dry Run ou da conversão, o que mais importa é um resumo rápido; os detalhes (nomes novos, incertos, pulados, erros) ficam a um clique. O autor rejeita: cara de site genérico, poluição visual, vazio demais.

Constraints: preservar toda a função e todos os textos existentes (pt-BR e en), atalhos (Cmd/Ctrl+O, Shift+Cmd/Ctrl+O, Cmd/Ctrl+Enter, Cmd/Ctrl+,), arrastar e soltar, Desfazer, Cancelar, regra de não iniciar duas conversões. Paleta pinada pelo autor: cinza, preto e laranja (SP-404 MKII / Roland). App offline: fontes servidas do próprio projeto.

## Direction contract

THESIS: o app é uma sinalização de aeroporto para levar um pack até o cartão SD: uma faixa de placa mostra só a próxima decisão e o número que importa em escala monumental. Recusa o padrão da categoria: área de soltar centralizada, lista cinza e um botão azul.

OWN-WORLD: painel laranja Roland (a cor da placa, reservada a caminho, passo atual e ação principal) com texto preto; preto de placa para quadrados de pictograma e para os números; cinzas grafite para o chão da janela e os painéis. Uma humanista legível de sinalização (Atkinson Hyperlegible Next) em caixa normal; números em peso bold e escala de portão. Pictogramas brancos em quadrados pretos; setas presas à borda.

STORY: o usuário vê onde está (Adicione, Confira, Converta, Cartão SD), solta o pack, confere o resumo em números grandes, converte e sabe que está pronto para o cartão; incertos e erros aparecem como desvios sinalizados.

FIRST VIEWPORT: faixa laranja de 112px no topo com os quatro passos (o atual em quadrado preto com número laranja) e, à direita, o total em número monumental dentro de um painel preto; engrenagem como pictograma. Abaixo, à esquerda, trilho grafite com as entradas (packs e arquivos soltos) e os botões de adicionar; à direita, a área de trabalho: vazia é uma placa de chegada com seta grande; com plano, uma fileira de contadores grandes (convertidos, copiados, pulados, incertos, erros) e a tabela origem → nome na SP-404 filtrável por abas. Barra inferior: Limpar, Desfazer à esquerda; Dry Run e Converter tudo (botão preto com seta) à direita.

FORM: wayfinding-cartography-signage-terminal-yellow-wayfinding (challenger escolhido pelo usuário, com o amarelo traduzido para o laranja da paleta pinada); assinatura: a faixa de passos que se re-sinaliza a cada etapa, com os números trocando como painel split-flap; seed:b9201aa090f55974

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

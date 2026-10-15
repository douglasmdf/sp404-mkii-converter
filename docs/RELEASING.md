# Publicando uma versão

## Versão sem assinatura (atual)

1. Atualize `version` em `Cargo.toml` (workspace), `app/package.json` e `app/src-tauri/tauri.conf.json`.
2. Commit, depois `git tag v0.1.0 && git push --tags`.
3. O workflow **Release** gera um rascunho de release no GitHub com:
   - `SP-404 MKII Converter_<versão>_aarch64.dmg` (Mac Apple Silicon)
   - `SP-404 MKII Converter_<versão>_x64.dmg` (Mac Intel)
   - `SP-404 MKII Converter_<versão>_x64-setup.exe` (Windows, instala sem administrador)
4. Revise e publique o rascunho.

Sem assinatura, o macOS mostra "desenvolvedor não identificado" (abrir com clique direito → Abrir)
e o Windows mostra o aviso do SmartScreen ("Mais informações" → "Executar assim mesmo").

## Antes de distribuir publicamente

- **Assinatura e notarização Apple:** conta Apple Developer; configure os secrets
  `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
  `APPLE_PASSWORD` e `APPLE_TEAM_ID` no repositório — o `tauri-action` usa automaticamente.
- **Assinatura Windows:** certificado de code signing; configure `bundle.windows.certificateThumbprint`
  (ou assinatura via Azure Trusted Signing) em `tauri.conf.json`.
- **Atualização automática:** adicione `tauri-plugin-updater`, gere as chaves com
  `npx tauri signer generate`, coloque a chave pública e o endpoint em `plugins.updater`
  no `tauri.conf.json`, mude `bundle.createUpdaterArtifacts` para `true` e configure os secrets
  `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- **Nome:** "SP-404" é marca da Roland. Para distribuição pública, considere um nome próprio
  (ex.: "… for SP-404 MKII") e um aviso de que o app não é afiliado à Roland.

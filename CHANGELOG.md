# Changelog

Todas as mudanças relevantes do Mustel são registradas aqui. As seções separam
**Features**, **Bug fixes** e as demais mudanças.

## [v1.2.0] - 2026-11-12
### Features
- Adiciona a aba About com os textos legais embutidos
- Empacota o mustel-gui.exe sem console no release do Windows
- Anexa executáveis soltos do Windows na release
- Test de conexão vira spinner, check ou X no cartão
- Adiciona toasts para recados efêmeros de ação
- Unifica o Test de servidor e perfil de IA no mesmo botão
- Adiciona log de atividade e relatório anonimizado
- Redesenha o rodapé do relatório de atividade
- Atualização automática pelo GitHub Releases
- Adiciona instalador Inno Setup ao release do Windows
- Adiciona o modo somente leitura das consultas na aba Query
- Torna o modo somente leitura das consultas o padrão da primeira execução
### Bug fixes
- Corrige as ações do perfil de IA vazando do painel
- Trunca o status da barra para conter o layout
- Mínimo da janela deixa de crescer com a escala da fonte
- Gravar a aparência preserva seletores e queries salvas
- Erro no toast fica 10 s e a barra anda sozinha
- Fecha o toast só no X e copia o erro pelo botão
- Seleciona a linha da Activity pelo clique com Sense::click
- Corrige abertura de links na GUI com a feature `links`
- Corrige links de e-mail no About e no relatório
- Corrige instalador Windows e documentação de atualização
- Encontra Inno Setup instalado só para o usuário atual
- Oculta falso-positivo e verifica CSV no smoke da GUI
### Segurança
- Restringe o log de atividade a permissões privadas no Unix
- Endurece o salvamento dos relatórios da Activity
- Endurece a imagem do teste Wine do instalador
### Documentação
- Corrige o aviso de licença do Inno Setup na documentação
- Corrige a afirmação sobre a licença do Inno Setup no plano do instalador
- Remove referências a planos da documentação de produto
- Remove referências a tarefas e planos da documentação
### Outras mudanças
- Concede `checks: write` aos jobs de auditoria
- Remove logs de depuração da aba Settings
- Remove campos não usados do modelo de release
- Atualiza `sha2` para 0.11 no core
- Adiciona scripts para preparar bump de release em PR
- Permite preparar release a partir de qualquer branch


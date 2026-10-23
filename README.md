# g-pet

Um fantasminha `(•‿•)` que mora na última linha do seu terminal. Ele anda de um lado para o outro, pisca e de vez em quando solta uma frase assustadora.

```
$ ls
Cargo.toml  src  target
$
(•‿•) Tem alguém aí?
```

## Como rodar

Requisitos: Linux e [Rust](https://rustup.rs) (o `cargo` vem junto). O projeto não tem dependências externas.

```bash
# compilar
cargo build --release

# rodar (o binário fica em target/release/g-pet)
./target/release/g-pet
```

O `g-pet` vai para background sozinho e devolve o prompt na hora. Você continua usando o terminal normalmente.

Para encerrar todos os fantasminhas:

```bash
./target/release/g-pet kill
```

Se quiser chamá-lo de qualquer lugar, copie o binário para uma pasta do seu `PATH`:

```bash
cp target/release/g-pet ~/.local/bin/
g-pet
```

Para ele aparecer em todo terminal novo, rode:

```bash
g-pet start
```

Isso acrescenta a inicialização ao `~/.bashrc` (ou ao `~/.zshrc`, se o seu shell for zsh). Se já houver uma inicialização em algum dos arquivos abaixo, ele não duplica. Se preferir fazer à mão, basta colocar `g-pet` no final do seu `~/.bashrc` (ou equivalente). Os terminais que já estão abertos não mudam: rode `g-pet` neles.

Para ele parar de iniciar nos terminais novos, rode:

```bash
g-pet stop
```

Isso remove a linha que inicia o `g-pet` de `~/.bashrc`, `~/.bash_profile`, `~/.profile`, `~/.zshrc` e `~/.zprofile`. Os fantasminhas dos terminais que já estão abertos continuam rodando; use `g-pet kill` para encerrá-los.

## Como funciona

- **Última linha reservada:** o programa usa uma região de rolagem do terminal (sequências de escape ANSI) para reservar a última linha só para o fantasminha. O resto da tela rola normalmente, sem encostar nele.
- **Animação:** a cada 150 ms ele sorteia o que fazer: andar, ficar parado, piscar `(-‿-)` ou falar uma frase. Quando chega na borda da tela, ele volta.
- **Não atrapalha outros programas:** ele só desenha quando o shell está no prompt. Se você abrir `vim`, `htop` ou qualquer programa que ocupe a tela, ele fica quieto e volta quando você sair.
- **Um por terminal:** se já houver um `g-pet` no terminal atual, o segundo não sobe.
- **Limpa depois de si:** ao receber `SIGTERM`/`SIGINT`/`SIGHUP` ou quando o shell que o iniciou fechar, ele apaga a última linha e devolve o terminal ao normal.
- **Se adapta ao tamanho:** se você redimensionar a janela, ele reajusta a região reservada. Frases que não cabem na largura são descartadas.

Todo o código está em `src/main.rs`. Ele escreve direto em `/dev/tty` e usa `/proc` para achar outros `g-pet`, por isso só funciona no Linux.

## Personalizando

No topo de `src/main.rs`:

- `PHRASES`: lista de frases que ele fala.
- `SPRITE` e `SPRITE_BLINK`: o desenho do fantasminha (se mudar o tamanho, ajuste também `SPRITE_W`).
- `TICK`: velocidade da animação.

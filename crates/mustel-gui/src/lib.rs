//! Frontend gráfico do Mustel (egui).
//!
//! Consome os mesmos casos de uso do `mustel-core` em processo, sem IPC (D9): recebe eventos por um
//! `ChannelSink`, roda o trabalho em um runtime tokio de background e redesenha via
//! `ctx.request_repaint()`. Ligado ao binário `mustel` pelo subcomando `gui` (T41).

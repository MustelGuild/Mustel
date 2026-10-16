//! Testes das primitivas de UI pela biblioteca.
//!
//! A suíte roda **sem terminal**: aqui se verifica o comportamento que protege o usuário nesse
//! cenário (falhar claro em vez de travar) e a renderização determinística. A interação real de
//! teclado é verificada por pseudo-terminal no CI Linux (`tests/pty_prompts.rs`).

use mustel::ui;
use mustel_core::error::exit_code;

#[test]
fn environment_without_terminal_is_detected() {
    assert!(!ui::is_interactive());
}

#[test]
fn every_prompt_refuses_to_block_without_a_terminal() {
    let options = vec!["dev".to_string(), "prod".to_string()];

    let errors = vec![
        (
            "seleção",
            ui::prompts::select_one("servidor", &options, None).err(),
        ),
        (
            "multi-seleção",
            ui::prompts::select_many("servidores", &options, &[0]).err(),
        ),
        (
            "confirmação",
            ui::prompts::confirm("continuar?", true).err(),
        ),
        ("texto", ui::prompts::text("nome", None).err()),
        ("senha", ui::prompts::password("senha").err()),
    ];

    for (label, error) in errors {
        let error = error.unwrap_or_else(|| panic!("{label}: sem terminal não pode prompt"));
        let message = error.to_string();
        assert!(
            message.contains("interactive terminal"),
            "{label}: mensagem precisa explicar o modo não-interativo, veio: {message}"
        );
        assert_eq!(
            error.exit_code(),
            exit_code::INVALID_STATE,
            "{label}: modo não-interativo é estado inválido (40)"
        );
    }
}

#[test]
fn table_rendering_is_deterministic() {
    let rendered = ui::table::render(
        &["Name", "Host", "Port"],
        &[
            vec![
                "dev".to_string(),
                "localhost".to_string(),
                "5432".to_string(),
            ],
            vec![
                "prod".to_string(),
                "prod-db.company.com".to_string(),
                "5432".to_string(),
            ],
        ],
    );
    insta::assert_snapshot!(rendered);
}

#[test]
fn styling_is_plain_without_a_terminal() {
    assert_eq!(ui::style::bold("dev"), "dev");
    assert_eq!(ui::style::warning("cuidado"), "cuidado");

    // A saída que compõe contrato (erro) não recebe cor de qualquer forma.
    let error = mustel_core::error::CoreError::InvalidState("boom".to_string());
    assert_eq!(mustel::exit::format_error(&error), "Error: boom");
}

//! The command-line surface: one name for the binary everywhere, and completion scripts that
//! are generated from the same command tree the parser uses.
//!
//! The completion files are written by the packagers, not by hand, so the only thing worth
//! testing here is that they exist for every shipped shell, name the binary, and describe the
//! verbs the binary actually has.

use clap::{CommandFactory, Parser, ValueEnum};
use genarch_settings::cli::{BINARY, Cli, CompletionShell, Verb};

/// Every verb an operator or a unit can invoke. A verb added without a completion line is a
/// completion that lies about what the binary accepts.
const VERBS: &[&str] = &["report", "apply", "harvest", "units", "converge", "completions", "import"];

#[test]
fn the_clap_name_is_the_binary_name() {
    // `BINARY` is the one home: the clap name, the error prefix and every completion script
    // read it. If the attribute drifts, `--version` and the installed path disagree.
    assert_eq!(Cli::command().get_name(), BINARY);
    assert_eq!(BINARY, "genarch-settings");
}

#[test]
fn every_shell_names_the_binary_and_every_verb() {
    assert_eq!(
        CompletionShell::value_variants().len(),
        3,
        "fish, bash and zsh are the shipped shells"
    );
    for shell in [CompletionShell::Bash, CompletionShell::Zsh, CompletionShell::Fish] {
        let script = genarch_settings::cli::completions(shell);
        assert!(script.contains(BINARY), "{shell:?}: the binary name is missing");
        for verb in VERBS {
            assert!(script.contains(verb), "{shell:?}: verb '{verb}' is not completed");
        }
    }
}

#[test]
fn each_shell_gets_its_own_dialect() {
    // A bash script is not a fish script: generating the same text for all three would pass a
    // "contains the name" test and still load nothing.
    let bash = genarch_settings::cli::completions(CompletionShell::Bash);
    let zsh = genarch_settings::cli::completions(CompletionShell::Zsh);
    let fish = genarch_settings::cli::completions(CompletionShell::Fish);
    assert!(bash.contains("complete -F _genarch__settings"), "bash uses complete -F");
    assert!(bash.contains(&format!(" {BINARY}")), "bash registers the binary name");
    assert!(zsh.starts_with(&format!("#compdef {BINARY}")), "zsh is an autoloaded _file");
    assert!(fish.contains(&format!("complete -c {BINARY}")), "fish uses complete -c");
}

#[test]
fn the_completions_verb_parses_and_rejects_an_unknown_shell() {
    let cli = Cli::try_parse_from([BINARY, "completions", "fish"]).expect("completions fish");
    assert!(matches!(cli.verb, Verb::Completions { shell: CompletionShell::Fish }));

    let err = Cli::try_parse_from([BINARY, "completions", "tcsh"]).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("bash"), "the error lists the real choices: {message}");
}

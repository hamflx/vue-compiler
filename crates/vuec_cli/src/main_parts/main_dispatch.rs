fn main() {
    match run_with_args(std::env::args_os()) {
        Ok(output) => {
            if !output.stdout.is_empty() {
                print!("{}", output.stdout);
            }
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
            }
            std::process::exit(output.code);
        }
        Err(err) => {
            eprintln!("{err:#}");
            std::process::exit(1);
        }
    }
}

fn run_with_args<I, T>(args: I) -> Result<RunOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                0
            } else {
                2
            };
            return Ok(RunOutput {
                stdout: if code == 0 {
                    error.to_string()
                } else {
                    String::new()
                },
                stderr: if code == 0 {
                    String::new()
                } else {
                    error.to_string()
                },
                code,
            });
        }
    };
    match cli.command {
        CliCommand::CompileTemplate(args) => compile_template_command(args),
        CliCommand::CompileSfc(args) => compile_sfc_command(args),
        CliCommand::CompileSsr(args) => compile_ssr_command(args),
        CliCommand::CompileBatch(args) => compile_batch_command(args),
        CliCommand::ParseSfc(args) => parse_sfc_command(args),
        CliCommand::Conformance(args) => conformance_command(args),
        CliCommand::Bench(args) => bench_command(args),
    }
}

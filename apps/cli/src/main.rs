#![forbid(unsafe_code)]

use std::{
    io::{self, Read},
    path::PathBuf,
    sync::Arc,
};

use archive_core::{
    ArchiveBackend, ArchiveError, ArchiveErrorCode, ArchiveFormat, CompressionProfile,
    ConflictPolicy, CreateArchiveRequest, ExtractArchiveRequest, ListArchiveRequest,
    ProgressReporter, TaskProgress, TestArchiveRequest,
};
use archive_sevenzip::SevenZipCliBackend;
use clap::{Args, Parser, Subcommand};
use secrecy::SecretString;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(name = "qzip-cli", about = "QZip M1 backend verifier (developer tool)")]
struct Cli {
    /// Directory that contains both 7z.exe and 7z.dll.
    #[arg(long, global = true)]
    sevenzip_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Capabilities,
    Create(CreateArgs),
    List(ArchiveArgs),
    Extract(ExtractArgs),
    Test(ArchiveArgs),
}

#[derive(Args)]
struct CreateArgs {
    #[arg(long)]
    format: ArchiveFormat,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "balanced")]
    profile: CompressionProfile,
    #[arg(long)]
    encrypt_headers: bool,
    #[command(flatten)]
    password: PasswordArgs,
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
}

#[derive(Args)]
struct ArchiveArgs {
    #[arg(long)]
    archive: PathBuf,
    #[command(flatten)]
    password: PasswordArgs,
}

#[derive(Args)]
struct ExtractArgs {
    #[arg(long)]
    archive: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "rename")]
    conflict: ConflictPolicy,
    #[command(flatten)]
    password: PasswordArgs,
}

#[derive(Args, Default)]
struct PasswordArgs {
    /// Read a password once from standard input. Passwords are never accepted as command-line values.
    #[arg(long)]
    password_stdin: bool,
    /// Prompt for a password without echoing it.
    #[arg(long)]
    password_prompt: bool,
}

struct JsonProgress;
impl ProgressReporter for JsonProgress {
    fn report(&self, progress: TaskProgress) {
        emit(
            "progress",
            json!({ "operation": progress.operation, "percent": progress.percent, "detail": progress.detail }),
        );
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let backend = Arc::new(SevenZipCliBackend::new(resolve_executable(
        cli.sevenzip_dir,
    )));
    let cancellation = CancellationToken::new();
    let result = match cli.command {
        Command::Capabilities => run_capabilities(backend, cancellation).await,
        Command::Create(args) => run_create(backend, args, cancellation).await,
        Command::List(args) => run_list(backend, args, cancellation).await,
        Command::Extract(args) => run_extract(backend, args, cancellation).await,
        Command::Test(args) => run_test(backend, args, cancellation).await,
    };
    if let Err(error) = result {
        emit(
            "failed",
            json!({ "code": error.code, "message": error.message }),
        );
        std::process::exit(exit_code(&error));
    }
}

fn resolve_executable(explicit: Option<PathBuf>) -> PathBuf {
    let directory = explicit
        .or_else(|| std::env::var_os("QZIP_7ZIP_DIR").map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("third_party/7zip/bin/win-x64")
        });
    directory.join(if cfg!(windows) { "7z.exe" } else { "7z" })
}

async fn run_capabilities(
    backend: Arc<SevenZipCliBackend>,
    cancellation: CancellationToken,
) -> Result<(), ArchiveError> {
    emit("started", json!({ "operation": "capabilities" }));
    let capabilities = await_or_cancel(backend.capabilities(), cancellation).await?;
    emit(
        "completed",
        serde_json::to_value(capabilities).map_err(|_| {
            ArchiveError::new(
                ArchiveErrorCode::Unknown,
                "could not serialize capabilities",
            )
        })?,
    );
    Ok(())
}
async fn run_create(
    backend: Arc<SevenZipCliBackend>,
    args: CreateArgs,
    cancellation: CancellationToken,
) -> Result<(), ArchiveError> {
    let password = read_password(args.password)?;
    emit("started", json!({ "operation": "create" }));
    let result = await_or_cancel(
        backend.create(
            CreateArchiveRequest {
                format: args.format,
                output: args.output,
                inputs: args.inputs,
                profile: args.profile,
                password,
                encrypt_headers: args.encrypt_headers,
                test_after_create: false,
            },
            Arc::new(JsonProgress),
            cancellation.child_token(),
        ),
        cancellation,
    )
    .await?;
    emit(
        "completed",
        json!({ "operation": "create", "warnings": result.warnings }),
    );
    Ok(())
}
async fn run_list(
    backend: Arc<SevenZipCliBackend>,
    args: ArchiveArgs,
    cancellation: CancellationToken,
) -> Result<(), ArchiveError> {
    let password = read_password(args.password)?;
    emit("started", json!({ "operation": "list" }));
    let entries = await_or_cancel(
        backend.list(
            ListArchiveRequest {
                archive: args.archive,
                password,
            },
            cancellation.child_token(),
        ),
        cancellation,
    )
    .await?;
    emit(
        "completed",
        json!({ "operation": "list", "entries": entries }),
    );
    Ok(())
}
async fn run_extract(
    backend: Arc<SevenZipCliBackend>,
    args: ExtractArgs,
    cancellation: CancellationToken,
) -> Result<(), ArchiveError> {
    let password = read_password(args.password)?;
    emit("started", json!({ "operation": "extract" }));
    let result = await_or_cancel(
        backend.extract(
            ExtractArchiveRequest {
                archive: args.archive,
                output: args.output,
                selected_entries: None,
                conflict_policy: args.conflict,
                password,
            },
            Arc::new(JsonProgress),
            cancellation.child_token(),
        ),
        cancellation,
    )
    .await?;
    emit(
        "completed",
        json!({ "operation": "extract", "warnings": result.warnings }),
    );
    Ok(())
}
async fn run_test(
    backend: Arc<SevenZipCliBackend>,
    args: ArchiveArgs,
    cancellation: CancellationToken,
) -> Result<(), ArchiveError> {
    let password = read_password(args.password)?;
    emit("started", json!({ "operation": "test" }));
    let result = await_or_cancel(
        backend.test(
            TestArchiveRequest {
                archive: args.archive,
                password,
            },
            cancellation.child_token(),
        ),
        cancellation,
    )
    .await?;
    emit(
        "completed",
        json!({ "operation": "test", "valid": result.valid, "warnings": result.warnings }),
    );
    Ok(())
}

async fn await_or_cancel<F, T>(
    operation: F,
    cancellation: CancellationToken,
) -> Result<T, ArchiveError>
where
    F: std::future::Future<Output = Result<T, ArchiveError>>,
{
    tokio::pin!(operation);
    tokio::select! { result = &mut operation => result, signal = tokio::signal::ctrl_c() => { if signal.is_ok() { cancellation.cancel(); let result = operation.await; if result.as_ref().is_err_and(|error| error.code == ArchiveErrorCode::Cancelled) { emit("cancelled", json!({})); } result } else { Err(ArchiveError::new(ArchiveErrorCode::Cancelled, "archive operation was cancelled")) } } }
}

fn read_password(args: PasswordArgs) -> Result<Option<SecretString>, ArchiveError> {
    if args.password_stdin && args.password_prompt {
        return Err(ArchiveError::invalid_option(
            "password",
            "use either --password-stdin or --password-prompt",
        ));
    }
    if args.password_stdin {
        let mut value = String::new();
        io::stdin().read_to_string(&mut value).map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::Unknown, "could not read password input")
        })?;
        return Ok(Some(SecretString::from(
            value.trim_end_matches(['\r', '\n']).to_owned(),
        )));
    }
    if args.password_prompt {
        return rpassword::prompt_password("Archive password: ")
            .map(SecretString::from)
            .map(Some)
            .map_err(|_| {
                ArchiveError::new(ArchiveErrorCode::Unknown, "could not read password input")
            });
    }
    Ok(None)
}
fn emit(event: &str, payload: serde_json::Value) {
    println!("{}", json!({ "event": event, "data": payload }));
}
fn exit_code(error: &ArchiveError) -> i32 {
    match error.code {
        ArchiveErrorCode::InvalidOption | ArchiveErrorCode::UnsupportedOption => 2,
        ArchiveErrorCode::BackendUnavailable => 3,
        ArchiveErrorCode::Cancelled => 130,
        _ => 4,
    }
}

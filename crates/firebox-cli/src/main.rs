mod client;
mod dto;
mod format;

use clap::{Args, Parser, Subcommand};
use client::{CliError, Client};
use dto::{ActionResponse, CreateVmRequest, NetworkConfigDto, VmDetail, VmSummary};

#[derive(Parser)]
#[command(name = "firebox", about = "Manage Firecracker MicroVMs")]
struct Cli {
    #[arg(long, default_value = "http://localhost:8080", global = true)]
    host: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// VM management
    Vm(VmArgs),
}

#[derive(Args)]
struct VmArgs {
    #[command(subcommand)]
    action: VmAction,
}

#[derive(Subcommand)]
enum VmAction {
    /// Create a new VM
    Create {
        #[arg(long)]
        id: Option<String>,
        #[arg(long, default_value = "1")]
        vcpus: u8,
        #[arg(long, default_value = "128")]
        memory: u32,
        #[arg(long)]
        kernel: String,
        #[arg(long)]
        rootfs: String,
        #[arg(long)]
        tap: Option<String>,
        #[arg(long)]
        mac: Option<String>,
    },
    /// List all VMs
    List,
    /// Get details of a VM
    Get { id: String },
    /// Start a VM
    Start { id: String },
    /// Stop a VM
    Stop { id: String },
    /// Delete a stopped VM
    Delete { id: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = Client::new(&cli.host);

    let result = run(cli.command, &client).await;

    if let Err(e) = result {
        eprintln!("Error: {e}");
        let code = match e {
            CliError::Api(_) => 1,
            CliError::Http(_) => 1,
        };
        std::process::exit(code);
    }
}

async fn run(cmd: Commands, client: &Client) -> Result<(), CliError> {
    match cmd {
        Commands::Vm(args) => match args.action {
            VmAction::Create { id, vcpus, memory, kernel, rootfs, tap, mac } => {
                let network = tap.map(|tap_device| NetworkConfigDto { tap_device, mac });
                let body = CreateVmRequest {
                    id,
                    vcpus,
                    memory_mb: memory,
                    kernel,
                    rootfs,
                    network,
                };
                let vm: VmSummary = client.post_json("/vms", &body).await?;
                println!("Created VM {}", vm.id);
            }

            VmAction::List => {
                let vms: Vec<VmSummary> = client.get("/vms").await?;
                format::print_vm_list(&vms);
            }

            VmAction::Get { id } => {
                let vm: VmDetail = client.get(&format!("/vms/{id}")).await?;
                format::print_vm_detail(&vm);
            }

            VmAction::Start { id } => {
                let r: ActionResponse = client.post(&format!("/vms/{id}/start")).await?;
                println!("VM {id} {}", r.status);
            }

            VmAction::Stop { id } => {
                let r: ActionResponse = client.post(&format!("/vms/{id}/stop")).await?;
                println!("VM {id} {}", r.status);
            }

            VmAction::Delete { id } => {
                client.delete(&format!("/vms/{id}")).await?;
                println!("VM {id} deleted");
            }
        },
    }
    Ok(())
}

mod config;
mod dto;
mod error;
mod handlers;

use std::sync::Arc;

use actix_web::{web, App, HttpServer};
use firebox_core::Core;
use firebox_store::MemoryStore;
use firebox_vmm::FirecrackerVmm;
use tracing::info;

use crate::config::load_config;
use crate::handlers::vms;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let daemon_cfg = load_config().expect("failed to load config");

    tracing_subscriber::fmt()
        .with_env_filter(&daemon_cfg.log_level)
        .init();

    let listen_addr = daemon_cfg.listen_addr.clone();

    let vmm = FirecrackerVmm::new(daemon_cfg.firecracker_bin.clone());
    let core = Arc::new(Core::new(
        Arc::new(MemoryStore::new()),
        Arc::new(vmm),
        Arc::new(daemon_cfg),
    ));

    info!("firebox daemon listening on {listen_addr}");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(Arc::clone(&core)))
            .service(
                web::scope("/api/v1")
                    .route("/vms", web::post().to(vms::create_vm::<MemoryStore, FirecrackerVmm>))
                    .route("/vms", web::get().to(vms::list_vms::<MemoryStore, FirecrackerVmm>))
                    .route("/vms/{id}", web::get().to(vms::get_vm::<MemoryStore, FirecrackerVmm>))
                    .route("/vms/{id}", web::delete().to(vms::delete_vm::<MemoryStore, FirecrackerVmm>))
                    .route("/vms/{id}/start", web::post().to(vms::start_vm::<MemoryStore, FirecrackerVmm>))
                    .route("/vms/{id}/stop", web::post().to(vms::stop_vm::<MemoryStore, FirecrackerVmm>)),
            )
    })
    .bind(&listen_addr)?
    .run()
    .await
}

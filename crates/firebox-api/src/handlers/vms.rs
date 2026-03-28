use std::sync::Arc;

use actix_web::{web, HttpResponse};
use firebox_core::{Core, VmConfig};
use firebox_store::Store;
use firebox_vmm::Vmm;

use crate::dto::{ActionResponse, CreateVmRequest, VmDetail, VmSummary};
use crate::error::ApiError;

pub async fn create_vm<S: Store, V: Vmm>(
    core: web::Data<Arc<Core<S, V>>>,
    body: web::Json<CreateVmRequest>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();
    let config = VmConfig {
        id: req.id,
        vcpus: req.vcpus,
        memory_mb: req.memory_mb,
        kernel: req.kernel,
        rootfs: req.rootfs,
        network: req.network.map(Into::into),
    };
    let vm = core.create_vm(config).await?;
    Ok(HttpResponse::Created().json(VmSummary::from(vm)))
}

pub async fn list_vms<S: Store, V: Vmm>(
    core: web::Data<Arc<Core<S, V>>>,
) -> Result<HttpResponse, ApiError> {
    let vms = core.list_vms().await?;
    let summaries: Vec<VmSummary> = vms.into_iter().map(VmSummary::from).collect();
    Ok(HttpResponse::Ok().json(summaries))
}

pub async fn get_vm<S: Store, V: Vmm>(
    core: web::Data<Arc<Core<S, V>>>,
    id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let vm = core.get_vm(&id).await?;
    Ok(HttpResponse::Ok().json(VmDetail::from(vm)))
}

pub async fn delete_vm<S: Store, V: Vmm>(
    core: web::Data<Arc<Core<S, V>>>,
    id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    core.delete_vm(&id).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn start_vm<S: Store, V: Vmm>(
    core: web::Data<Arc<Core<S, V>>>,
    id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let vm = core.start_vm(&id).await?;
    Ok(HttpResponse::Ok().json(ActionResponse { status: vm.status.to_string() }))
}

pub async fn stop_vm<S: Store, V: Vmm>(
    core: web::Data<Arc<Core<S, V>>>,
    id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let vm = core.stop_vm(&id).await?;
    Ok(HttpResponse::Ok().json(ActionResponse { status: vm.status.to_string() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use firebox_core::{Core, DaemonConfig};
    use firebox_store::MemoryStore;
    use firebox_vmm::StubVmm;

    type TestCore = Core<MemoryStore, StubVmm>;

    fn make_core() -> Arc<TestCore> {
        Arc::new(Core::new(
            Arc::new(MemoryStore::new()),
            Arc::new(StubVmm),
            Arc::new(DaemonConfig::default()),
        ))
    }

    macro_rules! app {
        ($core:expr) => {
            test::init_service(
                App::new()
                    .app_data(web::Data::new($core))
                    .service(
                        web::scope("/api/v1")
                            .route("/vms", web::post().to(create_vm::<MemoryStore, StubVmm>))
                            .route("/vms", web::get().to(list_vms::<MemoryStore, StubVmm>))
                            .route("/vms/{id}", web::get().to(get_vm::<MemoryStore, StubVmm>))
                            .route("/vms/{id}", web::delete().to(delete_vm::<MemoryStore, StubVmm>))
                            .route("/vms/{id}/start", web::post().to(start_vm::<MemoryStore, StubVmm>))
                            .route("/vms/{id}/stop", web::post().to(stop_vm::<MemoryStore, StubVmm>)),
                    ),
            )
            .await
        };
    }

    fn create_body() -> serde_json::Value {
        serde_json::json!({
            "id": "test-vm",
            "vcpus": 1,
            "memory_mb": 128,
            "kernel": "/boot/vmlinux",
            "rootfs": "/var/rootfs.ext4"
        })
    }

    #[actix_web::test]
    async fn test_create_and_get() {
        let app = app!(make_core());

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/vms")
                .set_json(create_body())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), 201);

        let resp = test::call_service(
            &app,
            test::TestRequest::get().uri("/api/v1/vms/test-vm").to_request(),
        )
        .await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_list_vms() {
        let app = app!(make_core());

        test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/vms")
                .set_json(create_body())
                .to_request(),
        )
        .await;

        let body: serde_json::Value = test::call_and_read_body_json(
            &app,
            test::TestRequest::get().uri("/api/v1/vms").to_request(),
        )
        .await;
        assert_eq!(body.as_array().unwrap().len(), 1);
    }

    #[actix_web::test]
    async fn test_start_stop_delete() {
        let app = app!(make_core());

        test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/vms")
                .set_json(create_body())
                .to_request(),
        )
        .await;

        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post().uri("/api/v1/vms/test-vm/start").to_request()
            )
            .await
            .status(),
            200
        );

        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post().uri("/api/v1/vms/test-vm/stop").to_request()
            )
            .await
            .status(),
            200
        );

        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::delete().uri("/api/v1/vms/test-vm").to_request()
            )
            .await
            .status(),
            204
        );
    }

    #[actix_web::test]
    async fn test_get_missing_returns_404() {
        let app = app!(make_core());
        let resp = test::call_service(
            &app,
            test::TestRequest::get().uri("/api/v1/vms/nope").to_request(),
        )
        .await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_start_already_running_returns_409() {
        let app = app!(make_core());

        test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/vms")
                .set_json(create_body())
                .to_request(),
        )
        .await;
        test::call_service(
            &app,
            test::TestRequest::post().uri("/api/v1/vms/test-vm/start").to_request(),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post().uri("/api/v1/vms/test-vm/start").to_request(),
        )
        .await;
        assert_eq!(resp.status(), 409);
    }
}

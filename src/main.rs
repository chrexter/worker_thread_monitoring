use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use num_cpus;
use serde::Serialize;
use std::io::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use tokio::task;

// Create a shared counter for active tasks
struct AppState {
    active_tasks: Arc<AtomicUsize>,
}

#[derive(Serialize)]
struct Response {
    message: String,
    thread_id: String,
    result: Option<f64>,
}

async fn monitor_handler(data: web::Data<AppState>) -> HttpResponse {
    let current_tasks = data.active_tasks.load(Ordering::Relaxed);
    let system_threads = num_cpus::get();

    HttpResponse::Ok().json(serde_json::json!({
        "active_tasks": current_tasks,
        "system_threads": system_threads,
        "worker_threads": num_cpus::get_physical() // Physical CPU cores used by default
    }))
}

async fn sample_handler(data: web::Data<AppState>) -> HttpResponse {
    // Increment the counter when handling a request
    data.active_tasks.fetch_add(1, Ordering::SeqCst);

    // Simulate some work
    task::spawn_blocking(|| {
        std::thread::sleep(std::time::Duration::from_secs(1));
    })
    .await
    .unwrap();

    // Decrement the counter when done
    data.active_tasks.fetch_sub(1, Ordering::SeqCst);

    HttpResponse::Ok().body("Request processed")
}

async fn get_threads() -> HttpResponse {
    let thread_name = thread::current().name().unwrap_or("unnamed").to_string();

    HttpResponse::Ok().json(serde_json::json!({
        "worker_thread": thread_name,
        "thread_id": format!("{:?}", thread::current().id())
    }))
}

// Light task - but still handled by worker thread
async fn light_handler() -> HttpResponse {
    HttpResponse::Ok().json(Response {
        message: "Handled by Actix worker".to_string(),
        thread_id: format!("{:?}", thread::current().name()),
        result: None,
    })
}

// Heavy task - automatically handled by worker thread
async fn heavy_handler() -> HttpResponse {
    // CPU-intensive work
    let result = (0..1000000).map(|i| f64::sqrt(i as f64)).sum();

    HttpResponse::Ok().json(Response {
        message: "Processed by Actix worker".to_string(),
        thread_id: format!("{:?}", thread::current().name()),
        result: Some(result),
    })
}

#[actix_web::main]
async fn main() -> Result<()> {
    let state = web::Data::new(AppState {
        active_tasks: Arc::new(AtomicUsize::new(0)),
    });

    let server = HttpServer::new(move || {
        println!("Worker thread initialized: {:?}", thread::current().name());

        App::new()
            .app_data(state.clone())
            .route("/monitor", web::get().to(monitor_handler))
            .route("/sample", web::get().to(sample_handler))
            .route("/thread-info", web::get().to(get_threads))
            .route("/light", web::get().to(light_handler))
            .route("/heavy", web::get().to(heavy_handler))
    });

    println!("Serving on http://localhost:3000...");

    server
        .workers(num_cpus::get_physical())
        .bind("127.0.0.1:3000")?
        .run()
        .await
}

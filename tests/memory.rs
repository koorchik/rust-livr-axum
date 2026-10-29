//! The memory taken by a request with many keys. A test of its own: the
//! allocator counts the bytes of the whole process, so no other test may run
//! at the same time.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode, header};
use axum::routing::post;
use livr_axum::LivrForm;
use tower::ServiceExt;

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// The system allocator, with the bytes in use and their peak
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let current = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(current, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

livr::schema! {
    pub struct Login {
        login: ["required", "string"],
        tags: {"list_of": "string"},
    }
}

#[tokio::test]
async fn drops_unknown_keys_of_forms_before_they_are_allocated() {
    // 2 MB of distinct keys: a map of them took 25 MB
    let mut body: String = (0..230_000).map(|i| format!("k{i}=&")).collect();
    body.push_str("login=jane&tags=a");
    let size = body.len();
    let app = Router::new()
        .route(
            "/",
            post(|LivrForm(login): LivrForm<Login>| async move { login.login }),
        )
        .layer(DefaultBodyLimit::max(4 << 20));
    let request = Request::post("/")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();

    let before = CURRENT.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let response = app.oneshot(request).await.unwrap();
    let peak = PEAK.load(Ordering::Relaxed) - before;

    // The body is moved into the request, not copied: what is allocated is
    // the router and the response, not the keys
    assert!(peak < 64 << 10, "{peak} bytes for a body of {size} bytes");
    assert_eq!(response.status(), StatusCode::OK);
}

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Header, Method};
use rocket::{Request, Response};

/// Cache policy for the static web app.
///
/// Build assets under `/assets/` are content-hashed by Vite, so their URL changes
/// whenever their contents do; they can be cached forever. Everything else the
/// file server hands out - the `index.html` shell, `sw.js`, the manifest, icons -
/// must be revalidated on every load. Without this a browser keeps serving a
/// stale `index.html` that points at an old asset bundle and never picks up a new
/// build until a manual hard refresh. Dynamic endpoints are left untouched.
pub struct CacheControl;

#[rocket::async_trait]
impl Fairing for CacheControl {
    fn info(&self) -> Info {
        Info {
            name: "Cache-Control for the static web app",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        if request.method() != Method::Get {
            return;
        }
        let path = request.uri().path().as_str();
        // Leave the API, docs, and metrics to their own (dynamic) policy.
        if path.starts_with("/api") || path.starts_with("/swagger") || path == "/metrics" {
            return;
        }
        if response.headers().contains("Cache-Control") {
            return;
        }
        let policy = if path.starts_with("/assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        response.set_header(Header::new("Cache-Control", policy));
    }
}

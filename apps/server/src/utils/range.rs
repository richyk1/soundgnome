//! Request guard for the HTTP `Range` header.
//!
//! Rocket has no built-in guard for it, and the audio routes need the raw value
//! to decide whether to answer 200 or 206.

use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use rocket_okapi::gen::OpenApiGenerator;
use rocket_okapi::request::{OpenApiFromRequest, RequestHeaderInput};

/// The raw `Range` header value, e.g. `bytes=0-1023`.
pub struct RangeHeader(pub String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RangeHeader {
    type Error = std::convert::Infallible;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match req.headers().get_one("Range") {
            Some(value) => Outcome::Success(RangeHeader(value.to_string())),
            // Absent is normal, not an error: the route then serves the whole
            // file. Routes take `Option<RangeHeader>`, so Forward yields None.
            None => Outcome::Forward(rocket::http::Status::Ok),
        }
    }
}

impl<'r> OpenApiFromRequest<'r> for RangeHeader {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}

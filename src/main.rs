//! Fake IP camera.
#![forbid(unsafe_code)]
use actix_web::{rt, web, App, HttpResponse, HttpServer, Responder};
use async_stream::try_stream;
use bytes::Bytes;
use clap::Parser;
use core::time::Duration;
use futures_util::Stream;
use http::header::{HeaderMap, CONTENT_LENGTH, CONTENT_TYPE};
use mimalloc::MiMalloc;
use multipart_stream::Part;
use std::sync::Arc;
use std::ffi::OsStr;

/// Default musl allocator is slow.
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Fake mjpeg camera, for testing purposes.
#[derive(Debug, Parser)]
struct Args {
    /// HTTP server listen address.
    #[arg(short, long, default_value = "localhost:8080")]
    addr: String,

    /// Images to display.
    imgs: Vec<std::path::PathBuf>,
}

/// Contains the list of images to cycle.
#[derive(Clone)]
struct Data(Arc<Vec<std::path::PathBuf>>);

impl Data {
    /// Outputs a stream of images.
    fn camera(&self) -> impl Stream<Item = std::io::Result<Part>> {
        let imgs = self.0.clone();
        try_stream! {
            loop {
                for f in &*imgs {
                    let mut headers = HeaderMap::new();
                    //TODO: switch statement based on image type.
                    let fextension = f.extension().and_then(OsStr::to_str).unwrap();
                    match fextension {
                        "jpeg" =>headers.insert(CONTENT_TYPE, mime::IMAGE_JPEG.as_ref().parse().unwrap()),
                        "png" =>headers.insert(CONTENT_TYPE, mime::IMAGE_PNG.as_ref().parse().unwrap()),
                        "webp" =>headers.insert(CONTENT_TYPE, "image/webp".parse().unwrap()),
                        _ =>headers.insert(CONTENT_TYPE, "image/*".parse().unwrap()),
                    };
                    // headers.insert(CONTENT_TYPE, mime::IMAGE_JPEG.as_ref().parse().unwrap());
                    let body = Bytes::from(std::fs::read(f)?);
                    headers.insert(CONTENT_LENGTH, body.len().into());
                    yield Part { headers, body };
                    rt::time::sleep(Duration::new(1, 0)).await;
                }
            }
        }
    }
}

/// HTTP endpoint to get mjpeg stream.
#[actix_web::get("/")]
async fn get(data: web::Data<Data>) -> impl Responder {
    const DELIMITER: &str = "foo";
    let o = multipart_stream::serialize(data.camera(), DELIMITER);
    let m = format!("multipart/x-mixed-replace; boundary={}", DELIMITER);
    HttpResponse::Ok().content_type(m).streaming(o)
}

/// Parses command line arguments and starts the HTTP server.
#[actix_web::main]
async fn main() -> Result<(), impl std::error::Error> {
    let args = Args::parse();
    let data = Data(args.imgs.into());
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .service(get)
    })
    .bind(args.addr)?
    .run()
    .await
}

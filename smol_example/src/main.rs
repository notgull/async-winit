//! Combine smol's networking primitives with the async-winit reactor.
//!
//! smol's I/O reactor runs on another thread, giving the main thread a chance to
//! run the winit reactor.

use async_winit::dpi::PhysicalSize;
use async_winit::event_loop::EventLoop;
use async_winit::window::Window;
use async_winit::ThreadUnsafe;

use color_eyre::eyre::{bail, eyre, Context, Error, Result};

use http::uri::Scheme;
use http::uri::Uri;
use smol::channel::bounded;
use smol::prelude::*;
use smol::Async;

use std::cell::RefCell;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::rc::Rc;
use std::sync::Arc;

use theo::{Display, RenderContext, Surface};

fn main() {
    // TODO: Convert to a library and use main2() as the entry point on Android.
    main2(EventLoop::new())
}

fn main2(event_loop: EventLoop<ThreadUnsafe>) {
    let target = event_loop.window_target().clone();

    event_loop.block_on(async move {
        // Overall program state.
        let state = RefCell::new(State::new());

        // Context for graphics.
        let gdisplay =
            RefCell::new(unsafe { Display::new(&target).expect("Failed to create display") });

        // Create an executor to handle all of our tasks.
        let executor = Rc::new(smol::LocalExecutor::new());

        // Create a channel that signifies that the HTTP query should run again.
        let (run_again, try_again) = bounded(1);

        // Spawn the future that does the HTTP work on this executor.
        executor
            .spawn({
                let executor = executor.clone();
                let state = &state;

                async move {
                    loop {
                        // Run the URL queries.
                        if let Err(e) = make_url_queries(state, &executor).await {
                            let mut stderr_writer = smol::Unblock::new(std::io::stderr());
                            stderr_writer
                                .write_all(format!("Error: {}", e).as_bytes())
                                .await
                                .ok();
                        }

                        // Indicate that we are no longer running.
                        state.borrow_mut().running = false;

                        // Wait for the next run.
                        try_again.recv().await.ok();

                        // Indicate that we are running again.
                        state.borrow_mut().running = true;
                    }
                }
            })
            .detach();

        loop {
            // Wait for the application to become resumed, poll the executor while we do.
            executor.run(target.resumed()).await;

            // Create a window.
            let window = Window::<ThreadUnsafe>::new().await.unwrap();
            state.borrow_mut().use_window(&window);

            // Wait for the application to be suspended.
            let mut suspend_guard = target.suspended().wait_guard();

            // Wait for the window to close.
            let mut wait_for_close = executor.spawn({
                let window = window.clone();
                async move {
                    window.close_requested().wait_once().await;
                    None
                }
            });

            // Draw to the window.
            let draw = executor.spawn({
                let window = window.clone();
                let state = &state;
                let gdisplay = &gdisplay;

                async move {
                    let mut graphics_context = None;
                    let mut draw_guard = window.redraw_requested().wait_guard();

                    loop {
                        // Wait until we need to draw.
                        let _guard = draw_guard.wait().await;

                        // Get the window's size.
                        let size = window.inner_size().await;

                        // Get the graphics context.
                        let graphics = match &mut graphics_context {
                            Some(graphics) => graphics,
                            graphics @ None => graphics.insert(
                                unsafe {
                                    gdisplay.borrow_mut().make_surface(
                                        &window,
                                        size.width,
                                        size.height,
                                    )
                                }
                                .await
                                .unwrap(),
                            ),
                        };

                        // Draw with the state.
                        state.borrow_mut().draw(gdisplay, graphics, size);

                        // Flush the graphics context.
                        gdisplay.borrow_mut().present().await;
                    }
                }
            });

            // Try to re-run the HTTP queries when the "R" key is pressed.
            let rerun_http = executor.spawn({
                let state = &state;
                let window = window.clone();
                let run_again = run_again.clone();

                async move {
                    window
                        .received_character()
                        .wait_many()
                        .for_each(|ch| {
                            if (ch == 'R' || ch == 'r') && !state.borrow().running {
                                run_again.try_send(()).ok();
                            }
                        })
                        .await;
                }
            });

            // Run the executor until either the window closes or the application suspends.
            let hold_guard = async {
                let hold_guard = suspend_guard.wait().await;
                Some(hold_guard)
            }
            .or(executor.run(&mut wait_for_close))
            .await;

            if let Some(guard) = hold_guard {
                // Wait for the tasks to die before suspending.
                rerun_http.cancel().await;
                wait_for_close.cancel().await;
                draw.cancel().await;
                drop((window, guard));
                state.borrow_mut().drop_window();
            } else {
                target.exit().await;
            }
        }
    });
}

async fn make_url_queries<'a>(
    state: &'a RefCell<State>,
    ex: &smol::LocalExecutor<'a>,
) -> std::io::Result<()> {
    // Read the urls.txt file.
    state.borrow_mut().requests.clear();
    let urls_path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "urls.txt".into());
    let urls = smol::fs::read_to_string(urls_path).await?;

    // Convert them into HTTP requests.
    for url in urls.lines() {
        let url = url.trim();
        if url.is_empty() {
            continue;
        }

        let url = url.to_string();
        state.borrow_mut().requests.push(HttpRequest {
            url: url.into(),
            status: HttpStatus::NotStarted,
        });
        if let Some(window) = &state.borrow().current_window {
            window.request_redraw();
        }
    }

    // Spawn the HTTP requests.
    let num_urls = state.borrow().requests.len();
    let tasks = (0..num_urls)
        .map(|i| {
            ex.spawn(async move {
                if let Err(e) = ping_address(state, i).await {
                    state.borrow_mut().requests[i].status = HttpStatus::Error(e);
                }
            })
        })
        .collect::<Vec<_>>();

    // Wait for all of the tasks to complete.
    for task in tasks {
        task.await;
    }

    Ok(())
}

async fn ping_address(state: &RefCell<State>, i: usize) -> Result<()> {
    let update = |status| {
        state.borrow_mut().requests[i].status = status;
        if let Some(window) = &state.borrow().current_window {
            window.request_redraw();
        }
    };

    // First, figure out where we need to connect to.
    let url = state.borrow().requests[i].url.clone();

    // Parse the URL.
    let url = url.parse::<Uri>().context("Failed to parse URL")?;

    // Find out where we want to connect to.
    let host = url.host().ok_or_else(|| eyre!("Hostname not found"))?;
    let scheme = if url.scheme() == Some(&Scheme::HTTP) {
        HttpScheme::Http
    } else if url.scheme() == Some(&Scheme::HTTPS) {
        HttpScheme::Https
    } else {
        bail!("Unsupported scheme")
    };

    let port = match url.port() {
        Some(port) => port.as_u16(),
        None => match scheme {
            HttpScheme::Http => 80,
            HttpScheme::Https => 443,
        },
    };

    // Resolve the address.
    let addr_task = smol::unblock({
        let host = host.to_owned();
        move || ToSocketAddrs::to_socket_addrs(&(host, port))
    });

    // Wait for DNS resolution.
    update(HttpStatus::DnsResolve);
    let addrs = addr_task.await.context("DNS resolution failed")?;

    // Connect to one of the addresses.
    update(HttpStatus::Connecting);
    let stream = connect_to_sockets(smol::Unblock::with_capacity(2, addrs)).await?;

    // Yield here to let other streams make progress.
    smol::future::yield_now().await;

    // Send the HTTP request over the given scheme.
    match scheme {
        HttpScheme::Http => http_over_stream(state, i, url, stream).await,

        HttpScheme::Https => {
            update(HttpStatus::EstablishingTls);

            // Establish a client configuration.
            let mut root_cert_store = async_rustls::rustls::RootCertStore::empty();
            root_cert_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(
                |ta| {
                    async_rustls::rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                        ta.subject,
                        ta.spki,
                        ta.name_constraints,
                    )
                },
            ));

            let client_config = async_rustls::rustls::client::ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth();

            let connector = async_rustls::TlsConnector::from(Arc::new(client_config));

            // Connect over TLS.
            let stream = connector
                .connect(
                    async_rustls::rustls::ServerName::try_from(host.to_string().as_str()).unwrap(),
                    stream,
                )
                .await?;

            // Send the HTTP request.
            http_over_stream(state, i, url, stream).await
        }
    }
}

async fn connect_to_sockets(sockets: impl Stream<Item = SocketAddr>) -> Result<Async<TcpStream>> {
    let mut last_err = None;

    let streams = sockets.then(Async::<TcpStream>::connect);
    smol::pin!(streams);

    streams
        .find_map(|result| match result {
            Ok(stream) => Some(stream),
            Err(e) => {
                last_err = Some(e.into());
                None
            }
        })
        .await
        .ok_or_else(|| last_err.unwrap_or_else(|| eyre!("No sockets were available")))
}

async fn http_over_stream(
    state: &RefCell<State>,
    i: usize,
    url: Uri,
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
) -> Result<()> {
    let update = |status| {
        state.borrow_mut().requests[i].status = status;
        if let Some(window) = &state.borrow().current_window {
            window.request_redraw();
        }
    };

    update(HttpStatus::Sending);
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        url.path(),
        url.host().unwrap()
    );

    stream.write_all(request.as_bytes()).await?;

    update(HttpStatus::Receiving);

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;

    // Yield here to let other streams make progress.
    smol::future::yield_now().await;

    // Parse the first line at UTF-8.
    let first_line =
        std::str::from_utf8(&response[..response.iter().position(|&b| b == b'\r').unwrap()])?;

    // Parse the status code.
    let status_code = first_line.split(' ').nth(1).unwrap().parse::<u16>()?;

    // Update the status code.
    update(HttpStatus::Done(status_code));

    println!("{} returned status code {}", url, status_code);

    Ok(())
}

#[derive(Copy, Clone)]
enum HttpScheme {
    Http,
    Https,
}

struct State {
    running: bool,
    requests: Vec<HttpRequest>,
    current_window: Option<Window<ThreadUnsafe>>,
}

impl State {
    fn new() -> State {
        Self {
            requests: Vec::new(),
            running: true,
            current_window: None,
        }
    }

    fn use_window(&mut self, window: &Window<ThreadUnsafe>) {
        assert!(self.current_window.is_none());
        self.current_window = Some(window.clone());
    }

    fn drop_window(&mut self) {
        self.current_window = None;
    }

    fn draw(
        &mut self,
        gdisplay: &RefCell<Display>,
        surface: &mut Surface,
        size: PhysicalSize<u32>,
    ) {
        use piet::{RenderContext as _, Text as _, TextLayout as _, TextLayoutBuilder as _};

        // Create a drawing context.
        let mut gdisplay = gdisplay.borrow_mut();
        let mut context = RenderContext::new(&mut gdisplay, surface, size.width, size.height)
            .expect("Failed to create render context");

        // Fill the background.
        context.clear(None, piet::Color::rgb(0.9, 0.9, 0.9));

        let mut current_y = 10.0;

        // Draw each HTTP request.
        for request in &self.requests {
            // Draw the text.
            let text = request
                .status
                .with_status(|status| format!("{}\r\n{}", request.url, status));
            let layout = context
                .text()
                .new_text_layout(text)
                .font(piet::FontFamily::SERIF, 12.0)
                .build()
                .unwrap();
            context.draw_text(&layout, (10.0, current_y));

            // Move the text down.
            current_y += layout.size().height + 10.0;
        }

        // Flush the drawing context.
        context.finish().unwrap();
        context.status().unwrap();
    }
}

struct HttpRequest {
    url: Rc<str>,
    status: HttpStatus,
}

enum HttpStatus {
    NotStarted,
    DnsResolve,
    Connecting,
    EstablishingTls,
    Sending,
    Receiving,
    Done(u16),
    Error(Error),
}

impl HttpStatus {
    fn with_status<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        match self {
            Self::NotStarted => f("Not started"),
            Self::DnsResolve => f("Resolving hostname"),
            Self::Connecting => f("Connecting to server"),
            Self::EstablishingTls => f("Establishing TLS handshake"),
            Self::Sending => f("Sending request"),
            Self::Receiving => f("Receiving response"),
            Self::Done(status) => f(&format!("Finished with status code: {}", status)),
            Self::Error(err) => f(&format!("Error: {}", err)),
        }
    }
}

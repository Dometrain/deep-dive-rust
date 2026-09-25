//! Module 7's browser client: a Leptos CSR app over the existing backend.
//!
//! The shape to notice is that there is no bespoke "task" type here -- the
//! frontend deserializes straight into `todo_shared::Task`, the *same* type
//! the backend serializes. `POST /tasks` doesn't even update the list
//! directly: it just fires the request, and the new task comes back through
//! the Module 7 `/ws` broadcast like any other client's would, proving the
//! real-time path end to end.

use futures::StreamExt;
use gloo_net::http::Request;
use gloo_net::websocket::{futures::WebSocket, Message};
use leptos::prelude::*;
use serde::Deserialize;
use todo_shared::{CreateTaskRequest, LoginRequest, LoginResponse, RegisterRequest, Task};
use wasm_bindgen_futures::spawn_local;

/// Where the `todo-ws-app` backend is expected to be listening (`cargo run`
/// in the crate root binds here by default). Cross-origin from `trunk serve`,
/// which is why the backend now sends permissive dev CORS headers -- see the
/// `Cors` builder in the backend's `main.rs`.
const API: &str = "http://127.0.0.1:8080";
/// The WebSocket endpoint's base -- a real client appends `?token=<jwt>` so
/// the backend can authenticate the handshake (a browser can't send an
/// `Authorization` header on a WebSocket upgrade, so the token travels in
/// the query string instead; see the backend's `routes::websocket`).
const WS_BASE: &str = "ws://127.0.0.1:8080/ws";

/// The backend's failure body (`error::json_error` on the backend side):
/// `{ "error": "..." }`. Parsed so a `/register` `409` can tell the user
/// "Username already taken" in the server's own words instead of something
/// generic the browser made up.
#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

fn main() {
    // Readable panics in the browser console instead of opaque WASM traps.
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

/// Loads the current task list once, right after login. Live additions after
/// this arrive over the WebSocket instead (see [`subscribe_ws`]).
fn load_tasks(
    token: RwSignal<Option<String>>,
    tasks: RwSignal<Vec<Task>>,
    error: RwSignal<Option<String>>,
) {
    let Some(bearer) = token.get_untracked() else {
        return;
    };
    spawn_local(async move {
        match Request::get(&format!("{API}/tasks"))
            .header("Authorization", &format!("Bearer {bearer}"))
            .send()
            .await
        {
            Ok(response) => match response.json::<Vec<Task>>().await {
                // Newest first, matching the order `/ws` pushes live ones in.
                Ok(mut list) => {
                    list.reverse();
                    tasks.set(list);
                }
                Err(err) => error.set(Some(format!("could not parse tasks: {err}"))),
            },
            Err(err) => error.set(Some(format!("could not load tasks: {err}"))),
        }
    });
}

/// Opens the `/ws` connection and, for the life of the page, prepends every
/// broadcast task to the list as it arrives. The token authenticates the
/// handshake as `?token=...` (see [`WS_BASE`]); the server then scopes the
/// feed to *this* user, so only this user's own tasks ever arrive here.
/// Dropping the read half when the socket closes is the entire teardown --
/// the mirror image of the server side, where dropping the
/// `broadcast::Receiver` is the whole cleanup.
fn subscribe_ws(
    token: RwSignal<Option<String>>,
    tasks: RwSignal<Vec<Task>>,
    error: RwSignal<Option<String>>,
) {
    let Some(bearer) = token.get_untracked() else {
        return;
    };
    let url = format!("{WS_BASE}?token={bearer}");
    let socket = match WebSocket::open(&url) {
        Ok(socket) => socket,
        Err(err) => {
            error.set(Some(format!("could not open /ws: {err}")));
            return;
        }
    };
    let (_writer, mut reader) = socket.split();
    spawn_local(async move {
        while let Some(Ok(Message::Text(text))) = reader.next().await {
            if let Ok(task) = serde_json::from_str::<Task>(&text) {
                tasks.update(|list| list.insert(0, task));
            }
        }
    });
}

#[component]
fn App() -> impl IntoView {
    let token = RwSignal::new(None::<String>);
    let tasks = RwSignal::new(Vec::<Task>::new());
    let error = RwSignal::new(None::<String>);

    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let new_title = RwSignal::new(String::new());

    // Which of the two auth endpoints the form targets. `false` (log in) is
    // the entry state -- a fresh database has no users, so the register mode
    // is the one a first-time visitor flips to.
    let registering = RwSignal::new(false);

    // Posts the credentials to `/login` or `/register`, whichever mode the
    // form is in. Both return the same `LoginResponse { token }` on success
    // -- `/register` deliberately logs a new user straight in (see its
    // handler) -- so the authenticated transition below is shared, not
    // forked per endpoint.
    let do_auth = move || {
        let register = registering.get_untracked();
        let endpoint = if register { "/register" } else { "/login" };
        let credentials = (username.get_untracked(), password.get_untracked());
        spawn_local(async move {
            // Same wire shape today, but each endpoint gets its own shared
            // type -- matching the backend's deliberate `RegisterRequest` /
            // `LoginRequest` split, so the two can grow apart without this
            // call site silently noticing.
            let built = if register {
                Request::post(&format!("{API}{endpoint}")).json(&RegisterRequest {
                    username: credentials.0,
                    password: credentials.1,
                })
            } else {
                Request::post(&format!("{API}{endpoint}")).json(&LoginRequest {
                    username: credentials.0,
                    password: credentials.1,
                })
            };
            let built = match built {
                Ok(built) => built,
                Err(err) => {
                    error.set(Some(format!("could not build request: {err}")));
                    return;
                }
            };
            match built.send().await {
                Ok(response) if response.ok() => match response.json::<LoginResponse>().await {
                    Ok(body) => {
                        token.set(Some(body.token));
                        error.set(None);
                        load_tasks(token, tasks, error);
                        subscribe_ws(token, tasks, error);
                    }
                    Err(err) => error.set(Some(format!("bad {endpoint} response: {err}"))),
                },
                // The backend shapes every failure body the same way
                // (`{ "error": "..." }`), e.g. `/register`'s `409` "Username
                // already taken" -- surface the server's words when present.
                Ok(response) => {
                    let fallback = if register {
                        "Registration failed"
                    } else {
                        "Invalid username or password"
                    };
                    let message = response
                        .text()
                        .await
                        .ok()
                        .and_then(|text| serde_json::from_str::<ErrorBody>(&text).ok())
                        .map(|body| body.error)
                        .unwrap_or_else(|| fallback.into());
                    error.set(Some(message));
                }
                Err(err) => error.set(Some(format!("request failed: {err}"))),
            }
        });
    };

    let create_task = move || {
        let title = new_title.get_untracked();
        if title.trim().is_empty() {
            return;
        }
        let Some(bearer) = token.get_untracked() else {
            return;
        };
        new_title.set(String::new());
        spawn_local(async move {
            let request = CreateTaskRequest {
                title,
                description: None,
                due_date: None,
            };
            // Fire and forget: the created task returns to us (and every other
            // connected client) via the `/ws` broadcast, so there's nothing to
            // append here by hand.
            if let Ok(built) = Request::post(&format!("{API}/tasks"))
                .header("Authorization", &format!("Bearer {bearer}"))
                .json(&request)
            {
                let _ = built.send().await;
            }
        });
    };

    let login_view = move || {
        view! {
            <p>"Log in -- or register a first account -- to see and create tasks."</p>
            <form on:submit=move |ev| { ev.prevent_default(); do_auth(); }>
                <input
                    placeholder="Username"
                    prop:value=move || username.get()
                    on:input=move |ev| username.set(event_target_value(&ev))
                />
                <input
                    type="password"
                    placeholder="Password"
                    prop:value=move || password.get()
                    on:input=move |ev| password.set(event_target_value(&ev))
                />
                <button type="submit">
                    {move || if registering.get() { "Register" } else { "Log in" }}
                </button>
            </form>
            <p>
                <button
                    class="link"
                    on:click=move |_| registering.set(!registering.get_untracked())
                >
                    {move || if registering.get() {
                        "Already have an account? Log in"
                    } else {
                        "No account yet? Register"
                    }}
                </button>
            </p>
        }
    };

    let board_view = move || {
        view! {
            <p class="live">"\u{25cf} live \u{2014} new tasks appear in real time via /ws"</p>
            <form on:submit=move |ev| { ev.prevent_default(); create_task(); }>
                <input
                    placeholder="New task title"
                    prop:value=move || new_title.get()
                    on:input=move |ev| new_title.set(event_target_value(&ev))
                />
                <button type="submit">"Add"</button>
            </form>
            <ul>
                <For
                    each=move || tasks.get()
                    key=|task| task.id
                    children=move |task| {
                        let class = if task.completed { "done" } else { "" };
                        view! { <li class=class>{task.title}</li> }
                    }
                />
            </ul>
        }
    };

    view! {
        <h1>"Todo \u{2014} Live"</h1>
        {move || error.get().map(|msg| view! { <p class="error">{msg}</p> })}
        {move || if token.get().is_some() {
            board_view().into_any()
        } else {
            login_view().into_any()
        }}
    }
}

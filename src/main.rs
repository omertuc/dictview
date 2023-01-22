use axum::{
    body::Body,
    extract::{Host, State},
    handler::HandlerWithoutStateExt,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    BoxError, Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use http::{header::AUTHORIZATION, Uri};
use levenshtein::levenshtein;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::{
    collections::{HashMap, HashSet},
    iter::once,
};
use std::{fs::read, path::PathBuf};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::{auth::RequireAuthorizationLayer, trace::TraceLayer};

use std::sync::Arc;

type Words = HashMap<String, Vec<(String, String)>>;

struct Gst {
    tree: TreeNode,
}

struct TreeNode {
    words: HashSet<String>,
    children: HashMap<char, TreeNode>,
}

impl TreeNode {
    fn new() -> TreeNode {
        TreeNode {
            words: HashSet::new(),
            children: HashMap::new(),
        }
    }
}

struct AppState {
    gst: Arc<Gst>,
    definitions: Words,
}

impl Gst {
    fn new(words: Vec<String>) -> Self {
        let tree = TreeNode::new();
        let mut gst = Self { tree };
        gst.add_words(words);
        gst
    }

    fn add_words(&mut self, words: Vec<String>) {
        for word in words.iter() {
            for suffix_length in 0..word.len() {
                let suffix = &word.chars().skip(suffix_length).collect::<String>();

                let mut cur = &mut self.tree;
                for c in suffix.chars() {
                    cur.children.entry(c).or_insert_with(|| TreeNode {
                        words: HashSet::from([word.clone()]),
                        children: HashMap::new(),
                    });

                    cur = cur.children.get_mut(&c).unwrap();

                    cur.words.insert(word.to_string());
                }
            }
        }
    }

    fn search(&self, word_to_find: String) -> Vec<String> {
        let mut cur = &self.tree;
        for c in word_to_find.chars() {
            if !cur.children.contains_key(&c) {
                return vec![];
            }
            cur = cur.children.get(&c).unwrap();
        }
        cur.words
            .iter()
            .map(|word| word.chars().collect())
            .collect()
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Password to protect the service
    #[arg(short, long)]
    password: String,

    /// Path of JSON file containing words
    #[arg(short, long)]
    words_file: String,

    /// Port to listen on
    #[arg(short, long)]
    https_port: u16,

    /// Port to listen on
    #[arg(short, long)]
    http_port: u16,

    /// HTTPs certificate file
    #[arg(short, long)]
    tlscert: String,

    /// HTTPs key file
    #[arg(short, long)]
    tlskey: String,
}

#[tokio::main]
pub async fn main() {
    let args = Args::parse();

    println!("Opening file...");
    let buf = read(args.words_file).unwrap();
    println!("Parsing JSON...");
    let dict: Vec<Vec<String>> = serde_json::from_slice(&buf).unwrap();
    println!("Extracting words...");
    let words = dict.iter().map(|word| word[3].to_string()).collect();
    println!("Building GST...");
    let tree = Gst::new(words);
    println!("Index ready");

    let mut definitions: Words = HashMap::new();
    for word in dict.iter() {
        definitions
            .entry(word[3].to_string())
            .or_insert_with(std::vec::Vec::new)
            .push((word[1].to_string(), word[0].to_string()));
    }

    let shared_state = Arc::new(AppState {
        gst: Arc::new(tree),
        definitions,
    });

    let config =
        RustlsConfig::from_pem_file(PathBuf::from(&args.tlscert), PathBuf::from(&args.tlskey))
            .await
            .unwrap();

    let ports = Ports {
        http: args.http_port,
        https: args.https_port,
    };
    // optional: spawn a second server to redirect http requests to this server
    tokio::spawn(redirect_http_to_https(ports));

    let app = Router::new()
        .route("/", get(root))
        .route("/complete", post(complete))
        .route("/define", post(define))
        .layer(SetSensitiveRequestHeadersLayer::new(once(AUTHORIZATION)))
        .layer(RequireAuthorizationLayer::basic("", args.password.as_str()))
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], ports.https));
    println!(
        "listening on {} using key {} and cert {}",
        addr, args.tlskey, args.tlscert
    );
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Clone, Copy)]
struct Ports {
    http: u16,
    https: u16,
}

async fn root() -> impl IntoResponse {
    Response::builder()
                .status(StatusCode::OK)
                .header(
                    "content-type",
                    "text/html",
                )
                .body(Body::from(r#"<!DOCTYPE html>

<html>
<head>
    <meta charset="utf-8">
    <title>Dictionary</title>
    <script>
        function complete() {
            const input = document.getElementById("input");
            const suggestions = document.getElementById("suggestions");
            const xhr = new XMLHttpRequest();
            xhr.open("POST", "/complete");
            xhr.setRequestHeader("Content-Type", "application/json");
            xhr.onload = function() {
                const data = JSON.parse(xhr.responseText);
                suggestions.innerHTML = data.suggestions.map(
                    word => `<div class="highlight" onclick="define('${word.key}')">${word.word_display}</div>`
                ).join("<br>");
            };
            xhr.send(JSON.stringify({text: input.value}));
        }

        function define(word) {
            const input = document.getElementById("input");
            const xhr = new XMLHttpRequest();
            const definitions = document.getElementById("definitions");
            xhr.open("POST", "/define");
            xhr.setRequestHeader("Content-Type", "application/json");
            xhr.onload = function() {
                const data = JSON.parse(xhr.responseText);
                definitions.innerHTML = data.definitions.join("<br><br>");
            };
            xhr.send(JSON.stringify({text: word}));
            window.scrollTo(0, 0);
        }
    </script>
    <style>
        body {
            direction: rtl;
            background: linear-gradient(-70deg, #7a9cc2, #3ce782, #8cffbb, #16745e);
            background-size: 400% 400%;
            // animation: gradient 15s ease infinite;
            height: 100vh;
        }

        @keyframes gradient {
            0% {
                background-position: 0% 50%;
            }
            50% {
                background-position: 100% 50%;
            }
            100% {
                background-position: 0% 50%;
            }
        }
        .highlight {
            cursor: pointer;
            border-radius: 11px;
            display: inline-block;
            user-select: none;
            padding: 35px;
            border: 1px solid #256729;
            margin: 3px;
        }
        .highlight:active {
            background-color: #30c080;
        }
        #definitions {
            font-size: 40px;
            font-family: system-ui;
            width: 50p;
            padding: 40px;
            line-height: 55px;
            background: linear-gradient(#00551d, #000000);
            -webkit-background-clip: text;
            background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        #suggestions {
            font-size: 40px;
            font-family: system-ui;
            width: 50p;
            padding: 40px;
            line-height: 55px;
            background: linear-gradient(#00551d, #000000);
            -webkit-background-clip: text;
            background-clip: text;
            -webkit-text-fill-color: transparent;
            font-weight: bold;
        }
        #input {
            border-radius: 30px;
            border-style: solid;
            text-align: center;
            border-color: cadetblue;
            font-size: 3em;
            padding: 16px;
            width: 80%;
        }
        #form {
            margin-top: 5%;
            width: 100%;
            display: flex;
            justify-content: center;
        }
    </style>
</head>
<body>
    <form id="form" onsubmit="return false;">
        <input id="input" placeholder="חיפוש" type="text" oninput="complete()">
    </form>
    <div id="definitions"></div>
    <div id="suggestions"></div>
</body>
</html>
"#))
                .unwrap()
}

/// Calculates the minimum levenshtein distance between any of the words
/// in the given word list (space separated) and the given word.
fn multi_levenshtein(search: &str, words: &str) -> usize {
    words
        .split_whitespace()
        .map(|word| levenshtein(search, word))
        .min()
        .unwrap()
}

async fn complete(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UserText>,
) -> impl IntoResponse {
    let mut suggestion_list: Vec<Suggestion> = state
        .gst
        .search(payload.text.clone())
        .iter()
        .take(100)
        .map(|word| Suggestion {
            key: word.to_string(),
            word_display: state
                .definitions
                .get(word)
                .unwrap()
                .iter()
                .map(|word| word.clone().0)
                .collect::<Vec<String>>()
                .join(", "),
        })
        .collect();

    suggestion_list.sort_by_key(|word| multi_levenshtein(&payload.text, &word.key));
    suggestion_list.sort_by_key(|word| word.key.len());

    let suggestions = CompleteSuggestions {
        suggestions: suggestion_list,
    };

    (StatusCode::OK, Json(suggestions))
}

async fn define(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UserText>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(Definition {
            definitions: state
                .definitions
                .get(&payload.text)
                .unwrap()
                .iter()
                .map(|word| word.1.clone())
                .collect(),
        }),
    )
}

#[derive(Deserialize)]
struct UserText {
    text: String,
}

#[derive(Serialize)]
struct Suggestion {
    key: String,
    word_display: String,
}

#[derive(Serialize)]
struct CompleteSuggestions {
    suggestions: Vec<Suggestion>,
}

#[derive(Serialize)]
struct Definition {
    definitions: Vec<String>,
}

async fn redirect_http_to_https(ports: Ports) {
    fn make_https(host: String, uri: Uri, ports: Ports) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        let https_host = host.replace(&ports.http.to_string(), &ports.https.to_string());
        parts.authority = Some(https_host.parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(host, uri, ports) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(_) => Err(StatusCode::BAD_REQUEST),
        }
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], ports.http));

    axum::Server::bind(&addr)
        .serve(redirect.into_make_service())
        .await
        .unwrap();
}

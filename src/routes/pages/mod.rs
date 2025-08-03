use crate::{
    misc::{PageError, Theme},
    models::User,
    server::{CurrentUser, UserTheme},
};
use askama::Template;
use axum::{
    Router,
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::Redirect,
    routing::get,
};
use axum::response::{Html, IntoResponse};

pub fn pages_router() -> Router {
    Router::new()
        .route("/", get(home))
        .route("/login", get(login))
        .route("/proxy/google_image", get(proxy_google_image))
        .layer(middleware::from_fn(auth_middleware))
        .fallback(not_found)
}

#[derive(Template)]
#[template(path = "index.html")]
struct HomeTemplate {
    theme: Theme,
    user: Option<User>,
}

async fn home(CurrentUser(user): CurrentUser, UserTheme(theme): UserTheme) -> HomeTemplate {
    let theme = theme.unwrap_or_default();
    HomeTemplate {
        theme,
        user: Some(user),
    }
}

impl IntoResponse for HomeTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {err}"),
            )
                .into_response(),
        }
    }
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    theme: Theme,
    user: Option<User>,
}

async fn login(UserTheme(theme): UserTheme) -> LoginTemplate {
    let theme = theme.unwrap_or_default();
    LoginTemplate { theme, user: None }
}

impl IntoResponse for LoginTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {err}"),
            )
                .into_response(),
        }
    }
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    theme: Theme,
    user: Option<User>,
    error: PageError,
}

async fn not_found(UserTheme(theme): UserTheme) -> ErrorTemplate {
    let theme = theme.unwrap_or_default();

    ErrorTemplate {
        theme,
        user: None,
        error: PageError {
            message: "Not Found".to_owned(),
            status: StatusCode::NOT_FOUND,
        },
    }
}

impl IntoResponse for ErrorTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {err}"),
            )
                .into_response(),
        }
    }
}

pub async fn error_handler_middleware(
    UserTheme(theme): UserTheme,
    request: Request,
    next: Next,
) -> axum::response::Response {
    let path = request.uri().path().to_string();
    let response = next.run(request).await;

    if response.status().is_client_error() || response.status().is_server_error() {
        let theme = theme.unwrap_or_default();
        let status = response.status();
        let message = status
            .canonical_reason()
            .map(|s| s.to_owned())
            .unwrap_or_else(|| "Something went wrong".to_owned());

        if status == StatusCode::UNAUTHORIZED {
            if path == "/login" {
                let html = LoginTemplate { theme, user: None }.render().unwrap();
                return Html(html).into_response();
            } else {
                return Redirect::to("/login").into_response();
            }
        }

        let html = ErrorTemplate {
            theme,
            user: None,
            error: PageError { status, message },
        }
        .render()
        .unwrap();
        return Html(html).into_response();
    }
    response
}

pub async fn auth_middleware(
    _user: CurrentUser,
    req: Request,
    next: Next,
) -> axum::response::Response {
    // We can be here only if we manage to extract CurrentUser
    if req.uri().path() == "/login" {
        Redirect::to("/").into_response()
    } else {
        next.run(req).await
    }
}

mod filters {
    pub fn take<T: std::fmt::Display>(
        s: T,
        _: &dyn askama::Values,
        count: usize,
    ) -> ::askama::Result<String> {
        let s = s.to_string();
        Ok(s[0..count].to_string())
    }
}

use axum::{extract::Query, http::header, response::Response};
use reqwest::Client;
use std::collections::HashMap;

pub async fn proxy_google_image(Query(params): Query<HashMap<String, String>>) -> Response {
    let url = match params.get("url") {
        Some(u) => u,
        None => return (StatusCode::BAD_REQUEST, "Missing `url` param").into_response(),
    };

    let client = Client::new();
    let Ok(resp) = client.get(url).send().await else {
        return (StatusCode::BAD_GATEWAY, "Failed to fetch image").into_response();
    };

    let status = resp.status();
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| header::HeaderValue::from_static("image/jpeg"));

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read image").into_response();
        }
    };

    (status, [(header::CONTENT_TYPE, content_type)], bytes).into_response()
}

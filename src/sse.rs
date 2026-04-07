//! Datastar SSE (Server-Sent Events) fragment helpers.
//!
//! Datastar is the hypermedia reactivity layer — the only JavaScript
//! dependency in Pavilion. These helpers generate SSE responses that
//! Datastar interprets as DOM operations (merge or remove fragments).
//!
//! # Usage
//!
//! ```ignore
//! use pavilion::sse;
//!
//! // Replace #profile-detail with new HTML
//! Ok(sse::fragment("#profile-detail", html).into_response())
//!
//! // Remove an element from the DOM
//! Ok(sse::remove("#notification-banner").into_response())
//! ```

use std::convert::Infallible;

use axum::response::sse::{Event, Sse};
use futures::stream;

/// Return an SSE response with a Datastar fragment merge.
///
/// The fragment replaces the element matching `selector` using Datastar's
/// morph merge strategy.
pub fn fragment(
    selector: impl Into<String>,
    html: impl Into<String>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let selector = selector.into();
    let html = html.into();
    let data = format!("selector {selector}\nmerge morph\nfragment {html}");
    let event = Event::default()
        .event("datastar-merge-fragments")
        .data(data);
    Sse::new(stream::once(async move { Ok(event) }))
}

/// Return an SSE response that removes the element matching `selector`.
pub fn remove(
    selector: impl Into<String>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let selector = selector.into();
    let event = Event::default()
        .event("datastar-remove-fragments")
        .data(format!("selector {selector}"));
    Sse::new(stream::once(async move { Ok(event) }))
}
